//! What the editor shows of `light` and `emitter` entities while it is open:
//! their lights and their particles, live, from the entities' properties.
//!
//! A `light` entity reads `radius` (48), `color` (`#rrggbb`, white),
//! `intensity` (1) and `falloff` (1). An `emitter` entity starts from its
//! `preset` (`explosion`, `smoke`, `sparkle`, `trail`, or the default) and
//! overrides any `EmitterConfig` field named in its properties; colours are
//! `#rrggbb` or `#rrggbbaa`, `blend` is `normal`, `additive` or `multiply`.

use amigo_core::Color;
use amigo_editor::{EditorSession, EntityPlacement};
use amigo_render::BlendMode;
use amigo_render::lighting::PointLight;
use amigo_render::particles::{EmitterConfig, EmitterShape, ParticleSystem};
use std::path::PathBuf;

/// Name prefix of the editor's preview emitters in the particle system.
const PREFIX: &str = "editor:";

fn parse<T: std::str::FromStr>(e: &EntityPlacement, key: &str) -> Option<T> {
    e.properties.get(key).and_then(|v| v.trim().parse().ok())
}

/// `#rrggbb` or `#rrggbbaa`.
pub(crate) fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(Color::from_hex(value)),
        8 => Some(Color::from_rgba(
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        )),
        _ => None,
    }
}

/// The point light a `light` entity stands for, centred on its tile.
pub(crate) fn light(e: &EntityPlacement, tile_size: f32) -> Option<PointLight> {
    (e.entity_type == "light").then(|| PointLight {
        position: (e.x + tile_size / 2.0, e.y + tile_size / 2.0),
        color: e
            .properties
            .get("color")
            .and_then(|c| parse_color(c))
            .unwrap_or(Color::WHITE),
        intensity: parse(e, "intensity").unwrap_or(1.0),
        radius: parse(e, "radius").unwrap_or(48.0),
        falloff: parse(e, "falloff").unwrap_or(1.0),
    })
}

/// The emitter configuration of an `emitter` entity.
pub(crate) fn emitter_config(e: &EntityPlacement) -> EmitterConfig {
    let mut c = match e.properties.get("preset").map(|p| p.trim()) {
        Some("explosion") => EmitterConfig::explosion(),
        Some("smoke") => EmitterConfig::smoke(),
        Some("sparkle") => EmitterConfig::sparkle(),
        Some("trail") => EmitterConfig::trail(),
        _ => EmitterConfig::default(),
    };
    macro_rules! field {
        ($($name:ident),*) => {
            $(if let Some(v) = parse(e, stringify!($name)) { c.$name = v; })*
        };
    }
    field!(
        max_particles,
        emission_rate,
        lifetime_min,
        lifetime_max,
        speed_min,
        speed_max,
        direction,
        spread,
        gravity,
        size_start,
        size_end,
        fade_out,
        burst
    );
    if let Some(color) = e.properties.get("color_start").and_then(|v| parse_color(v)) {
        c.color_start = color;
    }
    if let Some(color) = e.properties.get("color_end").and_then(|v| parse_color(v)) {
        c.color_end = color;
    }
    match e.properties.get("blend").map(|b| b.trim()) {
        Some("additive") => c.blend_mode = BlendMode::Additive,
        Some("multiply") => c.blend_mode = BlendMode::Multiply,
        Some("normal") => c.blend_mode = BlendMode::Normal,
        _ => {}
    }
    c
}

/// A name that changes whenever the entity does, so an edited emitter is
/// replaced and an unchanged one keeps its particles.
fn preview_name(index: usize, e: &EntityPlacement) -> String {
    let mut props: Vec<_> = e.properties.iter().collect();
    props.sort();
    format!("{PREFIX}{index}:{}:{}:{:?}", e.x, e.y, props)
}

/// Make the particle system's preview emitters match the session's
/// `emitter` entities; with no session (the editor closed) remove them all.
pub(crate) fn sync_emitters(particles: &mut ParticleSystem, session: Option<&EditorSession>) {
    let wanted: Vec<(String, &EntityPlacement)> = session
        .map(|s| {
            s.level
                .entities
                .iter()
                .enumerate()
                .filter(|(_, e)| e.entity_type == "emitter")
                .map(|(i, e)| (preview_name(i, e), e))
                .collect()
        })
        .unwrap_or_default();
    particles.remove_where(|n| n.starts_with(PREFIX) && !wanted.iter().any(|(w, _)| w == n));
    let ts = session.map_or(16.0, |s| s.level.tile_size as f32);
    for (name, e) in wanted {
        if !particles.contains(&name) {
            particles.spawn(
                &name,
                emitter_config(e),
                EmitterShape::Point,
                e.x + ts / 2.0,
                e.y + ts / 2.0,
            );
        }
    }
}

/// Write the emitter entity at `index` as
/// `assets/data/<name>.emitter.ron`, `<name>` being its `name` property or
/// `emitter_<index>`.
pub(crate) fn save_emitter(session: &EditorSession, index: usize) -> Result<PathBuf, String> {
    let e = session
        .level
        .entities
        .get(index)
        .filter(|e| e.entity_type == "emitter")
        .ok_or_else(|| format!("entity {index} is not an emitter"))?;
    let name = e
        .properties
        .get("name")
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty() && !n.contains(['/', '\\', '.']))
        .unwrap_or_else(|| format!("emitter_{index}"));
    let dir = session.assets_dir().join("data");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}.emitter.ron"));
    let text = ron::ser::to_string_pretty(&emitter_config(e), ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(kind: &str, props: &[(&str, &str)]) -> EntityPlacement {
        EntityPlacement {
            entity_type: kind.into(),
            x: 32.0,
            y: 16.0,
            properties: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn light_entities_become_point_lights() {
        let l = light(
            &entity("light", &[("radius", "80"), ("color", "#ff8000")]),
            16.0,
        )
        .expect("a light");
        assert_eq!(l.position, (40.0, 24.0));
        assert_eq!(l.radius, 80.0);
        assert!((l.color.g - 0.5).abs() < 0.01);
        assert!(light(&entity("spawn", &[]), 16.0).is_none());
    }

    #[test]
    fn emitter_properties_override_the_preset() {
        let c = emitter_config(&entity(
            "emitter",
            &[
                ("preset", "smoke"),
                ("emission_rate", "99"),
                ("blend", "additive"),
                ("color_start", "#00ff0080"),
            ],
        ));
        assert_eq!(c.emission_rate, 99.0);
        assert_eq!(c.blend_mode, BlendMode::Additive);
        assert!((c.color_start.a - 128.0 / 255.0).abs() < 0.01);
        assert_eq!(c.lifetime_max, EmitterConfig::smoke().lifetime_max);
    }

    #[test]
    fn preview_emitters_follow_the_entities() {
        let mut level = amigo_editor::AmigoLevel::new("t", 4, 4, 16);
        level
            .entities
            .push(entity("emitter", &[("preset", "sparkle")]));
        let mut session = EditorSession::new(level, "t.amigo");
        let mut particles = ParticleSystem::new();
        particles.spawn(
            "game",
            EmitterConfig::default(),
            EmitterShape::Point,
            0.0,
            0.0,
        );

        sync_emitters(&mut particles, Some(&session));
        assert_eq!(particles.emitter_count(), 2);
        sync_emitters(&mut particles, Some(&session));
        assert_eq!(particles.emitter_count(), 2, "an unchanged emitter is kept");

        session.level.entities[0].x = 48.0;
        sync_emitters(&mut particles, Some(&session));
        assert_eq!(particles.emitter_count(), 2, "a moved one is replaced");

        sync_emitters(&mut particles, None);
        assert_eq!(particles.names().collect::<Vec<_>>(), vec!["game"]);
    }

    #[test]
    fn an_emitter_saves_as_ron() {
        let dir = std::env::temp_dir().join(format!("amigo_emitter_{}", std::process::id()));
        let mut level = amigo_editor::AmigoLevel::new("t", 4, 4, 16);
        level
            .entities
            .push(entity("emitter", &[("name", "torch"), ("preset", "smoke")]));
        let session = EditorSession::new(level, dir.join("levels").join("t.amigo"));
        let path = save_emitter(&session, 0).expect("saves");
        assert_eq!(path, dir.join("data").join("torch.emitter.ron"));
        let text = std::fs::read_to_string(&path).expect("written");
        let back: EmitterConfig = ron::from_str(&text).expect("parses");
        assert_eq!(back.lifetime_max, EmitterConfig::smoke().lifetime_max);
        let _ = std::fs::remove_dir_all(dir);
    }
}
