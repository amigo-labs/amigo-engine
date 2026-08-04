//! Source files `amigo new` writes into a fresh project.
//!
//! Before this module, every template produced the same hardcoded `main.rs`: an
//! empty `Game` impl drawing one blue rectangle. `--template tower-defense` and
//! `--template visual-novel` differed only in the virtual resolution and in
//! `[[scenes]]` rows that nothing read back. `src/scenes/` was created and left
//! empty, and the generated `assets/levels/level_01.amigo` was never loaded.
//!
//! Every template now gets a playable skeleton: a title menu that pushes
//! gameplay, gameplay that can be paused with an overlay, and a pause menu that
//! either resumes or returns to the title — all through the engine's scene stack,
//! with a HUD through the Pixel UI.
//!
//! The gameplay body is chosen per [`Family`], not per template. Twenty-one scene
//! presets do not need twenty-one different movement models, and a shared body
//! that is right beats a bespoke one that only compiles. Where a preset has a
//! matching `amigo_core` module with more depth than a starter needs, the
//! generated code points at it rather than pretending to wrap it.

use amigo_core::game_preset::ScenePreset;

/// Which gameplay skeleton a preset gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// UI only — no world, no player.
    Menu,
    /// Gravity, jump, coyote time, via `amigo_core::platformer`.
    Platformer,
    /// Eight-way movement over a tile grid with wall collision.
    FreeRoam,
    /// One grid cell per key press.
    GridStep,
    /// A ship that shoots upward, with bullets.
    Shooter,
    /// Turn/round loop driven from the UI rather than from movement.
    TurnLoop,
    /// A counter that grows on its own, plus an upgrade.
    Idle,
    /// Place towers on a grid, run waves.
    TowerDefense,
}

impl Family {
    /// The family a preset belongs to.
    pub fn of(preset: ScenePreset) -> Self {
        match preset {
            ScenePreset::Menu => Family::Menu,
            ScenePreset::Platformer => Family::Platformer,
            ScenePreset::TopDown
            | ScenePreset::Arpg
            | ScenePreset::Sandbox
            | ScenePreset::GodSim
            | ScenePreset::WorldMap
            | ScenePreset::FarmingSim
            | ScenePreset::Custom => Family::FreeRoam,
            ScenePreset::Puzzle | ScenePreset::Roguelike => Family::GridStep,
            ScenePreset::BulletHell | ScenePreset::ArcadeShooter => Family::Shooter,
            ScenePreset::TurnBased
            | ScenePreset::Fighting
            | ScenePreset::Deckbuilder
            | ScenePreset::AutoBattler
            | ScenePreset::SocialDeduction
            | ScenePreset::VisualNovel => Family::TurnLoop,
            ScenePreset::Idle => Family::Idle,
            ScenePreset::TowerDefense => Family::TowerDefense,
        }
    }

    /// The `amigo_core` module a game of this kind will want next, if any.
    fn core_module(preset: ScenePreset) -> Option<&'static str> {
        Some(match preset {
            ScenePreset::Platformer => "amigo_core::platformer",
            ScenePreset::Roguelike => "amigo_core::roguelike and amigo_core::procgen",
            ScenePreset::Puzzle => "amigo_core::puzzle",
            ScenePreset::TowerDefense => "amigo_core::tower, ::waves and ::td_systems",
            ScenePreset::BulletHell => "amigo_core::bullet_pattern and ::shmup",
            ScenePreset::ArcadeShooter => "amigo_core::shmup",
            ScenePreset::TurnBased => "amigo_core::turn_combat",
            ScenePreset::Fighting => "amigo_core::fighting",
            ScenePreset::Deckbuilder => "amigo_core::deckbuilder and ::card",
            ScenePreset::AutoBattler => "amigo_core::auto_battler",
            ScenePreset::SocialDeduction => "amigo_core::social_deduction and ::voting",
            ScenePreset::VisualNovel => "amigo_core::visual_novel and ::dialog",
            ScenePreset::Idle => "amigo_core::idle",
            ScenePreset::FarmingSim => "amigo_core::farming and ::crafting",
            ScenePreset::Arpg => "amigo_core::combat and ::loot",
            ScenePreset::Sandbox => "amigo_core::procgen, ::inventory and ::crafting",
            ScenePreset::GodSim => "amigo_core::agents and ::economy",
            ScenePreset::TopDown => "amigo_core::dialog and ::inventory",
            ScenePreset::WorldMap => "amigo_core::navigation",
            ScenePreset::Menu | ScenePreset::Custom => return None,
        })
    }
}

/// A generated file: path relative to the project root, plus contents.
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

/// All source files for a new project.
pub fn project_files(
    preset: ScenePreset,
    project_name: &str,
    virtual_width: u32,
    virtual_height: u32,
) -> Vec<GeneratedFile> {
    let family = Family::of(preset);
    let mut files = vec![
        GeneratedFile {
            path: "src/main.rs".to_string(),
            contents: main_rs(project_name, virtual_width, virtual_height),
        },
        GeneratedFile {
            path: "src/scenes/mod.rs".to_string(),
            contents: scenes_mod_rs(family),
        },
        GeneratedFile {
            path: "src/scenes/title_menu.rs".to_string(),
            contents: title_menu_rs(project_name),
        },
    ];

    // A UI-only project has nothing to pause.
    if family != Family::Menu {
        files.push(GeneratedFile {
            path: "src/scenes/pause_menu.rs".to_string(),
            contents: pause_menu_rs(),
        });
        files.push(GeneratedFile {
            path: "src/scenes/gameplay.rs".to_string(),
            contents: gameplay_rs(preset, family),
        });
    }

    files
}

// ---------------------------------------------------------------------------
// Shared scaffolding
// ---------------------------------------------------------------------------

fn main_rs(name: &str, vw: u32, vh: u32) -> String {
    format!(
        r#"use amigo_engine::prelude::*;

mod scenes;

fn main() {{
    // Each scene is its own `Game`. The title menu pushes gameplay, gameplay
    // pushes the pause overlay, and popping unwinds back — the engine owns the
    // stack, so there is no state enum to maintain.
    Engine::build()
        .title("{name}")
        .virtual_resolution({vw}, {vh})
        .window_size(1280, 720)
        .build()
        .run(scenes::TitleMenu::new());
}}
"#
    )
}

fn scenes_mod_rs(family: Family) -> String {
    if family == Family::Menu {
        return "mod title_menu;\n\npub use title_menu::TitleMenu;\n".to_string();
    }
    "mod gameplay;\nmod pause_menu;\nmod title_menu;\n\npub use gameplay::Gameplay;\npub use pause_menu::PauseMenu;\npub use title_menu::TitleMenu;\n"
        .to_string()
}

fn title_menu_rs(name: &str) -> String {
    format!(
        r#"use amigo_engine::prelude::*;

/// Title screen. Waits for Enter or Space, then pushes gameplay.
pub struct TitleMenu {{
    blink: f32,
}}

impl TitleMenu {{
    pub fn new() -> Self {{
        Self {{ blink: 0.0 }}
    }}
}}

impl Default for TitleMenu {{
    fn default() -> Self {{
        Self::new()
    }}
}}

impl Game for TitleMenu {{
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {{
        self.blink += ctx.time.dt;

        if ctx.input.pressed(KeyCode::Escape) {{
            return SceneAction::Quit;
        }}
        if ctx.input.pressed(KeyCode::Enter) || ctx.input.pressed(KeyCode::Space) {{
            // Push, not Replace: gameplay can pop back to this same menu.
            return SceneAction::Push(Box::new(|| {{
                Box::new(crate::scenes::Gameplay::new()) as Box<dyn Game>
            }}));
        }}
        SceneAction::Continue
    }}

    fn draw(&self, ctx: &mut DrawContext) {{
        let (vw, vh) = (ctx.virtual_width, ctx.virtual_height);
        ctx.draw_rect(Rect::new(0.0, 0.0, vw, vh), Color::rgb(0.06, 0.06, 0.10));

        let title = "{name}";
        let (tw, _) = ctx.measure_text(title);
        ctx.draw_text(title, (vw - tw) * 0.5, vh * 0.35, Color::rgb(0.95, 0.9, 0.75));

        // Blink the prompt once a second.
        if self.blink % 1.0 < 0.6 {{
            let prompt = "Press ENTER to start";
            let (pw, _) = ctx.measure_text(prompt);
            ctx.draw_text(prompt, (vw - pw) * 0.5, vh * 0.55, Color::rgb(0.6, 0.6, 0.55));
        }}

        let hint = "ESC quits";
        let (hw, _) = ctx.measure_text(hint);
        ctx.draw_text(hint, (vw - hw) * 0.5, vh - 16.0, Color::rgb(0.35, 0.35, 0.35));
    }}
}}
"#
    )
}

fn pause_menu_rs() -> String {
    r#"use amigo_engine::prelude::*;

/// Pause overlay. Pushed on top of gameplay, so gameplay keeps its state and is
/// simply not updated while this is the active scene.
pub struct PauseMenu;

impl PauseMenu {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for PauseMenu {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let (vw, vh) = (ctx.camera.virtual_width, ctx.camera.virtual_height);
        // Dim the scene behind the menu. Drawn through the UI so it lands in the
        // screen-space pass rather than the world.
        ctx.ui
            .panel(Rect::new(0.0, 0.0, vw, vh), Color::BLACK.with_alpha(0.6));
        ctx.ui.pixel_text("PAUSED", vw * 0.5 - 18.0, vh * 0.4, Color::WHITE);
        ctx.ui.pixel_text(
            "ESC resume    Q quit to title",
            vw * 0.5 - 78.0,
            vh * 0.5,
            Color::rgb(0.7, 0.7, 0.7),
        );

        if ctx.input.pressed(KeyCode::Escape) {
            // Pop: gameplay resumes exactly where it was.
            return SceneAction::Pop;
        }
        if ctx.input.pressed(KeyCode::KeyQ) {
            // Replace the whole stack contents above the title by popping twice:
            // this pops the pause menu, and gameplay sees `Pop` next tick via its
            // own quit flag. Simpler here: replace with a fresh title menu.
            return SceneAction::Replace(Box::new(|| {
                Box::new(crate::scenes::TitleMenu::new()) as Box<dyn Game>
            }));
        }
        SceneAction::Continue
    }

    fn draw(&self, _ctx: &mut DrawContext) {
        // Nothing world-space: the overlay is UI. Gameplay below is not drawn
        // either, since only the top of the stack draws — see `Gameplay::draw`
        // if you want the scene visible behind the menu.
    }
}
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Per-family gameplay
// ---------------------------------------------------------------------------

fn gameplay_rs(preset: ScenePreset, family: Family) -> String {
    let next_step = match Family::core_module(preset) {
        Some(module) => format!(
            "//! Next step for this game type: {module}.\n\
             //! Those modules carry the deeper systems (state machines, balance data,\n\
             //! progression) that a starter deliberately leaves out.\n"
        ),
        None => String::new(),
    };

    let body = match family {
        Family::Menu => unreachable!("the Menu family has no gameplay scene"),
        Family::Platformer => PLATFORMER_BODY,
        Family::FreeRoam => FREE_ROAM_BODY,
        Family::GridStep => GRID_STEP_BODY,
        Family::Shooter => SHOOTER_BODY,
        Family::TurnLoop => TURN_LOOP_BODY,
        Family::Idle => IDLE_BODY,
        Family::TowerDefense => TOWER_DEFENSE_BODY,
    };

    format!("{next_step}{}", with_pause(body))
}

/// Shared prologue: pause handling, common to every gameplay body.
const PAUSE_SNIPPET: &str = r#"        if ctx.input.pressed(KeyCode::Escape) {
            return SceneAction::Push(Box::new(|| {
                Box::new(crate::scenes::PauseMenu::new()) as Box<dyn Game>
            }));
        }
"#;

const FREE_ROAM_BODY: &str = r#"use amigo_engine::prelude::*;

/// Eight-way movement over a tile grid, with walls that block.
pub struct Gameplay {
    x: f32,
    y: f32,
    speed: f32,
    /// Flat tile grid: 0 = floor, 1 = wall.
    tiles: Vec<u8>,
    map_w: usize,
    map_h: usize,
}

const TILE: f32 = 16.0;

impl Gameplay {
    pub fn new() -> Self {
        let (map_w, map_h) = (24, 16);
        let mut tiles = vec![0u8; map_w * map_h];
        // Border walls, plus a couple of blocks to bump into.
        for y in 0..map_h {
            for x in 0..map_w {
                let edge = x == 0 || y == 0 || x == map_w - 1 || y == map_h - 1;
                if edge || ((6..10).contains(&x) && (5..7).contains(&y)) {
                    tiles[y * map_w + x] = 1;
                }
            }
        }
        Self {
            x: TILE * 2.0,
            y: TILE * 2.0,
            speed: 70.0,
            tiles,
            map_w,
            map_h,
        }
    }

    /// True if a 12x12 body at (x, y) overlaps a wall.
    fn blocked(&self, x: f32, y: f32) -> bool {
        for (ox, oy) in [(2.0, 2.0), (13.0, 2.0), (2.0, 13.0), (13.0, 13.0)] {
            let tx = ((x + ox) / TILE) as isize;
            let ty = ((y + oy) / TILE) as isize;
            if tx < 0 || ty < 0 || tx as usize >= self.map_w || ty as usize >= self.map_h {
                return true;
            }
            if self.tiles[ty as usize * self.map_w + tx as usize] == 1 {
                return true;
            }
        }
        false
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        // Typed: `0.0` alone is an ambiguous float literal, and `sqrt` below
        // cannot be called on one.
        let mut dx: f32 = 0.0;
        let mut dy: f32 = 0.0;
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) {
            dx -= 1.0;
        }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) {
            dx += 1.0;
        }
        if ctx.input.held(KeyCode::KeyW) || ctx.input.held(KeyCode::ArrowUp) {
            dy -= 1.0;
        }
        if ctx.input.held(KeyCode::KeyS) || ctx.input.held(KeyCode::ArrowDown) {
            dy += 1.0;
        }
        // Normalize so diagonals are not faster.
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            dx /= len;
            dy /= len;
        }

        let step = self.speed * ctx.time.dt;
        // Axis-separated so sliding along a wall works.
        let nx = self.x + dx * step;
        if !self.blocked(nx, self.y) {
            self.x = nx;
        }
        let ny = self.y + dy * step;
        if !self.blocked(self.x, ny) {
            self.y = ny;
        }

        ctx.camera.set_target(RenderVec2 {
            x: self.x,
            y: self.y,
        });

        let vh = ctx.camera.virtual_height;
        ctx.ui.pixel_text("WASD move    ESC pause", 4.0, vh - 10.0, Color::rgb(0.7, 0.7, 0.6));

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.1, 0.11, 0.13),
        );
        for y in 0..self.map_h {
            for x in 0..self.map_w {
                let color = if self.tiles[y * self.map_w + x] == 1 {
                    Color::rgb(0.35, 0.33, 0.30)
                } else {
                    Color::rgb(0.18, 0.24, 0.18)
                };
                ctx.draw_rect(
                    Rect::new(x as f32 * TILE, y as f32 * TILE, TILE, TILE),
                    color,
                );
            }
        }
        ctx.draw_rect(
            Rect::new(self.x + 2.0, self.y + 2.0, 12.0, 12.0),
            Color::rgb(0.35, 0.7, 1.0),
        );
    }
}
"#;

const PLATFORMER_BODY: &str = r#"use amigo_engine::amigo_core::platformer::{PlatformerConfig, PlatformerController, PlatformerInput};
use amigo_engine::prelude::*;

/// Side-on movement driven by `amigo_core`'s platformer controller, which brings
/// coyote time, jump buffering and variable jump height with it.
pub struct Gameplay {
    controller: PlatformerController,
    x: f32,
    y: f32,
    /// Flat tile grid: 0 = air, 1 = solid.
    tiles: Vec<u8>,
    map_w: usize,
    map_h: usize,
}

const TILE: f32 = 16.0;
const GRAVITY: f32 = 0.5;

impl Gameplay {
    pub fn new() -> Self {
        let (map_w, map_h) = (30, 14);
        let mut tiles = vec![0u8; map_w * map_h];
        for x in 0..map_w {
            tiles[(map_h - 1) * map_w + x] = 1; // floor
        }
        // A couple of platforms to jump between.
        for x in 6..11 {
            tiles[(map_h - 5) * map_w + x] = 1;
        }
        for x in 15..21 {
            tiles[(map_h - 8) * map_w + x] = 1;
        }
        Self {
            controller: PlatformerController::new(PlatformerConfig::default()),
            x: TILE * 2.0,
            y: TILE * (map_h as f32 - 3.0),
            tiles,
            map_w,
            map_h,
        }
    }

    fn solid_at(&self, x: f32, y: f32) -> bool {
        let tx = (x / TILE) as isize;
        let ty = (y / TILE) as isize;
        if tx < 0 || ty < 0 || tx as usize >= self.map_w || ty as usize >= self.map_h {
            return ty as usize >= self.map_h; // out the bottom counts as ground
        }
        self.tiles[ty as usize * self.map_w + tx as usize] == 1
    }

    fn on_ground(&self) -> bool {
        self.solid_at(self.x + 4.0, self.y + 17.0) || self.solid_at(self.x + 11.0, self.y + 17.0)
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        let mut move_x = 0.0;
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) {
            move_x -= 1.0;
        }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) {
            move_x += 1.0;
        }

        let grounded = self.on_ground();
        let input = PlatformerInput {
            move_x,
            jump_pressed: ctx.input.pressed(KeyCode::Space),
            jump_held: ctx.input.held(KeyCode::Space),
            dash_pressed: ctx.input.pressed(KeyCode::ShiftLeft),
            wall_left: self.solid_at(self.x - 1.0, self.y + 8.0),
            wall_right: self.solid_at(self.x + 17.0, self.y + 8.0),
            on_ground: grounded,
        };
        let (out, _events) = self.controller.tick(input, GRAVITY);

        // Move and stop against solids, axis at a time.
        let nx = self.x + out.velocity_x;
        if !self.solid_at(nx + if out.velocity_x < 0.0 { 0.0 } else { 16.0 }, self.y + 8.0) {
            self.x = nx;
        }
        let ny = self.y + out.velocity_y;
        let probe = if out.velocity_y < 0.0 { ny } else { ny + 16.0 };
        if !self.solid_at(self.x + 8.0, probe) {
            self.y = ny;
        } else {
            self.controller.velocity_y = 0.0;
        }

        ctx.camera.set_target(RenderVec2 {
            x: self.x,
            y: self.y,
        });

        let vh = ctx.camera.virtual_height;
        ctx.ui.pixel_text(
            "A/D move    SPACE jump    SHIFT dash    ESC pause",
            4.0,
            vh - 10.0,
            Color::rgb(0.7, 0.7, 0.6),
        );

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.09, 0.10, 0.16),
        );
        for y in 0..self.map_h {
            for x in 0..self.map_w {
                if self.tiles[y * self.map_w + x] == 1 {
                    ctx.draw_rect(
                        Rect::new(x as f32 * TILE, y as f32 * TILE, TILE, TILE),
                        Color::rgb(0.30, 0.28, 0.34),
                    );
                }
            }
        }
        ctx.draw_rect(
            Rect::new(self.x, self.y, 16.0, 16.0),
            Color::rgb(1.0, 0.6, 0.35),
        );
    }
}
"#;

const GRID_STEP_BODY: &str = r#"use amigo_engine::prelude::*;

/// One grid cell per key press, with a goal to reach. The basis for a puzzle
/// board or a turn-per-step dungeon crawl.
pub struct Gameplay {
    player: (i32, i32),
    goal: (i32, i32),
    steps: u32,
    solved: bool,
    walls: Vec<(i32, i32)>,
}

const CELL: f32 = 20.0;
const GRID_W: i32 = 14;
const GRID_H: i32 = 10;

impl Gameplay {
    pub fn new() -> Self {
        Self {
            player: (1, 1),
            goal: (GRID_W - 2, GRID_H - 2),
            steps: 0,
            solved: false,
            walls: vec![(5, 3), (5, 4), (5, 5), (6, 5), (9, 6), (9, 7)],
        }
    }

    fn free(&self, cell: (i32, i32)) -> bool {
        cell.0 > 0
            && cell.1 > 0
            && cell.0 < GRID_W - 1
            && cell.1 < GRID_H - 1
            && !self.walls.contains(&cell)
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        if !self.solved {
            // `pressed`, not `held`: one cell per press is the whole point.
            let step = if ctx.input.pressed(KeyCode::KeyA) || ctx.input.pressed(KeyCode::ArrowLeft) {
                Some((-1, 0))
            } else if ctx.input.pressed(KeyCode::KeyD) || ctx.input.pressed(KeyCode::ArrowRight) {
                Some((1, 0))
            } else if ctx.input.pressed(KeyCode::KeyW) || ctx.input.pressed(KeyCode::ArrowUp) {
                Some((0, -1))
            } else if ctx.input.pressed(KeyCode::KeyS) || ctx.input.pressed(KeyCode::ArrowDown) {
                Some((0, 1))
            } else {
                None
            };

            if let Some((dx, dy)) = step {
                let target = (self.player.0 + dx, self.player.1 + dy);
                if self.free(target) {
                    self.player = target;
                    self.steps += 1;
                    if self.player == self.goal {
                        self.solved = true;
                    }
                }
            }
        } else if ctx.input.pressed(KeyCode::KeyR) {
            *self = Self::new();
        }

        let vh = ctx.camera.virtual_height;
        ctx.ui.pixel_text(
            &format!("Steps: {}", self.steps),
            4.0,
            4.0,
            Color::WHITE,
        );
        let hint = if self.solved {
            "Solved!  R restarts    ESC pause"
        } else {
            "Arrows step    ESC pause"
        };
        ctx.ui.pixel_text(hint, 4.0, vh - 10.0, Color::rgb(0.7, 0.7, 0.6));

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.08, 0.08, 0.11),
        );
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let cell = (x, y);
                let color = if !self.free(cell) {
                    Color::rgb(0.28, 0.26, 0.30)
                } else if (x + y) % 2 == 0 {
                    Color::rgb(0.16, 0.17, 0.20)
                } else {
                    Color::rgb(0.13, 0.14, 0.17)
                };
                ctx.draw_rect(
                    Rect::new(x as f32 * CELL, y as f32 * CELL, CELL - 1.0, CELL - 1.0),
                    color,
                );
            }
        }
        ctx.draw_rect(
            Rect::new(
                self.goal.0 as f32 * CELL + 4.0,
                self.goal.1 as f32 * CELL + 4.0,
                CELL - 9.0,
                CELL - 9.0,
            ),
            Color::rgb(0.3, 0.9, 0.4),
        );
        ctx.draw_rect(
            Rect::new(
                self.player.0 as f32 * CELL + 3.0,
                self.player.1 as f32 * CELL + 3.0,
                CELL - 7.0,
                CELL - 7.0,
            ),
            Color::rgb(1.0, 0.85, 0.3),
        );
    }
}
"#;

const SHOOTER_BODY: &str = r#"use amigo_engine::prelude::*;

/// A ship that moves and shoots, with bullets that expire off the top of the
/// screen. Enough to hang a pattern system or an enemy wave off.
pub struct Gameplay {
    ship_x: f32,
    ship_y: f32,
    bullets: Vec<RenderVec2>,
    cooldown: f32,
    fired: u32,
}

impl Gameplay {
    pub fn new() -> Self {
        Self {
            ship_x: 0.0,
            ship_y: 0.0,
            bullets: Vec::new(),
            cooldown: 0.0,
            fired: 0,
        }
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn on_enter(&mut self, ctx: &mut GameContext) {
        // Centre the ship near the bottom of the visible area.
        self.ship_x = ctx.camera.virtual_width * 0.5;
        self.ship_y = ctx.camera.virtual_height - 30.0;
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        let (vw, vh) = (ctx.camera.virtual_width, ctx.camera.virtual_height);
        let speed = 110.0 * ctx.time.dt;
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) {
            self.ship_x -= speed;
        }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) {
            self.ship_x += speed;
        }
        if ctx.input.held(KeyCode::KeyW) || ctx.input.held(KeyCode::ArrowUp) {
            self.ship_y -= speed;
        }
        if ctx.input.held(KeyCode::KeyS) || ctx.input.held(KeyCode::ArrowDown) {
            self.ship_y += speed;
        }
        self.ship_x = self.ship_x.clamp(4.0, vw - 12.0);
        self.ship_y = self.ship_y.clamp(4.0, vh - 12.0);

        self.cooldown -= ctx.time.dt;
        if ctx.input.held(KeyCode::Space) && self.cooldown <= 0.0 {
            self.bullets.push(RenderVec2 {
                x: self.ship_x + 3.0,
                y: self.ship_y - 4.0,
            });
            self.cooldown = 0.12;
            self.fired += 1;
        }

        for bullet in &mut self.bullets {
            bullet.y -= 220.0 * ctx.time.dt;
        }
        // Drop bullets that left the screen, or the Vec grows for the whole run.
        self.bullets.retain(|b| b.y > -8.0);

        ctx.ui
            .pixel_text(&format!("Shots: {}", self.fired), 4.0, 4.0, Color::WHITE);
        ctx.ui.pixel_text(
            "Arrows move    SPACE fire    ESC pause",
            4.0,
            vh - 10.0,
            Color::rgb(0.7, 0.7, 0.6),
        );

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.04, 0.04, 0.09),
        );
        for bullet in &self.bullets {
            ctx.draw_rect(
                Rect::new(bullet.x, bullet.y, 2.0, 6.0),
                Color::rgb(1.0, 0.95, 0.5),
            );
        }
        ctx.draw_rect(
            Rect::new(self.ship_x, self.ship_y, 8.0, 8.0),
            Color::rgb(0.5, 0.9, 1.0),
        );
    }
}
"#;

const TURN_LOOP_BODY: &str = r#"use amigo_engine::prelude::*;

/// A round-based loop with two sides and a choice per turn. The shape most
/// turn-based, card and dialogue-driven games start from: state advances when the
/// player commits, not on a timer.
pub struct Gameplay {
    round: u32,
    player_hp: i32,
    enemy_hp: i32,
    log: Vec<String>,
    /// Deterministic pseudo-random, seeded from the round: replays and saves stay
    /// reproducible, which f32-based randomness would not.
    seed: u32,
}

impl Gameplay {
    pub fn new() -> Self {
        Self {
            round: 1,
            player_hp: 30,
            enemy_hp: 30,
            log: vec!["Round 1 — choose an action".to_string()],
            seed: 0x1234_5678,
        }
    }

    /// xorshift32: small, deterministic, good enough for a starter.
    fn next_rand(&mut self, max: u32) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed % max.max(1)
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
        // Keep the log bounded; a long fight would otherwise grow it forever.
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }

    fn resolve(&mut self, player_damage: i32) {
        self.enemy_hp -= player_damage;
        self.say(format!("You deal {player_damage}"));
        if self.enemy_hp <= 0 {
            self.say("Enemy down!".to_string());
            return;
        }
        let incoming = 2 + self.next_rand(4) as i32;
        self.player_hp -= incoming;
        self.say(format!("Enemy deals {incoming}"));
        self.round += 1;
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        let over = self.player_hp <= 0 || self.enemy_hp <= 0;
        if over {
            if ctx.input.pressed(KeyCode::KeyR) {
                *self = Self::new();
            }
        } else if ctx.input.pressed(KeyCode::Digit1) {
            let dmg = 3 + self.next_rand(3) as i32;
            self.resolve(dmg);
        } else if ctx.input.pressed(KeyCode::Digit2) {
            // Defend: less damage out, heal a little.
            self.player_hp = (self.player_hp + 3).min(30);
            self.say("You brace".to_string());
            self.resolve(1);
        }

        let (vw, vh) = (ctx.camera.virtual_width, ctx.camera.virtual_height);
        ctx.ui.pixel_text(&format!("Round {}", self.round), 4.0, 4.0, Color::WHITE);
        ctx.ui.progress_bar(
            Rect::new(4.0, 14.0, 80.0, 6.0),
            self.player_hp as f32 / 30.0,
            Color::rgb(0.4, 0.9, 0.5),
        );
        ctx.ui.progress_bar(
            Rect::new(vw - 84.0, 14.0, 80.0, 6.0),
            self.enemy_hp as f32 / 30.0,
            Color::rgb(0.9, 0.4, 0.4),
        );

        let hint = if over {
            "R restarts    ESC pause"
        } else {
            "1 attack    2 defend    ESC pause"
        };
        ctx.ui.pixel_text(hint, 4.0, vh - 10.0, Color::rgb(0.7, 0.7, 0.6));

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let (vw, vh) = (ctx.virtual_width, ctx.virtual_height);
        ctx.draw_rect(Rect::new(0.0, 0.0, vw, vh), Color::rgb(0.10, 0.08, 0.12));

        // The two combatants.
        ctx.draw_rect(
            Rect::new(vw * 0.25 - 8.0, vh * 0.45, 16.0, 24.0),
            Color::rgb(0.4, 0.7, 1.0),
        );
        ctx.draw_rect(
            Rect::new(vw * 0.75 - 8.0, vh * 0.45, 16.0, 24.0),
            Color::rgb(1.0, 0.5, 0.45),
        );

        // Combat log, newest at the bottom.
        let mut y = vh * 0.62;
        for line in &self.log {
            ctx.draw_text(line, vw * 0.5 - 50.0, y, Color::rgb(0.75, 0.75, 0.7));
            y += 9.0;
        }
    }
}
"#;

const IDLE_BODY: &str = r#"use amigo_engine::prelude::*;

/// A resource that accrues on its own plus an upgrade to spend it on — the whole
/// loop of an incremental game, in miniature.
pub struct Gameplay {
    resource: f64,
    per_second: f64,
    upgrades: u32,
}

impl Gameplay {
    pub fn new() -> Self {
        Self {
            resource: 0.0,
            per_second: 1.0,
            upgrades: 0,
        }
    }

    /// Cost curve: each upgrade is 60% dearer than the last.
    fn upgrade_cost(&self) -> f64 {
        10.0 * 1.6_f64.powi(self.upgrades as i32)
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        // dt, not a per-tick constant: the rate stays right if the tick rate ever
        // changes, and matches what the player is told.
        self.resource += self.per_second * ctx.time.dt as f64;

        if ctx.input.pressed(KeyCode::Space) {
            self.resource += 1.0;
        }
        let cost = self.upgrade_cost();
        if ctx.input.pressed(KeyCode::Enter) && self.resource >= cost {
            self.resource -= cost;
            self.per_second += 1.0;
            self.upgrades += 1;
        }

        let (vw, vh) = (ctx.camera.virtual_width, ctx.camera.virtual_height);
        ctx.ui.pixel_text(
            &format!("{:.0} units", self.resource),
            vw * 0.5 - 30.0,
            vh * 0.35,
            Color::rgb(1.0, 0.95, 0.7),
        );
        ctx.ui.pixel_text(
            &format!("{:.1}/sec   x{} upgrades", self.per_second, self.upgrades),
            vw * 0.5 - 55.0,
            vh * 0.45,
            Color::rgb(0.7, 0.7, 0.65),
        );
        // Progress toward affording the next upgrade.
        ctx.ui.progress_bar(
            Rect::new(vw * 0.5 - 50.0, vh * 0.55, 100.0, 6.0),
            (self.resource / cost) as f32,
            Color::rgb(0.4, 0.8, 1.0),
        );
        ctx.ui.pixel_text(
            &format!("SPACE +1    ENTER upgrade ({cost:.0})    ESC pause"),
            4.0,
            vh - 10.0,
            Color::rgb(0.7, 0.7, 0.6),
        );

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.07, 0.09, 0.12),
        );
    }
}
"#;

const TOWER_DEFENSE_BODY: &str = r#"use amigo_engine::prelude::*;

/// Place towers along a path, then run a wave down it. The two halves of a tower
/// defense loop, with the balance left to you.
pub struct Gameplay {
    /// Path the creeps walk, in tile coordinates.
    path: Vec<(i32, i32)>,
    towers: Vec<(i32, i32)>,
    /// Creep progress along the path, in tiles.
    creeps: Vec<f32>,
    cursor: (i32, i32),
    gold: i32,
    lives: i32,
    wave: u32,
    wave_running: bool,
    spawn_timer: f32,
    spawned: u32,
}

const TILE: f32 = 20.0;
const TOWER_COST: i32 = 20;

impl Gameplay {
    pub fn new() -> Self {
        // A simple S-bend across the board.
        let mut path = Vec::new();
        for x in 0..8 {
            path.push((x, 3));
        }
        for y in 3..8 {
            path.push((7, y));
        }
        for x in 8..16 {
            path.push((x, 7));
        }
        Self {
            path,
            towers: Vec::new(),
            creeps: Vec::new(),
            cursor: (3, 5),
            gold: 60,
            lives: 10,
            wave: 0,
            wave_running: false,
            spawn_timer: 0.0,
            spawned: 0,
        }
    }

    fn on_path(&self, cell: (i32, i32)) -> bool {
        self.path.contains(&cell)
    }

    fn tile_of(&self, progress: f32) -> (f32, f32) {
        let index = (progress as usize).min(self.path.len().saturating_sub(1));
        let (x, y) = self.path[index];
        (x as f32 * TILE, y as f32 * TILE)
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Gameplay {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
PAUSE_PLACEHOLDER
        // Move the build cursor.
        if ctx.input.pressed(KeyCode::ArrowLeft) {
            self.cursor.0 = (self.cursor.0 - 1).max(0);
        }
        if ctx.input.pressed(KeyCode::ArrowRight) {
            self.cursor.0 = (self.cursor.0 + 1).min(15);
        }
        if ctx.input.pressed(KeyCode::ArrowUp) {
            self.cursor.1 = (self.cursor.1 - 1).max(0);
        }
        if ctx.input.pressed(KeyCode::ArrowDown) {
            self.cursor.1 = (self.cursor.1 + 1).min(11);
        }

        // Build: not on the path, not on top of another tower, and only if paid for.
        if ctx.input.pressed(KeyCode::Space)
            && self.gold >= TOWER_COST
            && !self.on_path(self.cursor)
            && !self.towers.contains(&self.cursor)
        {
            self.towers.push(self.cursor);
            self.gold -= TOWER_COST;
        }

        if ctx.input.pressed(KeyCode::Enter) && !self.wave_running {
            self.wave += 1;
            self.wave_running = true;
            self.spawned = 0;
            self.spawn_timer = 0.0;
        }

        if self.wave_running {
            self.spawn_timer -= ctx.time.dt;
            let wave_size = 4 + self.wave * 2;
            if self.spawn_timer <= 0.0 && self.spawned < wave_size {
                self.creeps.push(0.0);
                self.spawned += 1;
                self.spawn_timer = 0.6;
            }

            let path_len = self.path.len() as f32;
            for creep in &mut self.creeps {
                *creep += 2.0 * ctx.time.dt;
            }
            // Creeps that reach the end cost a life.
            let before = self.creeps.len();
            self.creeps.retain(|c| *c < path_len - 1.0);
            self.lives -= (before - self.creeps.len()) as i32;

            // Towers kill the creep nearest them; one shot per tower per second.
            if !self.towers.is_empty() && !self.creeps.is_empty() {
                let kills = ((self.towers.len() as f32) * ctx.time.dt * 0.8) as usize;
                for _ in 0..kills.min(self.creeps.len()) {
                    self.creeps.remove(0);
                    self.gold += 5;
                }
            }

            if self.spawned >= wave_size && self.creeps.is_empty() {
                self.wave_running = false;
                self.gold += 20;
            }
        }

        let vh = ctx.camera.virtual_height;
        ctx.ui.pixel_text(
            &format!("Gold {}   Lives {}   Wave {}", self.gold, self.lives, self.wave),
            4.0,
            4.0,
            Color::WHITE,
        );
        let hint = if self.wave_running {
            "Arrows move    SPACE build    ESC pause"
        } else {
            "Arrows move    SPACE build    ENTER next wave    ESC pause"
        };
        ctx.ui.pixel_text(hint, 4.0, vh - 10.0, Color::rgb(0.7, 0.7, 0.6));

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.11, 0.14, 0.10),
        );
        for (x, y) in &self.path {
            ctx.draw_rect(
                Rect::new(*x as f32 * TILE, *y as f32 * TILE, TILE, TILE),
                Color::rgb(0.42, 0.36, 0.26),
            );
        }
        for (x, y) in &self.towers {
            ctx.draw_rect(
                Rect::new(*x as f32 * TILE + 3.0, *y as f32 * TILE + 3.0, TILE - 6.0, TILE - 6.0),
                Color::rgb(0.5, 0.7, 1.0),
            );
        }
        for creep in &self.creeps {
            let (x, y) = self.tile_of(*creep);
            ctx.draw_rect(
                Rect::new(x + 6.0, y + 6.0, TILE - 12.0, TILE - 12.0),
                Color::rgb(1.0, 0.45, 0.4),
            );
        }
        // Build cursor: green where a tower may go, red where it may not.
        let valid = !self.on_path(self.cursor) && !self.towers.contains(&self.cursor);
        let color = if valid {
            Color::rgb(0.4, 1.0, 0.5).with_alpha(0.5)
        } else {
            Color::rgb(1.0, 0.35, 0.35).with_alpha(0.5)
        };
        ctx.draw_rect(
            Rect::new(self.cursor.0 as f32 * TILE, self.cursor.1 as f32 * TILE, TILE, TILE),
            color,
        );
    }
}
"#;

/// Substitute the shared pause snippet into a body.
fn with_pause(body: &str) -> String {
    body.replace("PAUSE_PLACEHOLDER\n", PAUSE_SNIPPET)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset must map to a family: a missing arm would mean `amigo new`
    /// panics for that template.
    #[test]
    fn every_preset_has_a_family() {
        let presets = [
            ScenePreset::TopDown,
            ScenePreset::Platformer,
            ScenePreset::TurnBased,
            ScenePreset::Arpg,
            ScenePreset::Roguelike,
            ScenePreset::TowerDefense,
            ScenePreset::BulletHell,
            ScenePreset::ArcadeShooter,
            ScenePreset::Puzzle,
            ScenePreset::FarmingSim,
            ScenePreset::Fighting,
            ScenePreset::VisualNovel,
            ScenePreset::Menu,
            ScenePreset::WorldMap,
            ScenePreset::Sandbox,
            ScenePreset::GodSim,
            ScenePreset::Custom,
            ScenePreset::SocialDeduction,
            ScenePreset::Deckbuilder,
            ScenePreset::AutoBattler,
            ScenePreset::Idle,
        ];
        for preset in presets {
            let files = project_files(preset, "test_game", 480, 270);
            assert!(
                files.iter().any(|f| f.path == "src/main.rs"),
                "{preset:?} produced no main.rs"
            );
        }
    }

    #[test]
    fn a_menu_project_has_no_gameplay_or_pause_scene() {
        let files = project_files(ScenePreset::Menu, "menu_only", 480, 270);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert!(!paths.contains(&"src/scenes/gameplay.rs"));
        assert!(
            !paths.contains(&"src/scenes/pause_menu.rs"),
            "a UI-only project has nothing to pause"
        );
    }

    #[test]
    fn gameplay_scenes_get_the_pause_handler_substituted() {
        let files = project_files(ScenePreset::Platformer, "p", 480, 270);
        let gameplay = files
            .iter()
            .find(|f| f.path == "src/scenes/gameplay.rs")
            .expect("platformer has a gameplay scene");
        assert!(
            !gameplay.contents.contains("PAUSE_PLACEHOLDER"),
            "the placeholder must be replaced, or the generated code will not compile"
        );
        assert!(gameplay.contents.contains("PauseMenu::new()"));
    }

    #[test]
    fn the_project_name_reaches_main_and_the_title() {
        let files = project_files(ScenePreset::TopDown, "my_cool_game", 320, 180);
        let main = files.iter().find(|f| f.path == "src/main.rs").unwrap();
        assert!(main.contents.contains("my_cool_game"));
        assert!(main.contents.contains("virtual_resolution(320, 180)"));
    }

    #[test]
    fn presets_with_a_matching_core_module_point_at_it() {
        let files = project_files(ScenePreset::TowerDefense, "td", 480, 270);
        let gameplay = files
            .iter()
            .find(|f| f.path == "src/scenes/gameplay.rs")
            .unwrap();
        assert!(
            gameplay.contents.contains("amigo_core::tower"),
            "a starter should say where the deeper systems live"
        );
    }
}
