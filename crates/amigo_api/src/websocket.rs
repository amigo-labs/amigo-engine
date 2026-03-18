//! WebSocket event streaming endpoint (RS-14).
//!
//! Provides a lightweight WebSocket-like event streaming server that pushes
//! engine events to connected clients in real-time instead of polling.
//! Uses a simple frame protocol over TCP for broad compatibility.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, info, warn};

/// An engine event that can be streamed to connected clients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineEvent {
    /// Event type for filtering (e.g. "entity.spawn", "scene.change", "audio.section").
    pub event_type: String,
    /// The tick at which this event occurred.
    pub tick: u64,
    /// Event payload as a JSON value.
    pub data: serde_json::Value,
}

/// Event types that clients can subscribe to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventFilter {
    /// All events.
    All,
    /// Only events matching this prefix (e.g. "entity" matches "entity.spawn").
    Prefix(String),
    /// Only events matching this exact type.
    Exact(String),
}

impl EventFilter {
    pub fn matches(&self, event_type: &str) -> bool {
        match self {
            EventFilter::All => true,
            EventFilter::Prefix(p) => event_type.starts_with(p.as_str()),
            EventFilter::Exact(e) => event_type == e,
        }
    }
}

/// A connected event streaming client.
struct StreamClient {
    stream: TcpStream,
    filters: Vec<EventFilter>,
    id: u64,
}

/// Shared state for the event streaming server.
pub struct EventStreamState {
    clients: Vec<StreamClient>,
    next_id: u64,
    /// Pending events to broadcast.
    pub pending_events: Vec<EngineEvent>,
}

impl EventStreamState {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
            next_id: 1,
            pending_events: Vec::new(),
        }
    }

    /// Queue an event for broadcasting to subscribed clients.
    pub fn push_event(&mut self, event: EngineEvent) {
        self.pending_events.push(event);
    }

    /// Broadcast all pending events to connected clients, then clear.
    pub fn flush(&mut self) {
        if self.pending_events.is_empty() {
            return;
        }

        let events: Vec<EngineEvent> = self.pending_events.drain(..).collect();

        // Broadcast to each client, removing disconnected ones
        self.clients.retain_mut(|client| {
            for event in &events {
                if client.filters.iter().any(|f| f.matches(&event.event_type)) {
                    let mut json = match serde_json::to_string(event) {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    json.push('\n');
                    if client.stream.write_all(json.as_bytes()).is_err() {
                        debug!("Event client {} disconnected", client.id);
                        return false;
                    }
                }
            }
            true
        });
    }

    /// Number of connected streaming clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

impl Default for EventStreamState {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared handle to the event stream state.
pub type SharedEventStream = Arc<Mutex<EventStreamState>>;

/// Create a new shared event stream state.
pub fn new_event_stream() -> SharedEventStream {
    Arc::new(Mutex::new(EventStreamState::new()))
}

/// Event streaming server that accepts TCP connections.
/// Clients send a JSON subscription message, then receive events as newline-delimited JSON.
pub struct EventStreamServer {
    pub port: u16,
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

/// Subscription request from a client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscribeRequest {
    /// Which event types to receive. Empty = all.
    #[serde(default)]
    pub filters: Vec<String>,
}

impl EventStreamServer {
    /// Start the event streaming server on the given port.
    pub fn start(port: u16, stream_state: SharedEventStream) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr)?;
        listener.set_nonblocking(true)?;
        info!("Event stream server listening on {}", addr);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let thread = thread::spawn(move || {
            run_stream_server(listener, stream_state, running_clone);
        });

        Ok(Self {
            port,
            running,
            thread: Some(thread),
        })
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for EventStreamServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_stream_server(
    listener: TcpListener,
    stream_state: SharedEventStream,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                debug!("Event stream client connected from {}", addr);
                // Read optional subscription, then add to clients
                let filters = read_subscription(&stream);
                let mut state = stream_state.lock().unwrap();
                let id = state.next_id;
                state.next_id += 1;
                state.clients.push(StreamClient {
                    stream,
                    filters,
                    id,
                });
                info!(
                    "Event stream client {} registered ({} total)",
                    id,
                    state.clients.len()
                );
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                warn!("Event stream accept error: {}", e);
            }
        }
    }
}

fn read_subscription(stream: &TcpStream) -> Vec<EventFilter> {
    // Try to read a subscription message with a short timeout.
    // If nothing arrives, subscribe to all events.
    use std::io::{BufRead, BufReader};

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    match reader.read_line(&mut line) {
        Ok(n) if n > 0 => {
            if let Ok(req) = serde_json::from_str::<SubscribeRequest>(&line) {
                if req.filters.is_empty() {
                    vec![EventFilter::All]
                } else {
                    req.filters
                        .into_iter()
                        .map(|f| {
                            if f == "*" {
                                EventFilter::All
                            } else if f.ends_with('*') {
                                EventFilter::Prefix(f.trim_end_matches('*').to_string())
                            } else {
                                EventFilter::Exact(f)
                            }
                        })
                        .collect()
                }
            } else {
                vec![EventFilter::All]
            }
        }
        _ => vec![EventFilter::All],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_filter_all() {
        let f = EventFilter::All;
        assert!(f.matches("entity.spawn"));
        assert!(f.matches("audio.section"));
    }

    #[test]
    fn event_filter_prefix() {
        let f = EventFilter::Prefix("entity".into());
        assert!(f.matches("entity.spawn"));
        assert!(f.matches("entity.destroy"));
        assert!(!f.matches("audio.section"));
    }

    #[test]
    fn event_filter_exact() {
        let f = EventFilter::Exact("scene.change".into());
        assert!(f.matches("scene.change"));
        assert!(!f.matches("scene.load"));
    }

    #[test]
    fn event_stream_state_push_and_count() {
        let mut state = EventStreamState::new();
        assert_eq!(state.client_count(), 0);
        state.push_event(EngineEvent {
            event_type: "test".into(),
            tick: 1,
            data: serde_json::json!({}),
        });
        assert_eq!(state.pending_events.len(), 1);
    }

    #[test]
    fn flush_clears_events() {
        let mut state = EventStreamState::new();
        state.push_event(EngineEvent {
            event_type: "test".into(),
            tick: 1,
            data: serde_json::json!({}),
        });
        state.flush();
        assert!(state.pending_events.is_empty());
    }

    #[test]
    fn server_start_stop() {
        let stream_state = new_event_stream();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut server = EventStreamServer::start(port, stream_state).unwrap();
        assert!(server.is_running());
        server.stop();
        assert!(!server.is_running());
    }
}
