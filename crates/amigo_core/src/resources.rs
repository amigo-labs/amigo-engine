use std::any::{Any, TypeId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Typed resource map
// ---------------------------------------------------------------------------

/// A type-erased container for game resources (singletons).
///
/// Resources are unique per type — at most one instance of each type.
/// This allows games to store custom data accessible from the game context
/// without modifying engine code.
///
/// # Example
/// ```
/// use amigo_core::resources::Resources;
///
/// struct PlayerStats { health: i32, gold: i32 }
/// struct GameSettings { difficulty: u8 }
///
/// let mut res = Resources::new();
/// res.insert(PlayerStats { health: 100, gold: 50 });
/// res.insert(GameSettings { difficulty: 2 });
///
/// let stats = res.get::<PlayerStats>().unwrap();
/// assert_eq!(stats.health, 100);
///
/// res.get_mut::<PlayerStats>().unwrap().gold += 10;
/// assert_eq!(res.get::<PlayerStats>().unwrap().gold, 60);
/// ```
pub struct Resources {
    data: HashMap<TypeId, Box<dyn Any>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a resource. Replaces any existing resource of the same type.
    pub fn insert<T: 'static>(&mut self, resource: T) {
        self.data.insert(TypeId::of::<T>(), Box::new(resource));
    }

    /// Get a shared reference to a resource.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Get a mutable reference to a resource.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.data
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Get a resource, inserting a default if not present.
    pub fn get_or_insert_with<T: 'static>(&mut self, f: impl FnOnce() -> T) -> &mut T {
        self.data
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(f()))
            .downcast_mut::<T>()
            .unwrap()
    }

    /// Get a resource, inserting `Default::default()` if not present.
    pub fn get_or_default<T: 'static + Default>(&mut self) -> &mut T {
        self.get_or_insert_with(T::default)
    }

    /// Remove a resource and return it.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.data
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Check if a resource of the given type exists.
    pub fn contains<T: 'static>(&self) -> bool {
        self.data.contains_key(&TypeId::of::<T>())
    }

    /// Number of stored resources.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the container is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Remove all resources.
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Health(i32);
    struct Gold(i32);

    #[derive(Default)]
    struct Score(u64);

    #[test]
    fn insert_and_get() {
        let mut res = Resources::new();
        res.insert(Health(100));
        res.insert(Gold(50));

        assert_eq!(res.get::<Health>().unwrap().0, 100);
        assert_eq!(res.get::<Gold>().unwrap().0, 50);
    }

    #[test]
    fn get_mut() {
        let mut res = Resources::new();
        res.insert(Health(100));

        res.get_mut::<Health>().unwrap().0 -= 20;
        assert_eq!(res.get::<Health>().unwrap().0, 80);
    }

    #[test]
    fn missing_returns_none() {
        let res = Resources::new();
        assert!(res.get::<Health>().is_none());
    }

    #[test]
    fn replace_existing() {
        let mut res = Resources::new();
        res.insert(Health(100));
        res.insert(Health(50));
        assert_eq!(res.get::<Health>().unwrap().0, 50);
    }

    #[test]
    fn remove_resource() {
        let mut res = Resources::new();
        res.insert(Health(100));
        let removed = res.remove::<Health>().unwrap();
        assert_eq!(removed.0, 100);
        assert!(!res.contains::<Health>());
    }

    #[test]
    fn get_or_default() {
        let mut res = Resources::new();
        let score = res.get_or_default::<Score>();
        assert_eq!(score.0, 0);

        score.0 = 42;
        assert_eq!(res.get::<Score>().unwrap().0, 42);
    }

    #[test]
    fn len_and_empty() {
        let mut res = Resources::new();
        assert!(res.is_empty());

        res.insert(Health(1));
        assert_eq!(res.len(), 1);

        res.insert(Gold(2));
        assert_eq!(res.len(), 2);
    }
}
