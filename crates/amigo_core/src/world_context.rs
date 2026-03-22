//! Cross-modal coherence system for AI-native content generation.
//!
//! [`WorldContext`] provides shared constraints that art, audio, and dialogue
//! generation tools consume so that independently generated assets feel
//! cohesive.  The struct is serialisable as RON and designed to live in game
//! data directories (e.g. `assets/worlds/frozen_peaks.ron`).
//!
//! See ADR-0013 for the full rationale.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level context
// ---------------------------------------------------------------------------

/// Shared context consumed by art, audio, and dialogue generation tools to
/// ensure cross-modal coherence within a game world / biome / level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldContext {
    /// Human-readable name for the world / area.
    pub name: String,

    /// Broad biome classification.
    pub biome: Biome,

    /// Intended emotional tone.
    pub mood: Mood,

    /// Historical / fictional era influencing dialogue and visuals.
    pub era: Era,

    /// Colour palette constraint for art generation.
    pub color_palette: Palette,

    /// Music style constraints for audio generation.
    pub music_style: MusicStyle,

    /// Visual style constraints for art generation.
    pub visual_style: VisualStyle,
}

// ---------------------------------------------------------------------------
// Supporting enums
// ---------------------------------------------------------------------------

/// Biome classification for world theming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    Forest,
    Desert,
    Ice,
    Volcano,
    Ocean,
    Cave,
    Sky,
    Swamp,
    Ruins,
    Urban,
    /// Escape hatch for biomes not yet enumerated.
    Custom,
}

/// Emotional mood that guides music tempo, palette warmth, and dialogue tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mood {
    Calm,
    Tense,
    Joyful,
    Melancholy,
    Epic,
    Mysterious,
    Eerie,
    Playful,
    Custom,
}

/// Historical/fictional era for dialogue and visual style hints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Era {
    Medieval,
    SciFi,
    Modern,
    Fantasy,
    Steampunk,
    PostApocalyptic,
    Custom,
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// A colour palette expressed as hex strings (e.g. `"#1a1a2e"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub danger: String,
}

// ---------------------------------------------------------------------------
// Music style
// ---------------------------------------------------------------------------

/// Constraints forwarded to audio generation tools (ACE-Step / AudioGen).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicStyle {
    pub genre: MusicGenre,
    /// Beats-per-minute range `(min, max)`.
    pub tempo_range: (u16, u16),
    pub key: MusicKey,
    /// Suggested instrument names (free-form strings understood by the model).
    pub instruments: Vec<String>,
}

/// Broad music genre classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicGenre {
    Orchestral,
    Chiptune,
    Electronic,
    Ambient,
    Rock,
    Jazz,
    Folk,
    Custom,
}

/// Musical key (major / minor).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicKey {
    Major,
    Minor,
}

// ---------------------------------------------------------------------------
// Visual style
// ---------------------------------------------------------------------------

/// Constraints forwarded to art generation tools (ComfyUI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisualStyle {
    pub art_type: ArtType,
    /// Tile size in pixels (e.g. 16, 32).
    pub tile_size: u32,
    pub lighting: Lighting,
    pub weather: Weather,
}

/// Art style classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtType {
    PixelArt,
    HandDrawn,
    Vector,
    Custom,
}

/// Ambient lighting level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lighting {
    Bright,
    Normal,
    Dim,
    Dark,
}

/// Weather overlay hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weather {
    Clear,
    Rain,
    Snow,
    Fog,
    Sandstorm,
    None,
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl Default for WorldContext {
    fn default() -> Self {
        Self {
            name: "Untitled World".into(),
            biome: Biome::Forest,
            mood: Mood::Calm,
            era: Era::Fantasy,
            color_palette: Palette {
                primary: "#2d5a27".into(),
                secondary: "#4a8c3f".into(),
                accent: "#f0e68c".into(),
                danger: "#ff4444".into(),
            },
            music_style: MusicStyle {
                genre: MusicGenre::Orchestral,
                tempo_range: (90, 120),
                key: MusicKey::Major,
                instruments: vec!["strings".into(), "flute".into(), "harp".into()],
            },
            visual_style: VisualStyle {
                art_type: ArtType::PixelArt,
                tile_size: 16,
                lighting: Lighting::Normal,
                weather: Weather::Clear,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ron() {
        let ctx = WorldContext {
            name: "Frozen Peaks".into(),
            biome: Biome::Ice,
            mood: Mood::Tense,
            era: Era::Medieval,
            color_palette: Palette {
                primary: "#1a1a2e".into(),
                secondary: "#4a6fa5".into(),
                accent: "#c4d7e0".into(),
                danger: "#ff4444".into(),
            },
            music_style: MusicStyle {
                genre: MusicGenre::Orchestral,
                tempo_range: (80, 110),
                key: MusicKey::Minor,
                instruments: vec![
                    "strings".into(),
                    "choir".into(),
                    "timpani".into(),
                ],
            },
            visual_style: VisualStyle {
                art_type: ArtType::PixelArt,
                tile_size: 16,
                lighting: Lighting::Dim,
                weather: Weather::Snow,
            },
        };

        let ron_str = ron::ser::to_string_pretty(&ctx, Default::default()).unwrap();
        let parsed: WorldContext = ron::from_str(&ron_str).unwrap();
        assert_eq!(ctx, parsed);
    }

    #[test]
    fn default_is_valid() {
        let ctx = WorldContext::default();
        assert_eq!(ctx.biome, Biome::Forest);
        assert_eq!(ctx.visual_style.tile_size, 16);
    }

    #[test]
    fn json_round_trip() {
        let ctx = WorldContext::default();
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: WorldContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, parsed);
    }
}
