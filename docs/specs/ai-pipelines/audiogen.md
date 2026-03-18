---
status: done
crate: amigo_audiogen
depends_on: ["engine/audio"]
last_updated: 2026-03-18
---

# Audio Generation Pipeline (amigo_audiogen)

## Purpose

Provides an MCP server and Rust library for AI-powered music and sound effect generation using ACE-Step (music) and AudioGen (SFX), with stem separation via Demucs, a clean-mode per-stem generation pipeline, and audio post-processing including BPM detection, bar-boundary snapping, loop-point finding, crossfade, loudness normalization, and spectral validation.

Existing implementation in `tools/amigo_audiogen/src/` (8 files: `lib.rs`, `acestep.rs`, `audiogen.rs`, `processing.rs`, `stems.rs`, `clean_mode.rs`, `tools.rs`, `main.rs`).

## Public API

### Core Types (`lib.rs`)

```rust
/// A request to generate music via ACE-Step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicRequest {
    pub world: String,              // default: "default"
    pub genre: String,              // default: "" (uses world default)
    pub bpm: u32,                   // default: 120
    pub duration_secs: f32,         // default: 30.0
    pub lyrics: Option<String>,
    pub section: MusicSection,      // default: Calm
    pub split_stems: bool,          // default: true
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MusicSection {
    Calm,
    Tense,
    Battle,
    Boss,
    Victory,
    Menu,
    Custom(String),
}

/// A request to generate SFX via AudioGen.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxRequest {
    pub prompt: String,
    pub duration_secs: f32,         // default: 2.0
    pub variants: u32,              // default: 3
    pub trim_silence: bool,         // default: true
    pub normalize: bool,            // default: true
    pub category: SfxCategory,      // default: Gameplay
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfxCategory {
    Gameplay,
    UI,
    Ambient,
    Impact,
    Explosion,
    Magic,
    Voice,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicResult {
    pub full_track_path: String,
    pub stem_paths: HashMap<String, String>,
    pub detected_bpm: f32,
    pub generation_time_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxResult {
    pub output_paths: Vec<String>,
    pub durations: Vec<f32>,
    pub generation_time_ms: u64,
}
```

### WorldAudioStyle (`lib.rs`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldAudioStyle {
    pub name: String,
    pub genre: String,
    pub genre_tags: Vec<String>,
    pub default_bpm: u32,
    pub sfx_style: String,
    pub key_instruments: Vec<String>,
}

impl WorldAudioStyle {
    pub fn builtin_styles() -> Vec<WorldAudioStyle>;  // 6 styles
    pub fn find(name: &str) -> Option<WorldAudioStyle>;
}
```

Six built-in audio styles:

| World | Genre | Default BPM | Key Instruments |
|-------|-------|-------------|-----------------|
| caribbean | pirate shanty | 130 | accordion, fiddle, drums, bass |
| lotr | orchestral fantasy | 100 | strings, brass, choir, drums |
| dune | ambient electronic | 90 | synth pad, percussion, bass drone, vocal |
| matrix | synthwave | 140 | synth lead, synth bass, drums, arpeggios |
| got | medieval orchestral | 85 | cello, war drums, brass, strings |
| stranger_things | 80s synth | 110 | analog synth, drums machine, bass synth, pad |

### ACE-Step Client (`acestep.rs`)

```rust
pub struct AceStepConfig {
    pub host: String,      // default: "127.0.0.1"
    pub port: u16,         // default: 7860
}

pub struct AceStepClient {
    pub config: AceStepConfig,
}

impl AceStepClient {
    pub fn new(config: AceStepConfig) -> Self;
    pub fn build_params(&self, request: &MusicRequest) -> AceStepParams;
    pub fn generate(&self, request: &MusicRequest) -> Result<MusicResult, AceStepError>;
    pub fn download(&self, remote_path: &str, local_path: &str) -> Result<(), AceStepError>;
    pub fn health_check(&self) -> Result<bool, AceStepError>;
}

pub struct AceStepParams {
    pub prompt: String,
    pub lyrics: String,
    pub duration: f32,
    pub steps: u32,        // default: 100
    pub cfg_scale: f32,    // default: 5.0
    pub seed: i64,         // default: -1 (random)
}
```

### AudioGen Client (`audiogen.rs`)

```rust
pub struct AudioGenConfig {
    pub host: String,      // default: "127.0.0.1"
    pub port: u16,         // default: 7861
}

pub struct AudioGenClient {
    pub config: AudioGenConfig,
}

impl AudioGenClient {
    pub fn new(config: AudioGenConfig) -> Self;
    pub fn build_params(&self, request: &SfxRequest) -> AudioGenParams;
    pub fn generate(&self, request: &SfxRequest) -> Result<SfxResult, AudioGenError>;
    pub fn download(&self, remote_path: &str, local_path: &str) -> Result<(), AudioGenError>;
    pub fn health_check(&self) -> Result<bool, AudioGenError>;
}

pub struct AudioGenParams {
    pub prompt: String,
    pub duration: f32,     // clamped to max 10.0s
    pub num_samples: u32,
    pub temperature: f32,  // default: 1.0
    pub top_k: u32,        // default: 250
}
```

### Audio Processing (`processing.rs`)

```rust
pub struct AudioBuffer {
    pub samples: Vec<f32>,    // mono f32 samples
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32) -> Self;
    pub fn duration_secs(&self) -> f32;
    pub fn trim_silence(&mut self, threshold_db: f32);
    pub fn normalize(&mut self, target_db: f32);
    pub fn detect_bpm(&self) -> Option<f32>;
    pub fn find_loop_point(&self, target_duration_secs: f32) -> Option<usize>;
    pub fn apply_loop_crossfade(&mut self, loop_point: usize, crossfade_samples: usize);
    pub fn bar_snap(&mut self, bpm: f32, beats_per_bar: u32);
    pub fn spectral_validate(&self, style_hint: &str) -> SpectralReport;
}

pub fn db_to_linear(db: f32) -> f32;
pub fn linear_to_db(linear: f32) -> f32;

/// Run the full post-processing pipeline on an AudioBuffer.
/// Order: Normalize -> BPM Detect -> Bar Snap -> Loop Crossfade -> Spectral Validate
pub fn run_pipeline(
    buf: &mut AudioBuffer,
    normalize_db: f32,
    beats_per_bar: u32,
    target_loop_secs: Option<f32>,
    style_hint: &str,
) -> PostProcessResult;

pub struct PostProcessResult {
    pub detected_bpm: Option<f32>,
    pub bar_snap_samples: Option<usize>,
    pub loop_point: Option<usize>,
    pub spectral_report: Option<SpectralReport>,
    pub duration_secs: f32,
}

pub struct SpectralReport {
    pub passed: bool,
    pub band_energies: [f32; 3],    // [low, mid, high] ratios
    pub message: String,
}

/// Adaptive music configuration generated from stem analysis.
pub struct AdaptiveMusicConfig {
    pub section_name: String,
    pub bpm: f32,
    pub beats_per_bar: u32,
    pub layers: Vec<LayerConfig>,
}

pub struct LayerConfig {
    pub name: String,
    pub stem_file: String,
    pub base_volume: f32,
    pub rule: LayerRule,
}

/// Must match `LayerRule` in `engine/audio.md` exactly.
pub enum LayerRule {
    AlwaysOn,
    Threshold { param: String, above: f32, fade_secs: f32 },
    Lerp { param: String, from: f32, to: f32 },
    Toggle { param: String, fade_secs: f32 },
}
```

### Stem Separation (`stems.rs`)

```rust
pub enum StemModel {
    Demucs,        // 4 stems: drums, bass, vocals, other
    Demucs6,       // 6 stems: drums, bass, vocals, guitar, piano, other
    Custom(String),
}

pub struct StemSplitConfig {
    pub model: StemModel,           // default: Demucs
    pub output_dir: String,         // default: "assets/audio/stems"
    pub format: AudioFormat,        // default: Ogg
    pub normalize_stems: bool,      // default: true
}

pub enum AudioFormat { Wav, Ogg }

pub struct StemSplitResult {
    pub stems: HashMap<String, String>,   // stem_name -> file_path
    pub processing_time_ms: u64,
}

pub const DEMUCS_STEMS: &[&str] = &["drums", "bass", "vocals", "other"];
pub const DEMUCS6_STEMS: &[&str] = &["drums", "bass", "vocals", "guitar", "piano", "other"];

pub fn split_stems(
    input_path: &str, config: &StemSplitConfig,
) -> Result<StemSplitResult, StemError>;

pub fn generate_adaptive_config(
    stems: &StemSplitResult, section: &MusicSection, bpm: f32,
) -> AdaptiveMusicConfig;
```

### Clean Mode Pipeline (`clean_mode.rs`)

```rust
pub enum CleanModeStep {
    GenerateMelody,
    GenerateStem(String),
    Mix,
    PostProcess,
}

pub enum PipelineState {
    Idle,
    Running(CleanModeStep),
    Completed,
    Failed(String),
}

pub struct CleanModeConfig {
    pub world: String,
    pub prompt: String,
    pub key: String,               // default: "C minor"
    pub bpm: u32,                  // default: 120
    pub duration_secs: f32,        // default: 30.0
    pub stems: Vec<String>,        // default: ["melody", "bass", "drums", "harmony"]
    pub output_dir: String,        // default: "assets/audio/generated"
}

pub struct CleanModePipeline {
    pub config: CleanModeConfig,
    pub state: PipelineState,
    pub completed_steps: Vec<(CleanModeStep, String)>,
    pub melody_ref: Option<String>,
    pub stem_paths: HashMap<String, String>,
    pub final_output: Option<String>,
    pub elapsed_ms: u64,
}

impl CleanModePipeline {
    pub fn new(config: CleanModeConfig) -> Self;
    pub fn next_step(&self) -> Option<CleanModeStep>;
    pub fn complete_step(&mut self, step: CleanModeStep, output_path: String);
    pub fn fail(&mut self, error: impl Into<String>);
    pub fn begin_step(&mut self, step: CleanModeStep);
    pub fn progress(&self) -> f32;       // 0.0 to 1.0
    pub fn is_completed(&self) -> bool;
    pub fn is_failed(&self) -> bool;
}
```

### MCP Tools (`tools.rs`)

18 MCP tools exposed via `list_tools()` and dispatched via `dispatch_tool()`:

| Tool | Description |
|------|-------------|
| `amigo_audiogen_generate_music` | Generate a music track using ACE-Step |
| `amigo_audiogen_generate_sfx` | Generate sound effects using AudioGen |
| `amigo_audiogen_split_stems` | Split audio into stems via Demucs |
| `amigo_audiogen_process` | Post-process audio (trim, normalize, BPM, loop) |
| `amigo_audiogen_list_styles` | List available world audio styles |
| `amigo_audiogen_server_status` | Check ACE-Step and AudioGen server status |
| `amigo_audiogen_generate_core_melody` | Generate core melody for clean-mode workflow |
| `amigo_audiogen_generate_stem` | Generate individual stem conditioned on melody |
| `amigo_audiogen_generate_variation` | Generate a variation of an existing track |
| `amigo_audiogen_extend_track` | Extend a track by generating a continuation |
| `amigo_audiogen_remix` | Remix a track with different genre/BPM |
| `amigo_audiogen_generate_ambient` | Generate ambient/atmosphere loop |
| `amigo_audiogen_loop_trim` | Trim audio to optimal loop point |
| `amigo_audiogen_normalize` | Normalize audio to target dB level |
| `amigo_audiogen_convert` | Convert audio format (WAV, OGG, FLAC) |
| `amigo_audiogen_preview` | Generate short preview clip |
| `amigo_audiogen_list_models` | List available AI models |
| `amigo_audiogen_queue_status` | Check generation queue status |

## Behavior

### Music Generation (Quick Mode)

1. `MusicRequest` arrives with world, section, BPM, duration.
2. `AceStepClient::build_params()` constructs a prompt by combining:
   - World genre (from `WorldAudioStyle`, e.g., "pirate shanty")
   - BPM (e.g., "130 BPM")
   - Section mood (e.g., "intense, aggressive, driving" for Battle)
   - Genre tags (e.g., "folk, sea shanty, accordion, fiddle")
3. The prompt is sent to ACE-Step's Gradio API at `POST /api/predict`.
4. The generated audio file is downloaded.
5. If `split_stems` is true, the track is split via Demucs into 4 stems (drums, bass, vocals, other).
6. An `AdaptiveMusicConfig` is generated from the stems with layer rules:
   - drums: `Threshold { param: "tension", above: 0.3 }`
   - bass: `AlwaysOn`
   - vocals: `Threshold { param: "boss", above: 0.5 }`
   - other/guitar/piano: `Lerp { param: "tension", from: 0.2, to: 1.0 }`

### Music Generation (Clean Mode)

The clean-mode pipeline generates each stem individually for zero bleed:

1. **GenerateMelody**: Generate a core melody reference track.
2. **GenerateStem("bass")**: Generate bass stem conditioned on the melody.
3. **GenerateStem("drums")**: Generate drums stem conditioned on the melody.
4. **GenerateStem("harmony")**: Generate harmony stem conditioned on the melody.
5. **Mix**: Combine all stems into the final track.
6. **PostProcess**: Normalize, detect BPM, find loop points.

`CleanModePipeline::next_step()` drives the state machine, returning the next step to execute. The pipeline tracks progress as a ratio of completed steps to total steps.

### SFX Generation

1. `SfxRequest` arrives with prompt, duration, variants, category.
2. `AudioGenClient::build_params()` prepends a category-specific prefix:
   - `Gameplay` -> "game sound effect, "
   - `UI` -> "user interface click sound, subtle, "
   - `Ambient` -> "ambient environmental sound, looping, "
   - `Impact` -> "impact sound, punchy, "
   - `Explosion` -> "explosion sound, powerful, "
   - `Magic` -> "magical sound effect, sparkle, "
   - `Voice` -> "vocal sound, "
3. Duration is clamped to 10.0s (AudioGen's maximum).
4. Sent to AudioGen's Gradio API at `POST /api/predict`.
5. Multiple variants are downloaded and returned.

### Audio Post-Processing Pipeline

`run_pipeline()` executes 5 steps in fixed order:

1. **Normalize**: Scale peak amplitude to target dB (e.g., -1.0 dB).
2. **BPM Detect**: Autocorrelation on the energy envelope (RMS over 50ms windows). Searches BPM range 60-200 by finding the lag with highest autocorrelation. Requires at least 2 seconds of audio.
3. **Bar Snap**: Given detected BPM and beats-per-bar, truncate the audio to the nearest complete bar boundary. `samples_per_bar = sample_rate * 60 / bpm * beats_per_bar`.
4. **Loop Crossfade**: Find the best loop point near a target duration by searching for zero crossings that match the waveform's start. Apply a short crossfade (~10ms) at the loop boundary.
5. **Spectral Validation**: Compute energy ratios in low/mid/high frequency bands. Style-specific validation checks (e.g., ambient tracks should have >15% low-end energy, battle tracks should have >10% high-frequency energy).

### Stem Separation

`split_stems()` produces output paths for each stem based on the model:
- **Demucs (4-stem)**: drums, bass, vocals, other
- **Demucs6 (6-stem)**: drums, bass, vocals, guitar, piano, other

`generate_adaptive_config()` builds an `AdaptiveMusicConfig` from the stems with preset layer rules and volume levels (drums: 0.8, bass: 0.7, vocals: 0.6, other: 0.5). Layers are sorted alphabetically for deterministic output.

## Internal Design

### Gradio API Integration

Both ACE-Step and AudioGen run as Gradio servers. The clients communicate via `ureq` with `POST /api/predict` (sending `{"data": [...]}` payloads) and download files via `GET /file={path}`. Health checks use `GET /api/status`.

### Prompt Engineering

`AceStepClient::build_params()` combines multiple signal sources into the generation prompt:
1. World genre (from `WorldAudioStyle::genre`)
2. BPM value
3. Section mood descriptor (mapped from `MusicSection` enum)
4. Genre tags (from `WorldAudioStyle::genre_tags`, comma-joined)

Format: `"{genre} music, {bpm} BPM, {section_mood}, {genre_tags}"`

`AudioGenClient::build_params()` prepends a category-specific prefix to improve generation quality for each sound type.

### BPM Detection Algorithm

Uses autocorrelation on a windowed energy envelope:
1. Compute RMS energy in 50ms windows with 50% overlap.
2. Calculate the mean energy across all windows.
3. For each lag in the BPM range (60-200 BPM), compute the normalized autocorrelation.
4. The lag with the highest correlation corresponds to the detected tempo.
5. Convert lag to BPM: `60.0 * envelope_rate / best_lag`.

### Loop Point Finding

Searches a window around the target duration for an optimal loop cut point:
1. Define a search window of ~100ms around the target sample position.
2. Score each sample by: `abs(sample_value) + abs(sample - first_sample) * 0.5`.
3. The lowest-scoring position gives a zero crossing that matches the waveform start.
4. Crossfade blends the end of the loop with the beginning over ~10ms.

### Spectral Validation

Approximates frequency band energies without FFT:
- **Low band**: Energy of a running-average smoothed signal (window = sample_rate / 300).
- **High band**: Energy of a first-order difference signal (approximates high-pass).
- **Mid band**: Total energy minus low minus high.

Normalizes to ratios summing to 1.0, then applies style-specific thresholds.

### Clean Mode State Machine

`CleanModePipeline` uses a deterministic step ordering:
1. Melody must be generated first (`melody_ref` is `None`).
2. Non-melody stems are generated in config order.
3. Mix runs after all stems are complete.
4. PostProcess runs after mix.

`next_step()` returns `None` when all steps are done (triggering `PipelineState::Completed`) or when the pipeline has failed. `progress()` computes `completed_steps / (stems.len() + 2)` where +2 accounts for the mix and postprocess steps.

### Engine Config Generation (`.music.ron` / `.sfx.ron`)

The audiogen pipeline generates raw audio files (WAV/OGG) and stem splits, but the engine's adaptive music system requires structured configuration files. These bridge tools close the gap:

```rust
/// Generate a `.music.ron` file from an AdaptiveMusicConfig.
/// The output file is ready for the engine's AdaptiveMusicEngine to load.
pub fn write_music_ron(
    config: &AdaptiveMusicConfig,
    output_path: &str,
) -> Result<(), IoError>;

/// Generate a `.sfx.ron` file from SFX generation results.
/// Maps generated variants to SfxDefinition fields.
pub fn write_sfx_ron(
    sfx_result: &SfxResult,
    request: &SfxRequest,
    output_path: &str,
) -> Result<(), IoError>;
```

**`.music.ron` Format** (consumed by `AdaptiveMusicEngine`):

```ron
MusicSection(
    name: "caribbean_battle",
    bpm: 130.0,
    beats_per_bar: 4,
    layers: [
        (name: "drums", file: "audio/stems/caribbean_battle_drums.ogg",
         base_volume: 0.8, rule: Threshold(param: "tension", above: 0.3, fade_secs: 1.0)),
        (name: "bass", file: "audio/stems/caribbean_battle_bass.ogg",
         base_volume: 0.7, rule: AlwaysOn),
        (name: "vocals", file: "audio/stems/caribbean_battle_vocals.ogg",
         base_volume: 0.6, rule: Toggle(param: "boss", fade_secs: 0.5)),
        (name: "other", file: "audio/stems/caribbean_battle_other.ogg",
         base_volume: 0.5, rule: Lerp(param: "tension", from: 0.2, to: 1.0)),
    ],
    transitions: [
        (target: "caribbean_calm", transition: CrossfadeOnBar(bars: 2)),
    ],
    stingers: [
        (name: "wave_start", file: "audio/stingers/wave_horn.ogg", quantize: NextBar),
    ],
)
```

**`.sfx.ron` Format** (consumed by `SfxManager`):

```ron
SfxDefinition(
    files: ["audio/sfx/cannon_fire_01.ogg", "audio/sfx/cannon_fire_02.ogg", "audio/sfx/cannon_fire_03.ogg"],
    volume: 0.8,
    pitch_variance: 0.1,
    max_concurrent: 3,
    cooldown: Some(0.05),
)
```

**MCP Tools** (additional):

| Tool | Description |
|------|-------------|
| `amigo_audiogen_export_music_ron` | Generate `.music.ron` from stems + config |
| `amigo_audiogen_export_sfx_ron` | Generate `.sfx.ron` from SFX variants |

**Pipeline integration:**
1. `amigo_audiogen_generate_music` → produces WAV + stems
2. `amigo_audiogen_export_music_ron` → produces `.music.ron` referencing those stems
3. Engine hot-reloads the `.music.ron` and plays the new section

This ensures generated audio is immediately usable by the engine without manual config authoring.

## Non-Goals

- Runtime audio generation (all generation is dev-time only).
- Art/visual asset generation (see [artgen](artgen.md)).
- Replacing hand-composed music (AI generation is a starting point).
- Real-time audio effects processing (handled by the engine's audio system).
- MIDI output or music notation.

## Open Questions

- ACE-Step LoRA fine-tuning workflow for custom world styles.
- Demucs vs. ACE-Step built-in stem separation quality comparison.
- Multi-GPU scheduling coordination with artgen.
- Whether to add a spectral matching tool that validates stems against a reference track.
- Whether to support MIDI-conditioned generation for precise musical control.

## Referenzen

- [engine/audio](../engine/audio.md) -- Runtime adaptive music playback
- [engine/positional-audio](../engine/positional-audio.md) -- 3D spatial audio
- [ai-pipelines/artgen](artgen.md) -- Art generation counterpart
- [ai-pipelines/agent-api](agent-api.md) -- MCP server architecture
