//! Audio playback control: crossfaded looping music, delayed sounds, stopping
//! one sound by its handle, bus mutes and pause.
//!
//! The sounds are synthesised into WAV bytes at startup and loaded with
//! `load_sound_from_bytes`, so the demo ships no audio files.

use amigo_engine::prelude::*;
use std::sync::Arc;

const RATE: u32 = 22_050;

/// A mono 16-bit WAV of `seconds` of `f(t)` (t in seconds, result in -1..1).
fn wav(seconds: f32, f: impl Fn(f32) -> f32) -> Arc<[u8]> {
    let frames = (seconds * RATE as f32) as u32;
    let data_len = frames * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let t = i as f32 / RATE as f32;
        let s = (f(t).clamp(-1.0, 1.0) * 12_000.0) as i16;
        out.extend_from_slice(&s.to_le_bytes());
    }
    out.into()
}

fn tone(freq: f32, t: f32) -> f32 {
    (t * freq * std::f32::consts::TAU).sin()
}

/// A two-second arpeggio over `root`, which loops seamlessly.
fn arpeggio(root: f32) -> Arc<[u8]> {
    wav(2.0, move |t| {
        let step = ((t * 4.0) as usize) % 4;
        let freq = root * [1.0, 1.25, 1.5, 2.0][step];
        let env = 1.0 - (t * 4.0).fract();
        0.4 * tone(freq, t) * env
    })
}

struct AudioDemo {
    track: &'static str,
    drone: SoundHandle,
}

impl Game for AudioDemo {
    fn init(&mut self, ctx: &mut GameContext) {
        let sounds = [
            ("track_a", arpeggio(220.0)),
            ("track_b", arpeggio(293.66)),
            ("click", wav(0.08, |t| tone(880.0, t) * (1.0 - t / 0.08))),
            (
                "drone",
                wav(1.0, |t| 0.3 * tone(110.0, t) + 0.1 * tone(165.0, t)),
            ),
        ];
        for (name, bytes) in sounds {
            if let Err(e) = ctx.audio.load_sound_from_bytes(name, bytes) {
                eprintln!("{e}");
            }
        }
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        // 1 / 2: crossfade between two looping tracks over a second.
        if ctx.input.pressed(KeyCode::Digit1) {
            ctx.audio.start_music("track_a", Fade::secs(1.0));
            self.track = "A";
        }
        if ctx.input.pressed(KeyCode::Digit2) {
            ctx.audio.start_music("track_b", Fade::secs(1.0));
            self.track = "B";
        }
        // Space: a click now and two more, delayed, on the audio thread.
        if ctx.input.pressed(KeyCode::Space) {
            for (i, pitch) in [1.0, 1.25, 1.5].into_iter().enumerate() {
                ctx.audio.play(
                    "click",
                    &PlaySettings {
                        delay: i as f64 * 0.15,
                        playback_rate: pitch,
                        ..Default::default()
                    },
                );
            }
        }
        // D: start a looping drone, or stop it by its handle with a fade.
        if ctx.input.pressed(KeyCode::KeyD) {
            if ctx.audio.is_playing(self.drone) {
                ctx.audio.stop(self.drone, Fade::secs(0.5));
            } else {
                self.drone = ctx.audio.play(
                    "drone",
                    &PlaySettings {
                        bus: Bus::Ambient,
                        loops: LoopMode::Forever,
                        fade_in: Fade::secs(0.5),
                        ..Default::default()
                    },
                );
            }
        }
        // M / N: mute music / effects; the music keeps its place.
        if ctx.input.pressed(KeyCode::KeyM) {
            let muted = ctx.audio.is_bus_muted(Bus::Music);
            ctx.audio.set_bus_muted(Bus::Music, !muted);
        }
        if ctx.input.pressed(KeyCode::KeyN) {
            let muted = ctx.audio.is_bus_muted(Bus::Sfx);
            ctx.audio.set_bus_muted(Bus::Sfx, !muted);
        }
        // P: pause everything.
        if ctx.input.pressed(KeyCode::KeyP) {
            if ctx.audio.is_paused() {
                ctx.audio.resume_all(Fade::secs(0.2));
            } else {
                ctx.audio.pause_all(Fade::secs(0.2));
            }
        }
        // Up / Down: master volume.
        if ctx.input.pressed(KeyCode::ArrowUp) {
            let v = ctx.audio.master_volume() + 0.1;
            ctx.audio.set_master_volume(v.min(1.0), Fade::secs(0.1));
        }
        if ctx.input.pressed(KeyCode::ArrowDown) {
            let v = ctx.audio.master_volume() - 0.1;
            ctx.audio.set_master_volume(v.max(0.0), Fade::secs(0.1));
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let lines = [
            "=== Audio Demo ===".to_string(),
            format!("Music: track {}", self.track),
            String::new(),
            "[1]/[2] crossfade to track A / B".into(),
            "[Space] three clicks, two of them delayed".into(),
            "[D] drone on / off (stops by handle)".into(),
            "[M] mute music  [N] mute effects".into(),
            "[P] pause / resume everything".into(),
            "[Up]/[Down] master volume".into(),
        ];
        for (i, line) in lines.iter().enumerate() {
            ctx.draw_text(line, 20.0, 20.0 + i as f32 * 14.0, Color::WHITE);
        }
    }
}

fn main() {
    Engine::build()
        .title("Audio Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(AudioDemo {
            track: "-",
            drone: SoundHandle::NONE,
        });
}
