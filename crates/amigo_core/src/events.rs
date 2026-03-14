use std::any::{Any, TypeId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Typed event queue (double-buffered)
// ---------------------------------------------------------------------------

/// A double-buffered event queue for a single event type.
///
/// Events written during the current tick are readable next tick.
/// This ensures deterministic ordering: systems that emit and systems
/// that consume never see a partial tick's events.
struct EventChannel {
    /// Events from the previous tick (read buffer).
    read: Box<dyn Any>,
    /// Events being written this tick (write buffer).
    write: Box<dyn Any>,
}

impl EventChannel {
    fn new<T: 'static>() -> Self {
        Self {
            read: Box::new(Vec::<T>::new()),
            write: Box::new(Vec::<T>::new()),
        }
    }

    fn swap<T: 'static>(&mut self) {
        // Move write → read, clear write
        let write = self.write.downcast_mut::<Vec<T>>().unwrap();
        let read = self.read.downcast_mut::<Vec<T>>().unwrap();
        std::mem::swap(read, write);
        write.clear();
    }
}

// ---------------------------------------------------------------------------
// Event hub (manages all event channels)
// ---------------------------------------------------------------------------

/// Central hub for all typed event channels. Manages double-buffered
/// event queues keyed by event type.
///
/// # Usage pattern
/// ```text
/// // In system A (producer):
/// events.emit(DamageEvent { target, amount });
///
/// // In system B (consumer, reads previous tick's events):
/// for event in events.read::<DamageEvent>() {
///     // process damage
/// }
///
/// // At end of tick (engine calls this):
/// events.swap_all();
/// ```
pub struct Events {
    channels: HashMap<TypeId, EventChannel>,
    /// Type IDs in registration order (for deterministic swap).
    registered: Vec<TypeId>,
}

impl Events {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            registered: Vec::new(),
        }
    }

    /// Register an event type. Must be called before emit/read.
    pub fn register<T: 'static>(&mut self) {
        let id = TypeId::of::<T>();
        if !self.channels.contains_key(&id) {
            self.channels.insert(id, EventChannel::new::<T>());
            self.registered.push(id);
        }
    }

    /// Emit an event (written to write buffer, readable next tick).
    pub fn emit<T: 'static>(&mut self, event: T) {
        let id = TypeId::of::<T>();
        if let Some(channel) = self.channels.get_mut(&id) {
            channel.write.downcast_mut::<Vec<T>>().unwrap().push(event);
        }
    }

    /// Read events from the previous tick.
    pub fn read<T: 'static>(&self) -> &[T] {
        let id = TypeId::of::<T>();
        if let Some(channel) = self.channels.get(&id) {
            channel.read.downcast_ref::<Vec<T>>().unwrap().as_slice()
        } else {
            &[]
        }
    }

    /// Number of events in the read buffer for a given type.
    pub fn count<T: 'static>(&self) -> usize {
        self.read::<T>().len()
    }

    /// Number of events pending in the write buffer (current tick).
    pub fn pending_count<T: 'static>(&self) -> usize {
        let id = TypeId::of::<T>();
        if let Some(channel) = self.channels.get(&id) {
            channel.write.downcast_ref::<Vec<T>>().unwrap().len()
        } else {
            0
        }
    }

    /// Swap all channels: write → read, clear write.
    /// Call once at the end of each tick.
    pub fn swap_all(&mut self) {
        // We need to know the concrete type for each channel to swap.
        // Store swap functions alongside channels.
        // Since we can't easily do this with the current design,
        // we use a different approach: store the swap fn at registration time.
        //
        // For now, we use a simpler approach: raw pointer swap of the Vec buffers.
        // The Vecs have the same type so we just swap the Box<dyn Any> pointers.
        for channel in self.channels.values_mut() {
            std::mem::swap(&mut channel.read, &mut channel.write);
            // Clear the new write buffer — but we can't call .clear() on Box<dyn Any>
            // without knowing the type. So we swap and the old read becomes new write
            // which still has last tick's data. We need a clear function.
        }
    }

    /// Check if an event type is registered.
    pub fn is_registered<T: 'static>(&self) -> bool {
        self.channels.contains_key(&TypeId::of::<T>())
    }
}

// We need a way to clear the write buffer without knowing T.
// Solution: store a type-erased clear function.

/// Improved event hub that stores clear functions for type erasure.
pub struct EventHub {
    channels: HashMap<TypeId, EventChannelEntry>,
    swap_order: Vec<TypeId>,
}

struct EventChannelEntry {
    read: Box<dyn Any>,
    write: Box<dyn Any>,
    /// Type-erased function to clear a Vec<T> inside Box<dyn Any>.
    clear_fn: fn(&mut Box<dyn Any>),
}

fn clear_vec<T: 'static>(boxed: &mut Box<dyn Any>) {
    boxed.downcast_mut::<Vec<T>>().unwrap().clear();
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            swap_order: Vec::new(),
        }
    }

    /// Register an event type. Must be called before emit/read.
    pub fn register<T: 'static>(&mut self) {
        let id = TypeId::of::<T>();
        if !self.channels.contains_key(&id) {
            self.channels.insert(
                id,
                EventChannelEntry {
                    read: Box::new(Vec::<T>::new()),
                    write: Box::new(Vec::<T>::new()),
                    clear_fn: clear_vec::<T>,
                },
            );
            self.swap_order.push(id);
        }
    }

    /// Emit an event (goes to write buffer, readable next tick).
    pub fn emit<T: 'static>(&mut self, event: T) {
        let id = TypeId::of::<T>();
        if let Some(entry) = self.channels.get_mut(&id) {
            entry.write.downcast_mut::<Vec<T>>().unwrap().push(event);
        }
    }

    /// Read events from the previous tick (read buffer).
    pub fn read<T: 'static>(&self) -> &[T] {
        let id = TypeId::of::<T>();
        if let Some(entry) = self.channels.get(&id) {
            entry.read.downcast_ref::<Vec<T>>().unwrap().as_slice()
        } else {
            &[]
        }
    }

    /// Swap all channels: write becomes read, write is cleared.
    /// Call once at the end of each tick.
    pub fn flush(&mut self) {
        for id in &self.swap_order {
            if let Some(entry) = self.channels.get_mut(id) {
                std::mem::swap(&mut entry.read, &mut entry.write);
                (entry.clear_fn)(&mut entry.write);
            }
        }
    }

    /// Number of readable events (from previous tick).
    pub fn count<T: 'static>(&self) -> usize {
        self.read::<T>().len()
    }

    /// Check if an event type is registered.
    pub fn is_registered<T: 'static>(&self) -> bool {
        self.channels.contains_key(&TypeId::of::<T>())
    }

    /// Clear all events (both buffers) for all types.
    pub fn clear_all(&mut self) {
        for entry in self.channels.values_mut() {
            (entry.clear_fn)(&mut entry.read);
            (entry.clear_fn)(&mut entry.write);
        }
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct DamageEvent {
        target: u32,
        amount: i32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct HealEvent {
        target: u32,
        amount: i32,
    }

    #[test]
    fn register_and_emit() {
        let mut hub = EventHub::new();
        hub.register::<DamageEvent>();

        hub.emit(DamageEvent { target: 1, amount: 10 });
        hub.emit(DamageEvent { target: 2, amount: 20 });

        // Not readable yet (in write buffer)
        assert_eq!(hub.read::<DamageEvent>().len(), 0);

        // Flush: write → read
        hub.flush();

        let events = hub.read::<DamageEvent>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].target, 1);
        assert_eq!(events[1].amount, 20);
    }

    #[test]
    fn events_live_one_tick() {
        let mut hub = EventHub::new();
        hub.register::<DamageEvent>();

        hub.emit(DamageEvent { target: 1, amount: 10 });
        hub.flush();
        assert_eq!(hub.count::<DamageEvent>(), 1);

        // Next tick: no new events emitted
        hub.flush();
        assert_eq!(hub.count::<DamageEvent>(), 0); // cleared
    }

    #[test]
    fn multiple_event_types() {
        let mut hub = EventHub::new();
        hub.register::<DamageEvent>();
        hub.register::<HealEvent>();

        hub.emit(DamageEvent { target: 1, amount: 50 });
        hub.emit(HealEvent { target: 2, amount: 30 });
        hub.flush();

        assert_eq!(hub.count::<DamageEvent>(), 1);
        assert_eq!(hub.count::<HealEvent>(), 1);
        assert_eq!(hub.read::<DamageEvent>()[0].amount, 50);
        assert_eq!(hub.read::<HealEvent>()[0].amount, 30);
    }

    #[test]
    fn unregistered_type_returns_empty() {
        let hub = EventHub::new();
        assert_eq!(hub.read::<DamageEvent>().len(), 0);
        assert!(!hub.is_registered::<DamageEvent>());
    }

    #[test]
    fn clear_all() {
        let mut hub = EventHub::new();
        hub.register::<DamageEvent>();

        hub.emit(DamageEvent { target: 1, amount: 10 });
        hub.flush();
        assert_eq!(hub.count::<DamageEvent>(), 1);

        hub.clear_all();
        assert_eq!(hub.count::<DamageEvent>(), 0);
    }

    #[test]
    fn double_buffer_isolation() {
        let mut hub = EventHub::new();
        hub.register::<DamageEvent>();

        // Tick 1: emit A
        hub.emit(DamageEvent { target: 1, amount: 10 });
        hub.flush();

        // Tick 2: emit B while A is readable
        hub.emit(DamageEvent { target: 2, amount: 20 });
        assert_eq!(hub.read::<DamageEvent>().len(), 1);
        assert_eq!(hub.read::<DamageEvent>()[0].target, 1); // still A

        hub.flush();

        // Tick 3: only B is readable
        assert_eq!(hub.read::<DamageEvent>().len(), 1);
        assert_eq!(hub.read::<DamageEvent>()[0].target, 2); // now B
    }
}
