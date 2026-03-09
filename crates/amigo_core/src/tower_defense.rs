use serde::{Serialize, Deserialize};

/// Targeting priority for towers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetingPriority {
    First,
    Last,
    Strongest,
    Weakest,
    Closest,
}

/// A single upgrade tier for a tower.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerUpgrade {
    pub cost: i32,
    pub damage_bonus: i32,
    pub range_bonus: i32,
    pub fire_rate_bonus: i32,
    pub description: String,
}

/// RON-loadable tower template.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerDefinition {
    pub id: u32,
    pub name: String,
    pub cost: i32,
    /// Fixed-point raw damage value.
    pub damage: i32,
    /// Fixed-point raw range in tiles.
    pub range: i32,
    /// Ticks between shots.
    pub fire_rate: i32,
    /// Fixed-point raw projectile speed.
    pub projectile_speed: i32,
    pub targeting: TargetingPriority,
    pub upgrades: Vec<TowerUpgrade>,
}

/// RON-loadable enemy template.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyDefinition {
    pub id: u32,
    pub name: String,
    pub health: i32,
    /// Fixed-point raw speed.
    pub speed: i32,
    /// Gold reward on kill.
    pub reward: i32,
    /// Lives lost when reaching the end.
    pub damage: i32,
    pub armor: i32,
    pub is_flying: bool,
    pub is_boss: bool,
}

/// Defines a single wave composed of enemy groups.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveDefinition {
    pub groups: Vec<WaveGroup>,
}

/// A group of enemies within a wave.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveGroup {
    pub enemy_type: u32,
    pub count: u32,
    /// Ticks between spawns within this group.
    pub spawn_interval: u32,
    /// Ticks to wait before this group starts.
    pub delay_before: u32,
    /// Which waypoint path to follow.
    pub path_index: u32,
}

/// An event indicating an enemy should be spawned.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnEvent {
    pub enemy_type: u32,
    pub path_index: u32,
}

/// Manages wave progression and enemy spawning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveManager {
    definitions: Vec<WaveDefinition>,
    current_wave: u32,
    current_group: usize,
    spawned_in_group: u32,
    spawn_timer: u32,
    delay_timer: u32,
    active: bool,
}

impl WaveManager {
    pub fn new(definitions: Vec<WaveDefinition>) -> Self {
        Self {
            definitions,
            current_wave: 0,
            current_group: 0,
            spawned_in_group: 0,
            spawn_timer: 0,
            delay_timer: 0,
            active: false,
        }
    }

    /// Begin spawning the current wave.
    pub fn start_wave(&mut self) {
        if (self.current_wave as usize) < self.definitions.len() {
            self.active = true;
            self.current_group = 0;
            self.spawned_in_group = 0;
            self.spawn_timer = 0;

            // Initialize the delay timer from the first group's delay_before.
            if let Some(wave) = self.definitions.get(self.current_wave as usize) {
                if let Some(group) = wave.groups.first() {
                    self.delay_timer = group.delay_before;
                } else {
                    self.delay_timer = 0;
                }
            }
        }
    }

    /// Advance one tick. Returns any enemies to spawn this tick.
    pub fn update(&mut self) -> Vec<SpawnEvent> {
        let mut events = Vec::new();

        if !self.active {
            return events;
        }

        let wave_index = self.current_wave as usize;
        let wave = match self.definitions.get(wave_index) {
            Some(w) => w.clone(),
            None => {
                self.active = false;
                return events;
            }
        };

        if self.current_group >= wave.groups.len() {
            self.active = false;
            self.current_wave += 1;
            return events;
        }

        // Handle delay before the current group.
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
            return events;
        }

        let group = &wave.groups[self.current_group];

        // Handle spawn timer.
        if self.spawned_in_group > 0 && self.spawn_timer > 0 {
            self.spawn_timer -= 1;
            return events;
        }

        // Spawn an enemy.
        if self.spawned_in_group < group.count {
            events.push(SpawnEvent {
                enemy_type: group.enemy_type,
                path_index: group.path_index,
            });
            self.spawned_in_group += 1;
            self.spawn_timer = group.spawn_interval;
        }

        // Check if the current group is finished.
        if self.spawned_in_group >= group.count {
            self.current_group += 1;
            self.spawned_in_group = 0;
            self.spawn_timer = 0;

            // Set up the delay for the next group.
            if self.current_group < wave.groups.len() {
                self.delay_timer = wave.groups[self.current_group].delay_before;
            }
        }

        events
    }

    /// Returns true if every group in the current wave has finished spawning.
    pub fn is_wave_complete(&self) -> bool {
        if self.active {
            return false;
        }
        true
    }

    /// Total number of waves.
    pub fn total_waves(&self) -> u32 {
        self.definitions.len() as u32
    }
}

/// Tracks player gold and lives.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Economy {
    pub gold: i32,
    pub lives: i32,
    pub total_earned: i32,
    pub total_spent: i32,
}

impl Economy {
    pub fn new(starting_gold: i32, starting_lives: i32) -> Self {
        Self {
            gold: starting_gold,
            lives: starting_lives,
            total_earned: starting_gold,
            total_spent: 0,
        }
    }

    pub fn can_afford(&self, cost: i32) -> bool {
        self.gold >= cost
    }

    /// Attempts to spend gold. Returns true if successful.
    pub fn spend(&mut self, amount: i32) -> bool {
        if self.gold >= amount {
            self.gold -= amount;
            self.total_spent += amount;
            true
        } else {
            false
        }
    }

    pub fn earn(&mut self, amount: i32) {
        self.gold += amount;
        self.total_earned += amount;
    }

    /// Subtract lives. Returns true if the player is still alive.
    pub fn lose_lives(&mut self, amount: i32) -> bool {
        self.lives -= amount;
        self.lives > 0
    }

    pub fn is_game_over(&self) -> bool {
        self.lives <= 0
    }
}
