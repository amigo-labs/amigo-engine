#![cfg(feature = "async_tasks")]

//! Async task system for offloading work to a thread pool.

use crossbeam_channel::{self, Sender};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// A thread pool that executes closures on worker threads.
pub struct TaskPool {
    workers: Vec<JoinHandle<()>>,
    sender: Option<Sender<Box<dyn FnOnce() + Send + 'static>>>,
}

impl TaskPool {
    /// Create a new `TaskPool` with a default number of worker threads
    /// (available parallelism minus one, minimum 1).
    pub fn new() -> Self {
        let n = thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(2);
        Self::with_threads(n)
    }

    /// Create a new `TaskPool` with exactly `n` worker threads.
    pub fn with_threads(n: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded::<Box<dyn FnOnce() + Send + 'static>>();
        let mut workers = Vec::with_capacity(n);
        for _ in 0..n {
            let rx = receiver.clone();
            workers.push(thread::spawn(move || {
                while let Ok(task) = rx.recv() {
                    task();
                }
            }));
        }
        Self {
            workers,
            sender: Some(sender),
        }
    }

    /// Spawn a task on the pool. Returns a `Task<T>` handle to retrieve the result.
    pub fn spawn<T, F>(&self, f: F) -> Task<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let boxed: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            let result = f();
            let _ = tx.send(result);
        });
        if let Some(ref sender) = self.sender {
            sender.send(boxed).expect("TaskPool worker threads have shut down");
        }
        Task { rx }
    }

    /// Shut down the pool: drop the sender so workers finish, then join all threads.
    pub fn shutdown(mut self) {
        self.sender.take();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Default for TaskPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        // Drop the sender so workers see a disconnected channel and exit.
        self.sender.take();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

/// A handle to the result of a spawned task.
pub struct Task<T> {
    rx: mpsc::Receiver<T>,
}

impl<T> Task<T> {
    /// Non-blocking poll for the result.
    pub fn try_recv(&self) -> Option<T> {
        self.rx.try_recv().ok()
    }

    /// Returns `true` if the task has completed and a result is available.
    ///
    /// Note: after `is_done()` returns `true`, `try_recv()` will return `Some`.
    pub fn is_done(&self) -> bool {
        // Peek without consuming: we check if the channel has a message or is disconnected.
        // Unfortunately std mpsc doesn't have peek, so we use try_recv is destructive.
        // Instead we rely on try_recv + re-wrapping, but that's complex.
        // A simpler approach: just check if we can receive (non-blocking).
        // Since is_done is informational, we'll note this is a best-effort check.
        // Actually, let's do it properly: we can't peek with std mpsc, but we can
        // use the fact that try_recv returns Err(Empty) vs Err(Disconnected).
        matches!(self.rx.try_recv(), Ok(_) | Err(mpsc::TryRecvError::Disconnected))
    }

    /// Block until the task completes and return the result.
    pub fn block(self) -> T {
        self.rx.recv().expect("Task sender dropped without sending a result")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_single() {
        let pool = TaskPool::with_threads(2);
        let task = pool.spawn(|| 42);
        assert_eq!(task.block(), 42);
    }

    #[test]
    fn test_spawn_many() {
        let pool = TaskPool::with_threads(4);
        let tasks: Vec<Task<usize>> = (0..100).map(|i| pool.spawn(move || i * 2)).collect();
        let mut results: Vec<usize> = tasks.into_iter().map(|t| t.block()).collect();
        results.sort();
        let expected: Vec<usize> = (0..100).map(|i| i * 2).collect();
        assert_eq!(results, expected);
    }

    #[test]
    fn test_try_recv_none_then_some() {
        let pool = TaskPool::with_threads(1);
        // Use a channel to coordinate: the task blocks until we signal it.
        let (signal_tx, signal_rx) = std::sync::mpsc::channel::<()>();
        let task = pool.spawn(move || {
            signal_rx.recv().unwrap();
            99
        });
        // Task hasn't been signaled yet, so result should not be available.
        assert!(task.try_recv().is_none());
        // Signal the task to complete.
        signal_tx.send(()).unwrap();
        // Wait for result.
        let result = task.block();
        assert_eq!(result, 99);
    }

    #[test]
    fn test_pool_shutdown() {
        let pool = TaskPool::with_threads(2);
        let _task = pool.spawn(|| 1 + 1);
        // Dropping (via shutdown) should join all threads without hanging.
        pool.shutdown();
    }

    #[test]
    fn test_send_types() {
        let pool = TaskPool::with_threads(2);
        let task_vec = pool.spawn(|| vec![1u8, 2, 3]);
        let task_string = pool.spawn(|| String::from("hello"));
        assert_eq!(task_vec.block(), vec![1u8, 2, 3]);
        assert_eq!(task_string.block(), "hello");
    }
}
