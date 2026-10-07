//! The generation tools beyond plain music and SFX: core melodies, stems
//! conditioned on a melody, variations, extensions, remixes and ambient
//! loops. Each runs one ComfyUI workflow; the ones that start from an
//! existing track upload it and condition ACE-Step on it
//! ([`build_audio_conditioned_workflow`]).

use crate::audio_edit;
use crate::tools::{
    ExtendTrackParams, GenerateAmbientParams, GenerateCoreMelodyParams, GenerateStemParams,
    GenerateVariationParams, RemixParams, ToolError, project_file, run_comfyui_audio_workflow,
    sanitize,
};
use crate::wav;
use crate::workflows::music::{build_audio_conditioned_workflow, build_music_workflow};
use crate::workflows::sfx::build_sfx_workflow;
use crate::{MusicRequest, MusicSection, SfxCategory, SfxRequest};
use amigo_comfyui::ComfyUiClient;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where generated audio goes, relative to the project.
const OUT: &str = "assets/generated/audio";

/// Runs generation workflows for a project.
pub struct Generator<'a> {
    pub client: &'a ComfyUiClient,
    /// The project directory: inputs are read and outputs written relative
    /// to it.
    pub base: &'a Path,
}

impl Generator<'_> {
    /// An input file, which must exist inside the project.
    fn input(&self, path: &str) -> Result<PathBuf, ToolError> {
        project_file(self.base, path)
    }

    /// Run `workflow` and download its audio to `rel`; returns `rel`.
    fn run(&self, workflow: &amigo_comfyui::ComfyPrompt, rel: &str) -> Result<String, ToolError> {
        let target = self.base.join(rel);
        run_comfyui_audio_workflow(self.client, workflow, &target.to_string_lossy()).map_err(
            |e| {
                ToolError::Backend(format!(
                    "{e}. Is ComfyUI running? Check with amigo_audiogen_server_status"
                ))
            },
        )?;
        Ok(rel.to_string())
    }

    /// Upload a track for a `LoadAudio` node. ComfyUI's upload endpoint
    /// takes any file, whatever its name says.
    fn upload(&self, local: &Path) -> Result<String, ToolError> {
        self.client
            .upload_image(local)
            .map_err(|e| ToolError::Backend(format!("could not upload {}: {e}", local.display())))
    }

    /// A WAV input's length in seconds, or `fallback` for other formats.
    fn seconds(local: &Path, fallback: f32) -> f32 {
        wav::read_wav(local)
            .map(|w| w.duration_secs())
            .ok()
            .filter(|d| *d > 0.0)
            .unwrap_or(fallback)
    }

    /// The core melody of the clean-mode workflow: one lead instrument, no
    /// drums, in `key`.
    pub fn core_melody(&self, p: &GenerateCoreMelodyParams) -> Result<Value, ToolError> {
        check_duration(p.duration_secs)?;
        let style = crate::WorldAudioStyle::find(&p.world, Some(self.base));
        let flavour = if p.prompt.trim().is_empty() {
            style
                .map(|s| s.genre)
                .unwrap_or_else(|| "game music".into())
        } else {
            p.prompt.clone()
        };
        let mut extra = HashMap::new();
        extra.insert("key".into(), json!(p.key));
        extra.insert("instrumental".into(), json!(true));
        let request = MusicRequest {
            world: p.world.clone(),
            genre: format!(
                "solo melody, single lead instrument, no drums, no chords, {}, {flavour}",
                p.key
            ),
            bpm: p.bpm,
            duration_secs: p.duration_secs,
            lyrics: None,
            section: MusicSection::Custom("melody".into()),
            split_stems: false,
            extra,
        };
        let rel = format!(
            "{OUT}/melody/{}_{}_{}bpm.wav",
            sanitize(&p.world),
            sanitize(&p.key),
            p.bpm
        );
        let path = self.run(&build_music_workflow(&request), &rel)?;
        Ok(json!({ "path": path, "key": p.key, "bpm": p.bpm, "duration_secs": p.duration_secs }))
    }

    /// One stem (bass, drums, pads…) that follows a melody.
    pub fn stem(&self, p: &GenerateStemParams) -> Result<Value, ToolError> {
        let melody = self.input(&p.melody_ref)?;
        let stem_type = sanitize(p.stem_type.trim());
        if stem_type.is_empty() {
            return Err(ToolError::BadInput("stem_type is empty".into()));
        }
        let uploaded = self.upload(&melody)?;
        let request = MusicRequest {
            world: "stem".into(),
            genre: format!(
                "isolated {} part only, accompanies the reference melody, {}",
                p.stem_type, p.prompt
            ),
            bpm: p.bpm,
            duration_secs: Self::seconds(&melody, 30.0),
            section: MusicSection::Custom(stem_type.clone()),
            split_stems: false,
            ..MusicRequest::default()
        };
        let rel = format!("{OUT}/stems/{}_{stem_type}.wav", file_stem(&melody));
        let path = self.run(
            &build_audio_conditioned_workflow(&request, &uploaded, 0.8),
            &rel,
        )?;
        Ok(json!({ "path": path, "stem_type": p.stem_type, "melody": p.melody_ref }))
    }

    /// The same track, changed by `strength` (0 keeps it, 1 ignores it).
    pub fn variation(&self, p: &GenerateVariationParams) -> Result<Value, ToolError> {
        let input = self.input(&p.input)?;
        let uploaded = self.upload(&input)?;
        let mut extra = HashMap::new();
        if let Some(seed) = p.seed {
            extra.insert("seed".into(), json!(seed));
        }
        let request = MusicRequest {
            world: "variation".into(),
            genre: "variation of the reference track, same instruments and tempo".into(),
            duration_secs: Self::seconds(&input, 30.0),
            split_stems: false,
            extra,
            ..MusicRequest::default()
        };
        let suffix = p.seed.map(|s| format!("_{s}")).unwrap_or_default();
        let rel = format!(
            "{OUT}/variations/{}_variation{suffix}.wav",
            file_stem(&input)
        );
        let strength = p.strength.clamp(0.0, 1.0);
        let path = self.run(
            &build_audio_conditioned_workflow(&request, &uploaded, 1.0 - strength),
            &rel,
        )?;
        Ok(json!({ "path": path, "strength": strength, "seed": p.seed }))
    }

    /// The track followed by `extend_secs` of generated continuation,
    /// joined with a one-second crossfade. Needs a WAV input.
    pub fn extend(&self, p: &ExtendTrackParams) -> Result<Value, ToolError> {
        check_duration(p.extend_secs)?;
        let input = self.input(&p.input)?;
        let original = wav::read_wav(&input).map_err(ToolError::BadInput)?;
        let uploaded = self.upload(&input)?;
        let request = MusicRequest {
            world: "extension".into(),
            genre: "seamless continuation of the reference track, same key, tempo and instruments"
                .into(),
            duration_secs: p.extend_secs + 1.0,
            split_stems: false,
            ..MusicRequest::default()
        };
        let stem = file_stem(&input);
        let continuation = self.run(
            &build_audio_conditioned_workflow(&request, &uploaded, 0.7),
            &format!("{OUT}/extended/{stem}_continuation.wav"),
        )?;
        let generated = wav::read_wav(&self.base.join(&continuation))
            .map_err(|e| ToolError::Backend(format!("the continuation is not usable: {e}")))?;
        let joined = audio_edit::append(&original, &generated, original.sample_rate as usize);
        let rel = format!("{OUT}/extended/{stem}_extended.wav");
        wav::write_wav(&self.base.join(&rel), &joined).map_err(ToolError::Backend)?;
        Ok(json!({
            "path": rel,
            "continuation": continuation,
            "duration_secs": joined.duration_secs(),
        }))
    }

    /// The track re-imagined in another genre and tempo.
    pub fn remix(&self, p: &RemixParams) -> Result<Value, ToolError> {
        let input = self.input(&p.input)?;
        if p.genre.trim().is_empty() {
            return Err(ToolError::BadInput("a remix needs a genre".into()));
        }
        let uploaded = self.upload(&input)?;
        let request = MusicRequest {
            world: "remix".into(),
            genre: format!("{} remix of the reference track, keep its melody", p.genre),
            bpm: p.bpm,
            duration_secs: Self::seconds(&input, 30.0),
            split_stems: false,
            ..MusicRequest::default()
        };
        let rel = format!(
            "{OUT}/remixes/{}_{}_{}bpm.wav",
            file_stem(&input),
            sanitize(&p.genre),
            p.bpm
        );
        let path = self.run(
            &build_audio_conditioned_workflow(&request, &uploaded, 0.5),
            &rel,
        )?;
        Ok(json!({ "path": path, "genre": p.genre, "bpm": p.bpm }))
    }

    /// An ambience (Stable Audio). With `looping`, its end is crossfaded
    /// into its start so it repeats without a seam.
    pub fn ambient(&self, p: &GenerateAmbientParams) -> Result<Value, ToolError> {
        check_duration(p.duration_secs)?;
        let request = SfxRequest {
            prompt: format!("continuous soundscape, no sudden events, {}", p.prompt),
            duration_secs: p.duration_secs,
            variants: 1,
            trim_silence: false,
            normalize: true,
            category: SfxCategory::Ambient,
        };
        let rel = format!("{OUT}/ambient/{}.wav", sanitize(&p.prompt));
        let path = self.run(&build_sfx_workflow(&request), &rel)?;
        let mut response = json!({ "path": path, "looping": false });
        if p.looping {
            let local = self.base.join(&rel);
            match wav::read_wav(&local) {
                Ok(audio) => {
                    let xf = (audio.sample_rate as usize).min(audio.frames() / 4);
                    let looped = audio_edit::make_loop(&audio, audio.frames() - xf, xf);
                    wav::write_wav(&local, &looped).map_err(ToolError::Backend)?;
                    response["looping"] = json!(true);
                    response["duration_secs"] = json!(looped.duration_secs());
                }
                Err(e) => {
                    response["warning"] = json!(format!("generated, but not made loopable: {e}"));
                }
            }
        }
        Ok(response)
    }
}

fn check_duration(secs: f32) -> Result<(), ToolError> {
    if secs.is_finite() && secs > 0.0 && secs <= 600.0 {
        Ok(())
    } else {
        Err(ToolError::BadInput(format!(
            "duration must be in (0, 600] seconds, got {secs}"
        )))
    }
}

fn file_stem(path: &Path) -> String {
    sanitize(
        &path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use amigo_comfyui::fake::FakeComfyUi;

    /// A WAV of `secs` seconds of a quiet sine at 1 kHz sample rate.
    fn tone(secs: f32) -> wav::WavData {
        let rate = 1000;
        wav::WavData {
            sample_rate: rate,
            channels: 1,
            samples: (0..(secs * rate as f32) as usize)
                .map(|i| (i as f32 * 0.1).sin() * 0.5)
                .collect(),
        }
    }

    fn setup() -> (FakeComfyUi, tempfile::TempDir) {
        let fake = FakeComfyUi::start();
        fake.set_output_audio(wav::encode_wav(&tone(4.0)));
        (fake, tempfile::tempdir().unwrap())
    }

    fn params<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_core_melody_asks_for_one_instrument_in_the_key() {
        let (fake, dir) = setup();
        let client = ComfyUiClient::new(fake.config());
        let generator = Generator {
            client: &client,
            base: dir.path(),
        };
        let v = generator
            .core_melody(&params(
                json!({ "world": "caribbean", "key": "A minor", "bpm": 130 }),
            ))
            .unwrap();
        assert_eq!(
            v["path"],
            "assets/generated/audio/melody/caribbean_A_minor_130bpm.wav"
        );
        assert!(dir.path().join(v["path"].as_str().unwrap()).is_file());
        let inputs = &fake.prompts()[0]["prompt"]["2"]["inputs"];
        assert!(
            inputs["genre"].as_str().unwrap().contains("A minor"),
            "{inputs}"
        );
        assert_eq!(inputs["bpm"], 130);
        assert!(inputs.get("reference_audio").is_none());
    }

    #[test]
    fn stems_variations_and_remixes_condition_on_the_input() {
        let (fake, dir) = setup();
        wav::write_wav(&dir.path().join("theme.wav"), &tone(2.0)).unwrap();
        let client = ComfyUiClient::new(fake.config());
        let generator = Generator {
            client: &client,
            base: dir.path(),
        };

        let stem = generator
            .stem(&params(
                json!({ "stem_type": "bass", "melody_ref": "theme.wav" }),
            ))
            .unwrap();
        assert_eq!(stem["path"], "assets/generated/audio/stems/theme_bass.wav");
        let variation = generator
            .variation(&params(
                json!({ "input": "theme.wav", "strength": 0.25, "seed": 7 }),
            ))
            .unwrap();
        assert_eq!(
            variation["path"],
            "assets/generated/audio/variations/theme_variation_7.wav"
        );
        let remix = generator
            .remix(&params(
                json!({ "input": "theme.wav", "genre": "synthwave", "bpm": 100 }),
            ))
            .unwrap();
        assert_eq!(
            remix["path"],
            "assets/generated/audio/remixes/theme_synthwave_100bpm.wav"
        );

        assert_eq!(fake.uploads(), ["theme.wav"; 3]);
        let prompts = fake.prompts();
        for p in &prompts {
            assert_eq!(p["prompt"]["4"]["inputs"]["audio"], "theme.wav");
            // The input's length carries over.
            assert_eq!(p["prompt"]["2"]["inputs"]["duration"], 2.0);
        }
        assert_eq!(
            prompts[1]["prompt"]["2"]["inputs"]["conditioning_strength"],
            0.75
        );
        assert_eq!(prompts[1]["prompt"]["2"]["inputs"]["seed"], 7);
        assert!(
            prompts[2]["prompt"]["2"]["inputs"]["genre"]
                .as_str()
                .unwrap()
                .starts_with("synthwave")
        );

        for err in [
            generator.stem(&params(
                json!({ "stem_type": "bass", "melody_ref": "gone.wav" }),
            )),
            generator.remix(&params(json!({ "input": "theme.wav", "genre": " " }))),
        ] {
            assert!(matches!(err, Err(ToolError::BadInput(_))), "{err:?}");
        }
    }

    #[test]
    fn extending_appends_the_continuation_with_a_crossfade() {
        let (fake, dir) = setup();
        wav::write_wav(&dir.path().join("song.wav"), &tone(2.0)).unwrap();
        let client = ComfyUiClient::new(fake.config());
        let generator = Generator {
            client: &client,
            base: dir.path(),
        };
        let v = generator
            .extend(&params(json!({ "input": "song.wav", "extend_secs": 3.0 })))
            .unwrap();
        assert_eq!(
            v["path"],
            "assets/generated/audio/extended/song_extended.wav"
        );
        // 2 s + 4 s generated - 1 s overlap.
        let out = wav::read_wav(&dir.path().join(v["path"].as_str().unwrap())).unwrap();
        assert!(
            (out.duration_secs() - 5.0).abs() < 0.01,
            "{}",
            out.duration_secs()
        );
        assert_eq!(fake.prompts()[0]["prompt"]["2"]["inputs"]["duration"], 4.0);

        std::fs::write(dir.path().join("song.ogg"), b"OggS").unwrap();
        let err = generator
            .extend(&params(json!({ "input": "song.ogg", "extend_secs": 3.0 })))
            .unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{err}");
        assert_eq!(fake.prompts().len(), 1, "checked before generating");
    }

    #[test]
    fn a_looping_ambience_folds_its_tail_into_its_start() {
        let (fake, dir) = setup();
        let client = ComfyUiClient::new(fake.config());
        let generator = Generator {
            client: &client,
            base: dir.path(),
        };
        let v = generator
            .ambient(&params(
                json!({ "prompt": "ocean waves", "duration_secs": 4.0 }),
            ))
            .unwrap();
        assert_eq!(v["path"], "assets/generated/audio/ambient/ocean_waves.wav");
        assert_eq!(v["looping"], true);
        // 4 s with a one-second crossfade: 3 s.
        assert_eq!(v["duration_secs"], 3.0);
        assert!(
            fake.prompts()[0]["prompt"]["2"]["inputs"]["prompt"]
                .as_str()
                .unwrap()
                .contains("ocean waves")
        );
        assert!(
            generator
                .ambient(&params(json!({ "prompt": "x", "duration_secs": -1.0 })))
                .is_err()
        );
    }

    #[test]
    fn a_failed_run_is_a_backend_error() {
        let (fake, dir) = setup();
        fake.fail_prompts_with("out of memory");
        let client = ComfyUiClient::new(fake.config());
        let generator = Generator {
            client: &client,
            base: dir.path(),
        };
        let err = generator
            .core_melody(&params(json!({ "world": "dune" })))
            .unwrap_err();
        assert!(matches!(err, ToolError::Backend(_)));
        assert!(err.to_string().contains("out of memory"), "{err}");
    }
}
