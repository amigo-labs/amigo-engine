use serde::{Deserialize, Serialize};

/// Unique identifier for an entity, with generational index for safe reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl EntityId {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({}v{})", self.index, self.generation)
    }
}

/// Generational arena for entity allocation and deallocation.
pub struct GenerationalArena {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free_list: Vec<u32>,
    count: usize,
}

impl GenerationalArena {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            free_list: Vec::new(),
            count: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            generations: Vec::with_capacity(capacity),
            alive: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            count: 0,
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        self.count += 1;
        if let Some(index) = self.free_list.pop() {
            self.generations[index as usize] += 1;
            self.alive[index as usize] = true;
            EntityId {
                index,
                generation: self.generations[index as usize],
            }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(0);
            self.alive.push(true);
            EntityId {
                index,
                generation: 0,
            }
        }
    }

    pub fn despawn(&mut self, id: EntityId) -> bool {
        let idx = id.index as usize;
        if idx < self.alive.len()
            && self.alive[idx]
            && self.generations[idx] == id.generation
        {
            self.alive[idx] = false;
            self.free_list.push(id.index);
            self.count -= 1;
            true
        } else {
            false
        }
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        let idx = id.index as usize;
        idx < self.alive.len()
            && self.alive[idx]
            && self.generations[idx] == id.generation
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn iter_alive(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter(|(_, &alive)| alive)
            .map(|(i, _)| EntityId {
                index: i as u32,
                generation: self.generations[i],
            })
    }
}

impl Default for GenerationalArena {
    fn default() -> Self {
        Self::new()
    }
}
