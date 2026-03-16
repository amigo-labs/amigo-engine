use crate::math::RenderVec2;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Wave definitions
// ---------------------------------------------------------------------------

/// A group of enemies to spawn within a wave.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnGroup {
    /// Enemy type identifier (game defines the mapping).
    pub enemy_type: u32,
    /// How many to spawn.
    pub count: u32,
    /// Delay between each spawn in seconds.
    pub spawn_interval: f32,
    /// Which spawn point index to use.
    pub spawn_point: usize,
}

/// A single wave definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveDef {
    /// Groups to spawn in this wave.
    pub groups: Vec<SpawnGroup>,
    /// Delay before the wave starts (after previous wave ends).
    pub start_delay: f32,
    /// Optional text / announcement.
    pub announcement: Option<String>,
}

impl WaveDef {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            start_delay: 3.0,
            announcement: None,
        }
    }

    pub fn with_group(
        mut self,
        enemy_type: u32,
        count: u32,
        interval: f32,
        spawn_point: usize,
    ) -> Self {
        self.groups.push(SpawnGroup {
            enemy_type,
            count,
            spawn_interval: interval,
            spawn_point,
        });
        self
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.start_delay = delay;
        self
    }

    pub fn with_announcement(mut self, text: impl Into<String>) -> Self {
        self.announcement = Some(text.into());
        self
    }

    /// Total number of enemies in this wave.
    pub fn total_enemies(&self) -> u32 {
        self.groups.iter().map(|g| g.count).sum()
    }
}

impl Default for WaveDef {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Spawn event
// ---------------------------------------------------------------------------

/// Event emitted when the spawner wants to create an enemy.
#[derive(Clone, Debug)]
pub struct SpawnEvent {
    pub enemy_type: u32,
    pub position: RenderVec2,
    pub wave_index: usize,
    pub group_index: usize,
}

// ---------------------------------------------------------------------------
// Wave Spawner
// ---------------------------------------------------------------------------

/// Runtime state for a single spawn group.
#[derive(Clone, Debug)]
struct GroupState {
    spawned: u32,
    timer: f32,
}

/// Phase of the wave spawner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavePhase {
    /// Waiting for the pre-wave delay.
    Waiting,
    /// Actively spawning enemies.
    Spawning,
    /// All spawned, waiting for enemies to be killed.
    Active,
    /// Wave complete, all enemies dead.
    Complete,
    /// All waves finished.
    Victory,
}

/// The wave spawner system.
pub struct WaveSpawner {
    pub waves: Vec<WaveDef>,
    pub spawn_points: Vec<RenderVec2>,
    pub current_wave: usize,
    pub phase: WavePhase,
    /// Enemies still alive from the current wave.
    pub enemies_alive: u32,
    /// Total enemies killed across all waves.
    pub total_kills: u32,

    delay_timer: f32,
    group_states: Vec<GroupState>,
    auto_advance: bool,
}

impl WaveSpawner {
    pub fn new(waves: Vec<WaveDef>, spawn_points: Vec<RenderVec2>) -> Self {
        Self {
            waves,
            spawn_points,
            current_wave: 0,
            phase: WavePhase::Waiting,
            enemies_alive: 0,
            total_kills: 0,
            delay_timer: 0.0,
            group_states: Vec::new(),
            auto_advance: true,
        }
    }

    /// Set whether waves advance automatically when all enemies are dead.
    pub fn set_auto_advance(&mut self, auto: bool) {
        self.auto_advance = auto;
    }

    /// Manually start the next wave.
    pub fn start_next_wave(&mut self) {
        if self.current_wave < self.waves.len() {
            self.start_wave(self.current_wave);
        }
    }

    fn start_wave(&mut self, index: usize) {
        self.current_wave = index;
        let wave = &self.waves[index];
        self.delay_timer = wave.start_delay;
        self.phase = WavePhase::Waiting;
        self.group_states = wave
            .groups
            .iter()
            .map(|_| GroupState {
                spawned: 0,
                timer: 0.0,
            })
            .collect();
    }

    /// Update the spawner. Returns spawn events for this tick.
    pub fn update(&mut self, dt: f32) -> Vec<SpawnEvent> {
        let mut events = Vec::new();

        match self.phase {
            WavePhase::Victory | WavePhase::Complete => {
                if self.auto_advance && self.phase == WavePhase::Complete {
                    let next = self.current_wave + 1;
                    if next < self.waves.len() {
                        self.start_wave(next);
                    } else {
                        self.phase = WavePhase::Victory;
                    }
                }
                return events;
            }

            WavePhase::Waiting => {
                self.delay_timer -= dt;
                if self.delay_timer <= 0.0 {
                    self.phase = WavePhase::Spawning;
                }
                return events;
            }

            WavePhase::Active => {
                if self.enemies_alive == 0 {
                    self.phase = WavePhase::Complete;
                }
                return events;
            }

            WavePhase::Spawning => {}
        }

        // Spawning phase: emit enemies
        let wave = &self.waves[self.current_wave];
        let mut all_done = true;

        for (gi, group) in wave.groups.iter().enumerate() {
            let state = &mut self.group_states[gi];
            if state.spawned >= group.count {
                continue;
            }
            all_done = false;

            state.timer -= dt;
            if state.timer <= 0.0 {
                let spawn_pos = self
                    .spawn_points
                    .get(group.spawn_point)
                    .copied()
                    .unwrap_or(RenderVec2::ZERO);

                events.push(SpawnEvent {
                    enemy_type: group.enemy_type,
                    position: spawn_pos,
                    wave_index: self.current_wave,
                    group_index: gi,
                });

                state.spawned += 1;
                self.enemies_alive += 1;
                state.timer = group.spawn_interval;
            }
        }

        if all_done {
            self.phase = WavePhase::Active;
        }

        events
    }

    /// Call when an enemy dies.
    pub fn on_enemy_killed(&mut self) {
        self.enemies_alive = self.enemies_alive.saturating_sub(1);
        self.total_kills += 1;
    }

    /// Current wave number (1-indexed for display).
    pub fn wave_number(&self) -> usize {
        self.current_wave + 1
    }

    /// Total wave count.
    pub fn total_waves(&self) -> usize {
        self.waves.len()
    }

    /// Get announcement for current wave.
    pub fn current_announcement(&self) -> Option<&str> {
        self.waves
            .get(self.current_wave)
            .and_then(|w| w.announcement.as_deref())
    }

    /// Reset the spawner.
    pub fn reset(&mut self) {
        self.current_wave = 0;
        self.enemies_alive = 0;
        self.total_kills = 0;
        self.phase = WavePhase::Waiting;
        self.group_states.clear();
        if !self.waves.is_empty() {
            self.start_wave(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_spawner_basic() {
        let waves = vec![WaveDef::new().with_delay(0.0).with_group(1, 3, 0.1, 0)];
        let spawn_points = vec![RenderVec2::new(100.0, 100.0)];
        let mut spawner = WaveSpawner::new(waves, spawn_points);
        spawner.start_next_wave();

        // Waiting phase with 0 delay → should go to spawning immediately
        let mut total_spawned = 0;
        for _ in 0..50 {
            let events = spawner.update(0.05);
            total_spawned += events.len();
        }
        assert_eq!(total_spawned, 3);
        assert_eq!(spawner.enemies_alive, 3);

        // Kill all enemies
        for _ in 0..3 {
            spawner.on_enemy_killed();
        }
        spawner.update(0.1);
        assert_eq!(spawner.phase, WavePhase::Complete);
    }

    #[test]
    fn multi_wave_auto_advance() {
        let waves = vec![
            WaveDef::new().with_delay(0.0).with_group(1, 1, 0.0, 0),
            WaveDef::new().with_delay(0.0).with_group(2, 1, 0.0, 0),
        ];
        let spawn_points = vec![RenderVec2::ZERO];
        let mut spawner = WaveSpawner::new(waves, spawn_points);
        spawner.start_next_wave();

        // Run ticks until we get wave 1 spawn
        let mut wave1_spawns = Vec::new();
        for _ in 0..10 {
            let events = spawner.update(0.1);
            wave1_spawns.extend(events);
            if !wave1_spawns.is_empty() {
                break;
            }
        }
        assert_eq!(wave1_spawns.len(), 1);
        assert_eq!(wave1_spawns[0].enemy_type, 1);

        // Kill and advance to wave 2
        spawner.on_enemy_killed();
        let mut wave2_spawns = Vec::new();
        for _ in 0..10 {
            let events = spawner.update(0.1);
            wave2_spawns.extend(events);
            if !wave2_spawns.is_empty() {
                break;
            }
        }
        assert_eq!(wave2_spawns.len(), 1);
        assert_eq!(wave2_spawns[0].enemy_type, 2);

        // Kill and finish
        spawner.on_enemy_killed();
        for _ in 0..10 {
            spawner.update(0.1);
        }
        assert_eq!(spawner.phase, WavePhase::Victory);
    }
}
