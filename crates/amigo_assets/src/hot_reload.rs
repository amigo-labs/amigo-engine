use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use tracing::{info, warn};

/// Watches the assets directory for changes and notifies the engine.
pub struct HotReloader {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<PathBuf>,
}

impl HotReloader {
    pub fn new(watch_path: PathBuf) -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        for path in event.paths {
                            let _ = tx.send(path);
                        }
                    }
                }
                Err(e) => {
                    warn!("File watcher error: {}", e);
                }
            })
            .ok()?;

        if watcher
            .watch(&watch_path, RecursiveMode::Recursive)
            .is_err()
        {
            warn!("Failed to watch directory: {:?}", watch_path);
            return None;
        }

        info!("Hot reload watching: {:?}", watch_path);
        Some(Self {
            _watcher: watcher,
            rx,
        })
    }

    /// Drain all pending file change notifications.
    pub fn poll_changes(&self) -> Vec<PathBuf> {
        let mut changes = Vec::new();
        while let Ok(path) = self.rx.try_recv() {
            changes.push(path);
        }
        changes
    }
}
