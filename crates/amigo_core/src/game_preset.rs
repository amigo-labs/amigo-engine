use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Scene preset — what kind of gameplay a scene uses
// ---------------------------------------------------------------------------

/// Defines what engine systems a scene activates.
/// A single game can mix multiple presets across different scenes.
///
/// Example: WorldMap (TopDown) → Dungeon (Roguelike) → Battle (TurnBased)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScenePreset {
    /// Top-down view with free movement. Good for overworlds, towns, Zelda-style.
    TopDown,
    /// Side-scrolling with gravity, jump, dash. Mario, Celeste, Metroid.
    Platformer,
    /// Turn-based combat (Pokémon, Final Fantasy).
    TurnBased,
    /// Click-to-move with real-time combat. Diablo, Path of Exile.
    Arpg,
    /// Procedural dungeon floors with permadeath. Binding of Isaac, Spelunky.
    Roguelike,
    /// Tower defense with waves and tower placement.
    TowerDefense,
    /// Bullet hell / shmup.
    BulletHell,
    /// Arcade shooter with scrolling, power-ups, and waves. Contra, Gradius, Space Invaders.
    ArcadeShooter,
    /// Grid-based puzzle (Match-3, Tetris, Sokoban).
    Puzzle,
    /// Farming / life sim (Stardew Valley).
    FarmingSim,
    /// Fighting game (Street Fighter).
    Fighting,
    /// Visual novel / dialog-heavy scene.
    VisualNovel,
    /// Menu / UI-only scene (title screen, settings, credits).
    Menu,
    /// World map / level select.
    WorldMap,
    /// Sandbox / survival with dynamic world. Terraria, Starbound.
    Sandbox,
    /// God sim / city builder with autonomous agents. WorldBox, Dwarf Fortress, RimWorld.
    GodSim,
    /// Custom — game defines its own systems.
    Custom,
}

impl ScenePreset {
    /// Human-readable name for the editor UI.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::TopDown => "Top-Down",
            Self::Platformer => "Platformer",
            Self::TurnBased => "Turn-Based Combat",
            Self::Arpg => "Action RPG",
            Self::Roguelike => "Roguelike",
            Self::TowerDefense => "Tower Defense",
            Self::BulletHell => "Bullet Hell",
            Self::ArcadeShooter => "Arcade Shooter",
            Self::Puzzle => "Puzzle",
            Self::FarmingSim => "Farming Sim",
            Self::Fighting => "Fighting",
            Self::VisualNovel => "Visual Novel",
            Self::Menu => "Menu",
            Self::WorldMap => "World Map",
            Self::Sandbox => "Sandbox / Survival",
            Self::GodSim => "God Sim",
            Self::Custom => "Custom",
        }
    }

    /// Short description for the wizard UI.
    pub fn description(self) -> &'static str {
        match self {
            Self::TopDown => "Free movement, top-down camera. Zelda, Undertale.",
            Self::Platformer => "Side-scrolling, gravity, jumping. Mario, Celeste.",
            Self::TurnBased => "Turn-based battles. Pokemon, Final Fantasy.",
            Self::Arpg => "Click-to-move, real-time combat, loot. Diablo.",
            Self::Roguelike => "Procedural dungeons, permadeath. Spelunky, Isaac.",
            Self::TowerDefense => "Place towers, survive waves. Bloons, Kingdom Rush.",
            Self::BulletHell => "Dodge bullets, shoot patterns. Touhou, Ikaruga.",
            Self::ArcadeShooter => "Scrolling shooter with power-ups and waves. Contra, Gradius, Space Invaders.",
            Self::Puzzle => "Grid-based puzzles. Tetris, Bejeweled, Sokoban.",
            Self::FarmingSim => "Grow crops, manage a farm. Stardew Valley.",
            Self::Fighting => "Frame-based combat, combos. Street Fighter.",
            Self::VisualNovel => "Story-driven, dialog choices. Ace Attorney.",
            Self::Menu => "Title screen, settings, credits.",
            Self::WorldMap => "Level select, overworld navigation.",
            Self::Sandbox => "Dynamic world, mining, building, crafting. Terraria, Starbound.",
            Self::GodSim => "Autonomous agents, simulation, world management. WorldBox, RimWorld.",
            Self::Custom => "Start from scratch, pick your own systems.",
        }
    }

    /// Which amigo_core systems this preset activates.
    pub fn default_systems(self) -> Vec<&'static str> {
        match self {
            Self::TopDown => vec![
                "navigation",
                "ai",
                "combat",
                "collision",
                "physics",
                "dialog",
                "inventory",
            ],
            Self::Platformer => vec!["platformer", "physics", "collision"],
            Self::TurnBased => vec!["turn_combat", "inventory"],
            Self::Arpg => vec!["navigation", "ai", "combat", "loot", "inventory", "waves"],
            Self::Roguelike => vec!["roguelike", "combat", "loot", "inventory", "procgen"],
            Self::TowerDefense => vec!["tower", "waves", "navigation", "combat"],
            Self::BulletHell => vec!["bullet_pattern", "collision"],
            Self::ArcadeShooter => vec![
                "bullet_pattern",
                "projectile",
                "collision",
                "physics",
                "economy",
                "waves",
                "combat",
            ],
            Self::Puzzle => vec!["puzzle"],
            Self::FarmingSim => vec!["farming", "inventory", "crafting", "dialog"],
            Self::Fighting => vec!["fighting"],
            Self::VisualNovel => vec!["dialog"],
            Self::Menu => vec![],
            Self::WorldMap => vec!["navigation"],
            Self::Sandbox => vec![
                "inventory",
                "crafting",
                "physics",
                "collision",
                "procgen",
                "navigation",
                "combat",
                "projectile",
                "loot",
            ],
            Self::GodSim => vec![
                "ai",
                "navigation",
                "procgen",
                "economy",
                "combat",
                "collision",
            ],
            Self::Custom => vec![],
        }
    }

    /// All available presets.
    pub fn all() -> &'static [ScenePreset] {
        &[
            Self::TopDown,
            Self::Platformer,
            Self::TurnBased,
            Self::Arpg,
            Self::Roguelike,
            Self::TowerDefense,
            Self::BulletHell,
            Self::ArcadeShooter,
            Self::Puzzle,
            Self::FarmingSim,
            Self::Fighting,
            Self::VisualNovel,
            Self::Menu,
            Self::WorldMap,
            Self::Sandbox,
            Self::GodSim,
            Self::Custom,
        ]
    }

    /// Presets suitable as "main gameplay" (excludes Menu, WorldMap, Custom).
    pub fn gameplay_presets() -> &'static [ScenePreset] {
        &[
            Self::TopDown,
            Self::Platformer,
            Self::TurnBased,
            Self::Arpg,
            Self::Roguelike,
            Self::TowerDefense,
            Self::BulletHell,
            Self::ArcadeShooter,
            Self::Puzzle,
            Self::FarmingSim,
            Self::Fighting,
            Self::VisualNovel,
            Self::Sandbox,
            Self::GodSim,
        ]
    }
}

// ---------------------------------------------------------------------------
// Scene definition — one entry in the project's scene graph
// ---------------------------------------------------------------------------

/// Defines a single scene within a game project.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneDef {
    /// Unique scene identifier (e.g. "title_menu", "world_1", "dungeon_floor").
    pub id: String,
    /// Human-readable name shown in editor.
    pub name: String,
    /// Which preset governs this scene's default systems.
    pub preset: ScenePreset,
    /// Path to the .amigo level file (if applicable).
    pub level_file: Option<String>,
    /// Scenes this scene can transition to.
    pub transitions: Vec<SceneTransition>,
    /// Custom key-value properties.
    pub properties: HashMap<String, String>,
}

impl SceneDef {
    pub fn new(id: impl Into<String>, name: impl Into<String>, preset: ScenePreset) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            preset,
            level_file: None,
            transitions: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_level(mut self, path: impl Into<String>) -> Self {
        self.level_file = Some(path.into());
        self
    }

    pub fn with_transition(mut self, transition: SceneTransition) -> Self {
        self.transitions.push(transition);
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// How one scene connects to another.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneTransition {
    /// Target scene ID.
    pub target: String,
    /// What triggers this transition.
    pub trigger: TransitionTrigger,
    /// Visual transition effect name (e.g. "fade", "slide_left", "cut").
    pub effect: String,
}

/// What triggers a scene transition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransitionTrigger {
    /// Player enters a specific zone/door.
    Zone { x: f32, y: f32, w: f32, h: f32 },
    /// A dialog choice or event.
    Event { event_name: String },
    /// Menu button press.
    Button { label: String },
    /// Level completed.
    LevelComplete,
    /// Player died.
    GameOver,
    /// Manual (triggered from code).
    Manual,
}

// ---------------------------------------------------------------------------
// Game project — the full project definition
// ---------------------------------------------------------------------------

/// A complete game project definition. Saved as `amigo_project.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameProject {
    /// Project name.
    pub name: String,
    /// Project version.
    pub version: String,
    /// Virtual resolution width.
    pub virtual_width: u32,
    /// Virtual resolution height.
    pub virtual_height: u32,
    /// ID of the first scene to show on launch.
    pub start_scene: String,
    /// All scene definitions.
    pub scenes: Vec<SceneDef>,
    /// Global project properties.
    pub properties: HashMap<String, String>,
}

impl GameProject {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
            virtual_width: 320,
            virtual_height: 180,
            start_scene: "title_menu".to_string(),
            scenes: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_resolution(mut self, w: u32, h: u32) -> Self {
        self.virtual_width = w;
        self.virtual_height = h;
        self
    }

    pub fn with_start_scene(mut self, id: impl Into<String>) -> Self {
        self.start_scene = id.into();
        self
    }

    pub fn add_scene(&mut self, scene: SceneDef) {
        self.scenes.push(scene);
    }

    pub fn find_scene(&self, id: &str) -> Option<&SceneDef> {
        self.scenes.iter().find(|s| s.id == id)
    }

    pub fn find_scene_mut(&mut self, id: &str) -> Option<&mut SceneDef> {
        self.scenes.iter_mut().find(|s| s.id == id)
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    /// Validate the project. Returns a list of issues.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.name.is_empty() {
            issues.push("Project name is empty".to_string());
        }

        if self.scenes.is_empty() {
            issues.push("Project has no scenes".to_string());
        }

        if self.find_scene(&self.start_scene).is_none() && !self.scenes.is_empty() {
            issues.push(format!("Start scene '{}' not found", self.start_scene));
        }

        // Check transition targets exist
        let scene_ids: Vec<&str> = self.scenes.iter().map(|s| s.id.as_str()).collect();
        for scene in &self.scenes {
            for t in &scene.transitions {
                if !scene_ids.contains(&t.target.as_str()) {
                    issues.push(format!(
                        "Scene '{}' has transition to unknown scene '{}'",
                        scene.id, t.target
                    ));
                }
            }
        }

        issues
    }
}

// ---------------------------------------------------------------------------
// Project templates — quick-start presets for new projects
// ---------------------------------------------------------------------------

/// A project template that the wizard uses to scaffold a new project.
#[derive(Clone, Debug)]
pub struct ProjectTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub primary_preset: ScenePreset,
    pub resolution: (u32, u32),
}

impl ProjectTemplate {
    /// Generate a GameProject from this template.
    pub fn create_project(&self, project_name: &str) -> GameProject {
        let mut project =
            GameProject::new(project_name).with_resolution(self.resolution.0, self.resolution.1);

        // Every project gets a title menu
        let mut title = SceneDef::new("title_menu", "Title Menu", ScenePreset::Menu);
        title.transitions.push(SceneTransition {
            target: "gameplay".to_string(),
            trigger: TransitionTrigger::Button {
                label: "Start Game".to_string(),
            },
            effect: "fade".to_string(),
        });
        project.add_scene(title);

        // Main gameplay scene
        let gameplay = SceneDef::new("gameplay", "Gameplay", self.primary_preset)
            .with_level("levels/level_01.amigo".to_string());
        project.add_scene(gameplay);

        // Pause menu
        let mut pause = SceneDef::new("pause_menu", "Pause Menu", ScenePreset::Menu);
        pause.transitions.push(SceneTransition {
            target: "title_menu".to_string(),
            trigger: TransitionTrigger::Button {
                label: "Quit to Title".to_string(),
            },
            effect: "fade".to_string(),
        });
        project.add_scene(pause);

        project
    }
}

/// All available project templates.
pub fn project_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate {
            name: "Platformer",
            description: "Side-scrolling platformer with jumping and dashing.",
            primary_preset: ScenePreset::Platformer,
            resolution: (320, 180),
        },
        ProjectTemplate {
            name: "Top-Down Adventure",
            description: "Top-down exploration with NPCs and combat.",
            primary_preset: ScenePreset::TopDown,
            resolution: (320, 180),
        },
        ProjectTemplate {
            name: "Action RPG",
            description: "Click-to-move, monsters, loot, skills.",
            primary_preset: ScenePreset::Arpg,
            resolution: (480, 270),
        },
        ProjectTemplate {
            name: "Roguelike",
            description: "Procedural dungeons, permadeath, item runs.",
            primary_preset: ScenePreset::Roguelike,
            resolution: (320, 180),
        },
        ProjectTemplate {
            name: "Turn-Based RPG",
            description: "Overworld exploration with turn-based battles.",
            primary_preset: ScenePreset::TopDown,
            resolution: (320, 180),
        },
        ProjectTemplate {
            name: "Tower Defense",
            description: "Place towers, survive enemy waves.",
            primary_preset: ScenePreset::TowerDefense,
            resolution: (480, 270),
        },
        ProjectTemplate {
            name: "Puzzle Game",
            description: "Grid-based puzzle (Match-3, Tetris, Sokoban).",
            primary_preset: ScenePreset::Puzzle,
            resolution: (240, 320),
        },
        ProjectTemplate {
            name: "Farming Sim",
            description: "Grow crops, talk to NPCs, craft items.",
            primary_preset: ScenePreset::FarmingSim,
            resolution: (320, 180),
        },
        ProjectTemplate {
            name: "Bullet Hell",
            description: "Dodge and shoot, boss patterns.",
            primary_preset: ScenePreset::BulletHell,
            resolution: (240, 320),
        },
        ProjectTemplate {
            name: "Arcade Shooter",
            description: "Scrolling shooter with power-ups, waves, and high scores.",
            primary_preset: ScenePreset::ArcadeShooter,
            resolution: (320, 240),
        },
        ProjectTemplate {
            name: "Visual Novel",
            description: "Story-driven with dialog choices.",
            primary_preset: ScenePreset::VisualNovel,
            resolution: (480, 270),
        },
        ProjectTemplate {
            name: "Sandbox / Survival",
            description: "Dynamic world with mining, building, crafting, and exploration.",
            primary_preset: ScenePreset::Sandbox,
            resolution: (480, 270),
        },
        ProjectTemplate {
            name: "God Sim",
            description: "Autonomous agents, settlements, simulation speed control.",
            primary_preset: ScenePreset::GodSim,
            resolution: (480, 270),
        },
        ProjectTemplate {
            name: "Custom",
            description: "Empty project. Pick your own systems.",
            primary_preset: ScenePreset::Custom,
            resolution: (320, 180),
        },
    ]
}

// ---------------------------------------------------------------------------
// Save / Load project
// ---------------------------------------------------------------------------

/// Serialize a project to RON and write to disk.
pub fn save_project(path: &std::path::Path, project: &GameProject) -> Result<(), std::io::Error> {
    let ron_string = ron::ser::to_string_pretty(project, ron::ser::PrettyConfig::default())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(path, ron_string)
}

/// Load a project from a RON file.
pub fn load_project(path: &std::path::Path) -> Result<GameProject, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    ron::from_str(&contents).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_metadata() {
        for preset in ScenePreset::all() {
            assert!(!preset.display_name().is_empty());
            assert!(!preset.description().is_empty());
        }
    }

    #[test]
    fn preset_systems() {
        let systems = ScenePreset::Platformer.default_systems();
        assert!(systems.contains(&"platformer"));
        assert!(systems.contains(&"physics"));

        let systems = ScenePreset::Arpg.default_systems();
        assert!(systems.contains(&"navigation"));
        assert!(systems.contains(&"combat"));
        assert!(systems.contains(&"loot"));
    }

    #[test]
    fn scene_def_builder() {
        let scene = SceneDef::new("dungeon", "Dark Dungeon", ScenePreset::Roguelike)
            .with_level("levels/dungeon.amigo")
            .with_property("difficulty", "hard");

        assert_eq!(scene.id, "dungeon");
        assert_eq!(scene.preset, ScenePreset::Roguelike);
        assert_eq!(scene.level_file.as_deref(), Some("levels/dungeon.amigo"));
        assert_eq!(scene.properties.get("difficulty").unwrap(), "hard");
    }

    #[test]
    fn game_project_basics() {
        let mut project = GameProject::new("Test Game").with_resolution(320, 180);
        project.add_scene(SceneDef::new("menu", "Menu", ScenePreset::Menu));
        project.add_scene(SceneDef::new("game", "Game", ScenePreset::Platformer));
        project.start_scene = "menu".to_string();

        assert_eq!(project.scene_count(), 2);
        assert!(project.find_scene("menu").is_some());
        assert!(project.find_scene("game").is_some());
        assert!(project.find_scene("nonexistent").is_none());
    }

    #[test]
    fn project_validation_ok() {
        let mut project = GameProject::new("Good Project");
        project.add_scene(SceneDef::new("title_menu", "Title", ScenePreset::Menu));
        let issues = project.validate();
        assert!(issues.is_empty(), "Should be valid: {:?}", issues);
    }

    #[test]
    fn project_validation_missing_start() {
        let mut project = GameProject::new("Bad Project");
        project.start_scene = "nonexistent".to_string();
        project.add_scene(SceneDef::new("menu", "Menu", ScenePreset::Menu));
        let issues = project.validate();
        assert!(issues.iter().any(|i| i.contains("Start scene")));
    }

    #[test]
    fn project_validation_bad_transition() {
        let mut project = GameProject::new("Bad Trans");
        let scene =
            SceneDef::new("menu", "Menu", ScenePreset::Menu).with_transition(SceneTransition {
                target: "nowhere".to_string(),
                trigger: TransitionTrigger::Manual,
                effect: "fade".to_string(),
            });
        project.add_scene(scene);
        project.start_scene = "menu".to_string();
        let issues = project.validate();
        assert!(issues.iter().any(|i| i.contains("nowhere")));
    }

    #[test]
    fn template_creates_valid_project() {
        let templates = project_templates();
        for template in &templates {
            let project = template.create_project("My Game");
            assert_eq!(project.name, "My Game");
            assert!(
                project.scene_count() >= 2,
                "Template '{}' should create at least 2 scenes",
                template.name
            );
            assert!(project.find_scene("title_menu").is_some());
            assert!(project.find_scene("gameplay").is_some());
        }
    }

    #[test]
    fn mixed_presets_in_project() {
        // This is the key feature: one project, multiple game types
        let mut project = GameProject::new("Mixed RPG");

        project.add_scene(SceneDef::new("title", "Title", ScenePreset::Menu));
        project.add_scene(
            SceneDef::new("worldmap", "World Map", ScenePreset::WorldMap)
                .with_level("levels/overworld.amigo"),
        );
        project.add_scene(
            SceneDef::new("town", "Town", ScenePreset::TopDown).with_level("levels/town.amigo"),
        );
        project.add_scene(SceneDef::new("dungeon", "Dungeon", ScenePreset::Roguelike));
        project.add_scene(SceneDef::new("battle", "Battle", ScenePreset::TurnBased));
        project.add_scene(SceneDef::new("shop", "Shop", ScenePreset::VisualNovel));

        project.start_scene = "title".to_string();

        assert_eq!(project.scene_count(), 6);

        // Each scene has its own preset/systems
        assert_eq!(
            project.find_scene("worldmap").unwrap().preset,
            ScenePreset::WorldMap
        );
        assert_eq!(
            project.find_scene("dungeon").unwrap().preset,
            ScenePreset::Roguelike
        );
        assert_eq!(
            project.find_scene("battle").unwrap().preset,
            ScenePreset::TurnBased
        );

        let issues = project.validate();
        assert!(issues.is_empty());
    }

    #[test]
    fn project_serialization_roundtrip() {
        let mut project = GameProject::new("Serialize Test");
        project.add_scene(SceneDef::new("menu", "Menu", ScenePreset::Menu));
        project.add_scene(
            SceneDef::new("game", "Game", ScenePreset::Platformer)
                .with_level("levels/test.amigo")
                .with_property("music", "theme.ogg"),
        );
        project.start_scene = "menu".to_string();

        // Serialize to RON string
        let ron = ron::ser::to_string_pretty(&project, ron::ser::PrettyConfig::default()).unwrap();
        // Deserialize back
        let loaded: GameProject = ron::from_str(&ron).unwrap();

        assert_eq!(loaded.name, "Serialize Test");
        assert_eq!(loaded.scene_count(), 2);
        assert_eq!(
            loaded.find_scene("game").unwrap().level_file.as_deref(),
            Some("levels/test.amigo")
        );
    }
}
