use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// A per-tick queue that collects commands and drains them for processing.
#[derive(Debug)]
pub struct CommandQueue<T> {
    commands: Vec<T>,
}

impl<T> Default for CommandQueue<T> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

impl<T> CommandQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a command to be processed on the next drain.
    pub fn push(&mut self, cmd: T) {
        self.commands.push(cmd);
    }

    /// Drain all queued commands, returning them in insertion order.
    pub fn drain(&mut self) -> Vec<T> {
        std::mem::take(&mut self.commands)
    }

    /// Returns `true` if no commands are queued.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the number of queued commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }
}

/// A log of all commands paired with the tick they were issued on,
/// enabling deterministic replay of a game session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandLog<T> {
    entries: Vec<(u64, T)>,
}

impl<T> Default for CommandLog<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> CommandLog<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a command that was executed at the given tick.
    pub fn record(&mut self, tick: u64, cmd: T) {
        self.entries.push((tick, cmd));
    }

    /// Iterate over all recorded (tick, command) pairs in order.
    pub fn iter(&self) -> impl Iterator<Item = &(u64, T)> {
        self.entries.iter()
    }

    /// Returns the total number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestCommand {
        DoSomething,
        SetValue { value: u32 },
    }

    #[test]
    fn command_queue_push_and_drain() {
        let mut queue = CommandQueue::new();
        assert!(queue.is_empty());

        queue.push(TestCommand::DoSomething);
        queue.push(TestCommand::SetValue { value: 42 });
        assert_eq!(queue.len(), 2);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn command_log_record_and_iter() {
        let mut log = CommandLog::new();
        log.record(0, TestCommand::DoSomething);
        log.record(5, TestCommand::SetValue { value: 2 });

        let entries: Vec<_> = log.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, 0);
        assert_eq!(entries[1].0, 5);
    }
}
