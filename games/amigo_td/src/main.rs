//! Amigo TD — Example tower defense game demonstrating the Amigo Engine.
//!
//! Shows how to use:
//! - Tower system (TowerDef, TowerInstance)
//! - Wave system (WaveScheduler, WaveDef, SpawnGroup)
//! - UI drawing with DrawContext
//! - Scene system for game flow

use amigo_engine::prelude::*;
use amigo_core::tower::*;
use amigo_core::waves::*;

// ---------------------------------------------------------------------------
// Game-local data (no ECS needed for this simple example)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct EnemyData {
    x: f32,
    y: f32,
    health: i32,
    max_health: i32,
    speed: f32,
    path_index: usize,
    path_t: f32,
    gold_reward: u32,
    alive: bool,
}

#[derive(Clone, Debug)]
struct TowerData {
    x: f32,
    y: f32,
    def_id: usize,
    cooldown: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GamePhase {
    Build,
    Wave,
    Victory,
    GameOver,
}

// ---------------------------------------------------------------------------
// Game state
// ---------------------------------------------------------------------------

struct TdGame {
    enemy_path: Vec<(f32, f32)>,
    tower_defs: Vec<TowerDef>,
    waves: Vec<WaveDef>,
    current_wave: usize,
    enemies: Vec<EnemyData>,
    towers: Vec<TowerData>,
    spawn_timer: f32,
    spawn_queue: Vec<(i32, f32, u32)>, // (health, speed, gold)
    gold: u32,
    lives: i32,
    score: u32,
    selected_tower: usize,
    phase: GamePhase,
    announce_timer: f32,
}

impl TdGame {
    fn new() -> Self {
        // Enemy path (S-curve).
        let enemy_path = vec![
            (0.0, 90.0),
            (80.0, 90.0),
            (80.0, 30.0),
            (160.0, 30.0),
            (160.0, 150.0),
            (240.0, 150.0),
            (240.0, 90.0),
            (320.0, 90.0),
        ];

        let tower_defs = vec![
            TowerDef {
                id: 0,
                name: "Arrow Tower".to_string(),
                tiers: vec![TowerTier {
                    damage: 10,
                    range: 50.0,
                    attack_speed: 1.0,
                    cost: 50,
                    attack_type: TowerAttackType::SingleTarget,
                    sprite_name: "tower_arrow".to_string(),
                }],
                targeting: TargetingStrategy::First,
            },
            TowerDef {
                id: 1,
                name: "Frost Tower".to_string(),
                tiers: vec![TowerTier {
                    damage: 5,
                    range: 40.0,
                    attack_speed: 0.5,
                    cost: 75,
                    attack_type: TowerAttackType::Splash { radius: 30.0 },
                    sprite_name: "tower_frost".to_string(),
                }],
                targeting: TargetingStrategy::Nearest,
            },
            TowerDef {
                id: 2,
                name: "Cannon Tower".to_string(),
                tiers: vec![TowerTier {
                    damage: 30,
                    range: 60.0,
                    attack_speed: 0.3,
                    cost: 100,
                    attack_type: TowerAttackType::Splash { radius: 25.0 },
                    sprite_name: "tower_cannon".to_string(),
                }],
                targeting: TargetingStrategy::Strongest,
            },
        ];

        let waves = vec![
            WaveDef {
                groups: vec![SpawnGroup {
                    enemy_type: 0,
                    count: 5,
                    spawn_interval: 0.8,
                    spawn_point: 0,
                }],
                start_delay: 2.0,
                announcement: Some("Wave 1: Goblins!".to_string()),
            },
            WaveDef {
                groups: vec![
                    SpawnGroup {
                        enemy_type: 0,
                        count: 8,
                        spawn_interval: 0.6,
                        spawn_point: 0,
                    },
                    SpawnGroup {
                        enemy_type: 1,
                        count: 3,
                        spawn_interval: 1.5,
                        spawn_point: 0,
                    },
                ],
                start_delay: 3.0,
                announcement: Some("Wave 2: Goblins & Orcs!".to_string()),
            },
            WaveDef {
                groups: vec![SpawnGroup {
                    enemy_type: 1,
                    count: 10,
                    spawn_interval: 0.5,
                    spawn_point: 0,
                }],
                start_delay: 3.0,
                announcement: Some("Wave 3: Orc Horde!".to_string()),
            },
            WaveDef {
                groups: vec![SpawnGroup {
                    enemy_type: 2,
                    count: 1,
                    spawn_interval: 0.0,
                    spawn_point: 0,
                }],
                start_delay: 2.0,
                announcement: Some("BOSS WAVE!".to_string()),
            },
        ];

        Self {
            enemy_path,
            tower_defs,
            waves,
            current_wave: 0,
            enemies: Vec::new(),
            towers: Vec::new(),
            spawn_timer: 0.0,
            spawn_queue: Vec::new(),
            gold: 200,
            lives: 20,
            score: 0,
            selected_tower: 0,
            phase: GamePhase::Build,
            announce_timer: 0.0,
        }
    }

    fn enemy_stats(enemy_type: u32) -> (i32, f32, u32) {
        match enemy_type {
            0 => (30, 30.0, 10),   // Goblin
            1 => (80, 20.0, 25),   // Orc
            2 => (500, 10.0, 200), // Boss
            _ => (50, 25.0, 15),
        }
    }

    fn start_wave(&mut self) {
        if self.current_wave >= self.waves.len() {
            self.phase = GamePhase::Victory;
            return;
        }

        let wave = &self.waves[self.current_wave];
        self.announce_timer = 2.0;
        self.spawn_queue.clear();

        for group in &wave.groups {
            let (health, speed, gold) = Self::enemy_stats(group.enemy_type);
            for _ in 0..group.count {
                self.spawn_queue.push((health, speed, gold));
            }
        }

        self.spawn_timer = wave.start_delay;
        self.phase = GamePhase::Wave;
    }

    fn spawn_enemy(&mut self, health: i32, speed: f32, gold_reward: u32) {
        let start = self.enemy_path[0];
        self.enemies.push(EnemyData {
            x: start.0,
            y: start.1,
            health,
            max_health: health,
            speed,
            path_index: 0,
            path_t: 0.0,
            gold_reward,
            alive: true,
        });
    }
}

// ---------------------------------------------------------------------------
// Game trait
// ---------------------------------------------------------------------------

impl Game for TdGame {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let dt = ctx.time.dt;

        if ctx.input.pressed(KeyCode::Escape) {
            return SceneAction::Quit;
        }

        // Tower selection
        if ctx.input.pressed(KeyCode::Digit1) { self.selected_tower = 0; }
        if ctx.input.pressed(KeyCode::Digit2) { self.selected_tower = 1; }
        if ctx.input.pressed(KeyCode::Digit3) { self.selected_tower = 2; }

        if self.announce_timer > 0.0 {
            self.announce_timer -= dt;
        }

        match self.phase {
            GamePhase::Build => {
                if ctx.input.pressed(KeyCode::Space) {
                    self.start_wave();
                }

                // Place tower on click
                if ctx.input.mouse_pressed(MouseButton::Left) {
                    let mpos = ctx.input.mouse_world_pos();
                    let def = &self.tower_defs[self.selected_tower];
                    let cost = def.tiers[0].cost;
                    if self.gold >= cost {
                        self.gold -= cost;
                        self.towers.push(TowerData {
                            x: mpos.x,
                            y: mpos.y,
                            def_id: self.selected_tower,
                            cooldown: 0.0,
                        });
                    }
                }
            }

            GamePhase::Wave => {
                // Spawn from queue
                self.spawn_timer -= dt;
                if self.spawn_timer <= 0.0 && !self.spawn_queue.is_empty() {
                    let (hp, spd, gold) = self.spawn_queue.remove(0);
                    self.spawn_enemy(hp, spd, gold);
                    self.spawn_timer = 0.6;
                }

                // Move enemies
                let path = &self.enemy_path;
                for enemy in &mut self.enemies {
                    if !enemy.alive { continue; }

                    if enemy.path_index + 1 >= path.len() {
                        // Reached end
                        enemy.alive = false;
                        self.lives -= 1;
                        continue;
                    }

                    let (ax, ay) = path[enemy.path_index];
                    let (bx, by) = path[enemy.path_index + 1];
                    let dx = bx - ax;
                    let dy = by - ay;
                    let seg_len = (dx * dx + dy * dy).sqrt();

                    if seg_len > 0.0 {
                        enemy.path_t += (enemy.speed * dt) / seg_len;
                    }

                    if enemy.path_t >= 1.0 {
                        enemy.path_t = 0.0;
                        enemy.path_index += 1;
                    }

                    let t = enemy.path_t.clamp(0.0, 1.0);
                    enemy.x = ax + dx * t;
                    enemy.y = ay + dy * t;
                }

                // Tower attacks
                for tower in &mut self.towers {
                    tower.cooldown -= dt;
                    if tower.cooldown > 0.0 { continue; }

                    let def = &self.tower_defs[tower.def_id];
                    let tier = &def.tiers[0];

                    // Find first enemy in range
                    for enemy in &mut self.enemies {
                        if !enemy.alive || enemy.health <= 0 { continue; }

                        let dx = enemy.x - tower.x;
                        let dy = enemy.y - tower.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist <= tier.range {
                            enemy.health -= tier.damage;
                            tower.cooldown = 1.0 / tier.attack_speed;

                            if enemy.health <= 0 {
                                enemy.alive = false;
                                self.gold += enemy.gold_reward;
                                self.score += enemy.gold_reward;
                            }
                            break;
                        }
                    }
                }

                // Remove dead enemies
                self.enemies.retain(|e| e.alive);

                // Check wave complete
                if self.spawn_queue.is_empty() && self.enemies.is_empty() {
                    self.current_wave += 1;
                    if self.current_wave >= self.waves.len() {
                        self.phase = GamePhase::Victory;
                    } else {
                        self.phase = GamePhase::Build;
                    }
                }

                if self.lives <= 0 {
                    self.phase = GamePhase::GameOver;
                }
            }

            GamePhase::Victory | GamePhase::GameOver => {
                if ctx.input.pressed(KeyCode::KeyR) {
                    *self = TdGame::new();
                }
            }
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        // Draw path
        for i in 0..self.enemy_path.len().saturating_sub(1) {
            let (ax, ay) = self.enemy_path[i];
            let (bx, by) = self.enemy_path[i + 1];
            let min_x = ax.min(bx) - 4.0;
            let min_y = ay.min(by) - 4.0;
            let w = (bx - ax).abs() + 8.0;
            let h = (by - ay).abs() + 8.0;
            ctx.draw_rect(
                Rect::new(min_x, min_y, w, h),
                Color::new(0.3, 0.25, 0.2, 1.0),
            );
        }

        // Draw towers
        for tower in &self.towers {
            let color = match tower.def_id {
                0 => Color::new(0.2, 0.5, 0.8, 1.0), // Arrow: blue
                1 => Color::new(0.3, 0.8, 0.9, 1.0), // Frost: cyan
                2 => Color::new(0.7, 0.3, 0.1, 1.0), // Cannon: brown
                _ => Color::WHITE,
            };
            ctx.draw_rect(Rect::new(tower.x - 6.0, tower.y - 6.0, 12.0, 12.0), color);
        }

        // Draw enemies with health bars
        for enemy in &self.enemies {
            ctx.draw_rect(
                Rect::new(enemy.x - 4.0, enemy.y - 4.0, 8.0, 8.0),
                Color::new(0.8, 0.2, 0.2, 1.0),
            );
            let frac = enemy.health as f32 / enemy.max_health.max(1) as f32;
            let bar_w = 10.0;
            ctx.draw_rect(
                Rect::new(enemy.x - bar_w * 0.5, enemy.y - 8.0, bar_w, 2.0),
                Color::new(0.3, 0.0, 0.0, 0.8),
            );
            ctx.draw_rect(
                Rect::new(enemy.x - bar_w * 0.5, enemy.y - 8.0, bar_w * frac, 2.0),
                Color::new(0.0, 0.8, 0.0, 0.9),
            );
        }

        // HUD bars
        let gold_w = (self.gold as f32 / 5.0).min(100.0);
        ctx.draw_rect(Rect::new(4.0, 4.0, gold_w, 6.0), Color::new(1.0, 0.85, 0.0, 0.9));
        let lives_w = (self.lives as f32 / 20.0 * 60.0).max(0.0);
        ctx.draw_rect(Rect::new(4.0, 14.0, lives_w, 6.0), Color::new(0.9, 0.1, 0.1, 0.9));
        let wave_frac = (self.current_wave as f32 + 1.0) / self.waves.len() as f32;
        ctx.draw_rect(Rect::new(4.0, 24.0, 60.0 * wave_frac, 6.0), Color::new(0.3, 0.7, 1.0, 0.9));

        // Phase indicator
        let (phase_color, phase_w) = match self.phase {
            GamePhase::Build => (Color::new(0.0, 0.7, 0.0, 0.8), 40.0),
            GamePhase::Wave => (Color::new(0.9, 0.5, 0.0, 0.8), 40.0),
            GamePhase::Victory => (Color::new(1.0, 0.85, 0.0, 0.9), 120.0),
            GamePhase::GameOver => (Color::new(0.8, 0.0, 0.0, 0.9), 120.0),
        };

        match self.phase {
            GamePhase::Victory | GamePhase::GameOver => {
                ctx.draw_rect(Rect::new(100.0, 80.0, phase_w, 20.0), phase_color);
            }
            _ => {
                ctx.draw_rect(Rect::new(140.0, 4.0, phase_w, 8.0), phase_color);
            }
        }

        // Selected tower indicator
        let sel_x = 4.0 + self.selected_tower as f32 * 16.0;
        ctx.draw_rect(Rect::new(sel_x, 170.0, 12.0, 4.0), Color::new(1.0, 1.0, 1.0, 0.8));
    }
}

fn main() {
    Engine::build()
        .title("Amigo TD — Tower Defense Example")
        .virtual_resolution(320, 180)
        .window_size(1280, 720)
        .build()
        .run(TdGame::new());
}
