use crate::ecs::EntityId;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique task definition identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u32);

/// Unique station identifier — corresponds to a TriggerZone id in the tilemap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StationId(pub u32);

// ---------------------------------------------------------------------------
// Task definitions
// ---------------------------------------------------------------------------

/// Who is eligible to perform a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEligibility {
    /// Any player can perform this task.
    Anyone,
    /// Only players on a given team (matches `LobbyPlayer.team`).
    Team(u8),
    /// Only specific entities can perform this task.
    Entities(Vec<EntityId>),
}

/// Definition of a task players can perform at a station.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDef {
    pub id: TaskId,
    pub name: String,
    /// Which station this task is performed at.
    pub station_id: StationId,
    /// Time in seconds to complete the task. 0.0 means instant.
    pub duration: f32,
    /// Who can perform this task.
    pub eligibility: TaskEligibility,
    /// Whether the task can be performed more than once after completion.
    pub repeatable: bool,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Runtime status of a single task instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is available but nobody is working on it.
    Available,
    /// A player is actively working on it.
    InProgress { worker: EntityId, progress: f32 },
    /// Task was completed.
    Completed { completed_by: EntityId },
}

/// A live task instance in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskInstance {
    pub def_id: TaskId,
    pub station_id: StationId,
    pub status: TaskStatus,
    pub duration: f32,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events produced by the task system.
#[derive(Clone, Debug)]
pub enum TaskEvent {
    Started {
        task_id: TaskId,
        station_id: StationId,
        worker: EntityId,
    },
    Progressed {
        task_id: TaskId,
        station_id: StationId,
        worker: EntityId,
        progress: f32,
    },
    Interrupted {
        task_id: TaskId,
        station_id: StationId,
        worker: EntityId,
    },
    Completed {
        task_id: TaskId,
        station_id: StationId,
        worker: EntityId,
    },
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Registry holding all task definitions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskRegistry {
    defs: FxHashMap<TaskId, TaskDef>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: TaskDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: TaskId) -> Option<&TaskDef> {
        self.defs.get(&id)
    }

    pub fn by_station(&self, station_id: StationId) -> Vec<&TaskDef> {
        self.defs
            .values()
            .filter(|d| d.station_id == station_id)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Task state manager
// ---------------------------------------------------------------------------

/// Manages all live task instances.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskState {
    tasks: Vec<TaskInstance>,
}

impl TaskState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn task instances from all registered definitions.
    pub fn spawn_tasks(&mut self, registry: &TaskRegistry) {
        self.tasks.clear();
        for def in registry.defs.values() {
            self.tasks.push(TaskInstance {
                def_id: def.id,
                station_id: def.station_id,
                status: TaskStatus::Available,
                duration: def.duration,
            });
        }
    }

    /// Player begins working on a task at a station. Returns false if invalid.
    pub fn begin_task(
        &mut self,
        station_id: StationId,
        worker: EntityId,
        team: u8,
        registry: &TaskRegistry,
    ) -> bool {
        let Some(task) = self
            .tasks
            .iter_mut()
            .find(|t| t.station_id == station_id && matches!(t.status, TaskStatus::Available))
        else {
            return false;
        };

        // Check eligibility.
        if let Some(def) = registry.get(task.def_id) {
            match &def.eligibility {
                TaskEligibility::Anyone => {}
                TaskEligibility::Team(required) => {
                    if *required != team {
                        return false;
                    }
                }
                TaskEligibility::Entities(allowed) => {
                    if !allowed.contains(&worker) {
                        return false;
                    }
                }
            }
        }

        // Instant completion for zero-duration tasks.
        if task.duration <= 0.0 {
            task.status = TaskStatus::Completed {
                completed_by: worker,
            };
        } else {
            task.status = TaskStatus::InProgress {
                worker,
                progress: 0.0,
            };
        }

        true
    }

    /// Player stops working (moved away, died, etc). Resets progress to Available.
    pub fn interrupt(&mut self, worker: EntityId) {
        for task in &mut self.tasks {
            if let TaskStatus::InProgress {
                worker: w,
                progress: _,
            } = &task.status
            {
                if *w == worker {
                    task.status = TaskStatus::Available;
                }
            }
        }
    }

    /// Advance all in-progress tasks. Returns events.
    pub fn update(&mut self, dt: f32) -> Vec<TaskEvent> {
        let mut events = Vec::new();

        for task in &mut self.tasks {
            if let TaskStatus::InProgress {
                worker,
                ref mut progress,
            } = task.status
            {
                *progress += dt;

                if *progress >= task.duration {
                    events.push(TaskEvent::Completed {
                        task_id: task.def_id,
                        station_id: task.station_id,
                        worker,
                    });
                    task.status = TaskStatus::Completed {
                        completed_by: worker,
                    };
                } else {
                    events.push(TaskEvent::Progressed {
                        task_id: task.def_id,
                        station_id: task.station_id,
                        worker,
                        progress: *progress,
                    });
                }
            }
        }

        events
    }

    /// How many tasks are completed out of total.
    pub fn completion_count(&self) -> (u32, u32) {
        let total = self.tasks.len() as u32;
        let completed = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Completed { .. }))
            .count() as u32;
        (completed, total)
    }

    /// Check if all tasks are complete.
    pub fn all_complete(&self) -> bool {
        !self.tasks.is_empty()
            && self
                .tasks
                .iter()
                .all(|t| matches!(t.status, TaskStatus::Completed { .. }))
    }

    /// Get task at a specific station.
    pub fn task_at_station(&self, station_id: StationId) -> Option<&TaskInstance> {
        self.tasks.iter().find(|t| t.station_id == station_id)
    }

    /// Get all tasks as a slice.
    pub fn tasks(&self) -> &[TaskInstance] {
        &self.tasks
    }

    /// Reset all tasks to Available (for new round).
    pub fn reset(&mut self) {
        for task in &mut self.tasks {
            task.status = TaskStatus::Available;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn make_registry() -> TaskRegistry {
        let mut reg = TaskRegistry::new();
        reg.register(TaskDef {
            id: TaskId(1),
            name: "Fix wiring".into(),
            station_id: StationId(10),
            duration: 3.0,
            eligibility: TaskEligibility::Anyone,
            repeatable: false,
        });
        reg.register(TaskDef {
            id: TaskId(2),
            name: "Upload data".into(),
            station_id: StationId(20),
            duration: 5.0,
            eligibility: TaskEligibility::Team(0),
            repeatable: false,
        });
        reg
    }

    fn eid(n: u32) -> EntityId {
        EntityId::from_raw(n, 0)
    }

    #[test]
    fn spawn_and_complete() {
        let reg = make_registry();
        let mut state = TaskState::new();
        state.spawn_tasks(&reg);

        assert_eq!(state.completion_count(), (0, 2));

        assert!(state.begin_task(StationId(10), eid(1), 0, &reg));
        // Progress the task to completion.
        let events = state.update(4.0);
        assert!(events.iter().any(|e| matches!(
            e,
            TaskEvent::Completed {
                task_id: TaskId(1),
                ..
            }
        )));
        assert_eq!(state.completion_count(), (1, 2));
    }

    #[test]
    fn team_eligibility_denied() {
        let reg = make_registry();
        let mut state = TaskState::new();
        state.spawn_tasks(&reg);

        // Task 2 requires team 0; team 1 should be denied.
        assert!(!state.begin_task(StationId(20), eid(1), 1, &reg));
    }

    #[test]
    fn interrupt_resets_progress() {
        let reg = make_registry();
        let mut state = TaskState::new();
        state.spawn_tasks(&reg);

        assert!(state.begin_task(StationId(10), eid(1), 0, &reg));
        state.update(1.0); // partial progress
        state.interrupt(eid(1));

        let task = state.task_at_station(StationId(10)).unwrap();
        assert!(matches!(task.status, TaskStatus::Available));
    }

    #[test]
    fn all_complete() {
        let reg = make_registry();
        let mut state = TaskState::new();
        state.spawn_tasks(&reg);

        assert!(state.begin_task(StationId(10), eid(1), 0, &reg));
        state.update(4.0);
        assert!(state.begin_task(StationId(20), eid(2), 0, &reg));
        state.update(6.0);

        assert!(state.all_complete());
    }

    #[test]
    fn instant_task() {
        let mut reg = TaskRegistry::new();
        reg.register(TaskDef {
            id: TaskId(99),
            name: "Instant".into(),
            station_id: StationId(99),
            duration: 0.0,
            eligibility: TaskEligibility::Anyone,
            repeatable: false,
        });

        let mut state = TaskState::new();
        state.spawn_tasks(&reg);
        assert!(state.begin_task(StationId(99), eid(1), 0, &reg));
        assert!(matches!(
            state.task_at_station(StationId(99)).unwrap().status,
            TaskStatus::Completed { .. }
        ));
    }

    #[test]
    fn reset_clears_all() {
        let reg = make_registry();
        let mut state = TaskState::new();
        state.spawn_tasks(&reg);

        assert!(state.begin_task(StationId(10), eid(1), 0, &reg));
        state.update(4.0);

        state.reset();
        assert_eq!(state.completion_count(), (0, 2));
    }
}
