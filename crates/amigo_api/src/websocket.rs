//! WebSocket event streaming endpoint (RS-14).
//!
//! Provides a lightweight WebSocket-like event streaming server that pushes
//! engine events to connected clients in real-time instead of polling.
//! Uses a simple frame protocol over TCP for broad compatibility.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
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

/// Maximum number of simultaneously connected streaming clients.
///
/// Each client costs a socket and up to [`MAX_CLIENT_BACKLOG`] of buffered
/// output, so the count has to be bounded.
const MAX_CLIENTS: usize = 32;

/// Bytes that may pile up for one client before it is dropped as a slow
/// consumer. Reached only if a subscriber stops reading while events keep
/// coming.
const MAX_CLIENT_BACKLOG: usize = 1 << 20;

/// A connected event streaming client.
struct StreamClient {
    stream: TcpStream,
    filters: Vec<EventFilter>,
    id: u64,
    /// Bytes accepted for this client but not yet taken by the socket.
    ///
    /// Exists so that broadcasting never blocks: the socket is non-blocking and
    /// whatever it refuses stays here until the next flush.
    outbox: VecDeque<u8>,
}

impl StreamClient {
    /// Push as much of the outbox into the socket as it will take right now.
    ///
    /// Returns `false` if the client should be dropped: the peer is gone, or it
    /// is too far behind to keep buffering for.
    fn pump(&mut self) -> bool {
        while !self.outbox.is_empty() {
            let chunk = self.outbox.as_slices().0;
            match self.stream.write(chunk) {
                Ok(0) => return false,
                Ok(n) => {
                    self.outbox.drain(..n);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    debug!("Event client {} disconnected", self.id);
                    return false;
                }
            }
        }

        if self.outbox.len() > MAX_CLIENT_BACKLOG {
            warn!(
                "Dropping event client {}: {} bytes backlogged, not keeping up",
                self.id,
                self.outbox.len()
            );
            return false;
        }

        true
    }
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
    ///
    /// Never blocks, however slow or unresponsive a subscriber is. This runs on
    /// the engine thread with the shared state locked, so a blocking
    /// `write_all` here would stall the game for as long as one subscriber's
    /// socket buffer stays full. Events are encoded into per-client outboxes
    /// and pushed through non-blocking sockets; a client that falls more than
    /// [`MAX_CLIENT_BACKLOG`] behind is dropped.
    pub fn flush(&mut self) {
        if !self.pending_events.is_empty() {
            let events: Vec<EngineEvent> = self.pending_events.drain(..).collect();

            // Encode once per event rather than once per (event, client).
            let encoded: Vec<(String, String)> = events
                .into_iter()
                .filter_map(|event| match serde_json::to_string(&event) {
                    Ok(mut json) => {
                        json.push('\n');
                        Some((event.event_type, json))
                    }
                    Err(e) => {
                        warn!("Dropping unserializable engine event: {}", e);
                        None
                    }
                })
                .collect();

            for client in &mut self.clients {
                for (event_type, json) in &encoded {
                    if client.filters.iter().any(|f| f.matches(event_type)) {
                        client.outbox.extend(json.as_bytes());
                    }
                }
            }
        }

        // Drain outboxes as far as the sockets allow, dropping dead and
        // hopelessly-behind clients.
        self.clients.retain_mut(|client| client.pump());
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
                // Read optional subscription, then add to clients. Reading
                // happens before the lock is taken so a silent client delays
                // only this accept loop, never the engine thread.
                let filters = read_subscription(&stream);

                // Broadcasting must not block, so the client socket is
                // non-blocking and `StreamClient::pump` buffers the remainder.
                if let Err(e) = stream.set_nonblocking(true) {
                    warn!("Rejecting event stream client: {}", e);
                    continue;
                }

                let mut state = crate::lock_or_recover(&stream_state);
                if state.clients.len() >= MAX_CLIENTS {
                    warn!(
                        "Rejecting event stream client from {}: {} clients already connected",
                        addr, MAX_CLIENTS
                    );
                    continue;
                }
                let id = state.next_id;
                state.next_id += 1;
                state.clients.push(StreamClient {
                    stream,
                    filters,
                    id,
                    outbox: VecDeque::new(),
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

    // ── Event filtering ────────────────────────────────────────────

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

    // ── Stream state ──────────────────────────────────────────────

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

    // ── Server lifecycle ───────────────────────────────────────────

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

    // ── Slow consumers ────────────────────────────────────────────

    /// Register a client whose peer never reads anything.
    ///
    /// Returns the state plus the peer socket, which must stay alive for the
    /// connection to remain open.
    fn state_with_silent_client() -> (EventStreamState, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // The peer connects and then never reads a single byte.
        let peer = TcpStream::connect(addr).unwrap();
        let (server_side, _) = listener.accept().unwrap();
        server_side.set_nonblocking(true).unwrap();

        let mut state = EventStreamState::new();
        state.clients.push(StreamClient {
            stream: server_side,
            filters: vec![EventFilter::All],
            id: 1,
            outbox: VecDeque::new(),
        });
        (state, peer)
    }

    /// One event carrying a 64 KiB payload, so that socket buffers — which the
    /// kernel autotunes into the megabytes on loopback — fill in a reasonable
    /// number of rounds.
    fn big_event(i: u64) -> EngineEvent {
        EngineEvent {
            event_type: "entity.spawn".into(),
            tick: i,
            data: serde_json::json!({ "blob": "x".repeat(64 * 1024) }),
        }
    }

    /// Feed the state one big event at a time until `stop` holds, on a worker
    /// thread with a deadline.
    ///
    /// The deadline is the point of the exercise: with a blocking `write_all`,
    /// `flush` never returns once the peer stops reading, so a same-thread test
    /// would hang instead of failing. Returns the final state for further
    /// assertions.
    ///
    /// # Panics
    /// If the pumping thread makes no progress before the deadline, or panics.
    fn pump_until_with_deadline(
        mut state: EventStreamState,
        budget_rounds: usize,
        stop: impl Fn(&EventStreamState) -> bool + Send + 'static,
    ) -> EventStreamState {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reached = None;
            for round in 0..budget_rounds {
                state.push_event(big_event(round as u64));
                state.flush();
                if stop(&state) {
                    reached = Some(round);
                    break;
                }
            }
            let _ = tx.send((reached, state));
        });

        match rx.recv_timeout(std::time::Duration::from_secs(20)) {
            Ok((Some(_), state)) => state,
            Ok((None, state)) => panic!(
                "condition not reached within {} rounds ({} clients left, {} bytes buffered)",
                budget_rounds,
                state.client_count(),
                state.clients.first().map_or(0, |c| c.outbox.len())
            ),
            Err(_) => panic!(
                "flush made no progress within the deadline on a client that never reads \
                 — it is blocking on the socket instead of buffering"
            ),
        }
    }

    /// A subscriber that stops reading must not stall the caller. `flush` runs
    /// on the engine thread with the shared state locked, so a blocking
    /// `write_all` here froze the game for as long as the socket stayed full.
    ///
    /// Once the kernel refuses more data, unsent bytes have to land in the
    /// outbox — that is the observable difference between buffering and
    /// blocking.
    #[test]
    fn flush_does_not_block_on_a_silent_client() {
        let (state, _peer) = state_with_silent_client();

        // Stop as soon as the socket refuses data: unsent bytes landing in the
        // outbox is the observable difference between buffering and blocking.
        let state = pump_until_with_deadline(state, 512, |s| {
            s.clients.first().is_some_and(|c| !c.outbox.is_empty())
        });

        assert_eq!(state.client_count(), 1, "client should still be connected");
    }

    /// Buffering for a silent client is bounded: past the backlog cap the client
    /// is dropped instead of growing the outbox without limit.
    #[test]
    fn hopelessly_slow_client_is_dropped() {
        let (state, _peer) = state_with_silent_client();

        let state = pump_until_with_deadline(state, 1024, |s| s.client_count() == 0);

        assert_eq!(state.client_count(), 0);
    }

    /// End-to-end version of the above: the client goes through the real accept
    /// path, so this also covers the server putting the socket into
    /// non-blocking mode. Broadcasting from the engine side must stay
    /// responsive even though the subscriber never reads.
    #[test]
    fn server_side_broadcast_survives_a_silent_subscriber() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let stream_state = new_event_stream();
        let mut server = EventStreamServer::start(port, stream_state.clone()).unwrap();

        // Subscribe to everything, then never read a byte.
        let mut peer = TcpStream::connect(("127.0.0.1", port)).unwrap();
        writeln!(peer, "{{\"filters\":[]}}").unwrap();

        // Wait for the accept loop to register the client.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if crate::lock_or_recover(&stream_state).client_count() == 1 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "client was never registered"
            );
            thread::sleep(std::time::Duration::from_millis(20));
        }

        // Broadcast from a worker so a regression fails on the deadline rather
        // than hanging the test binary.
        let (tx, rx) = std::sync::mpsc::channel();
        let broadcast_state = stream_state.clone();
        thread::spawn(move || {
            for round in 0..512 {
                let mut state = crate::lock_or_recover(&broadcast_state);
                state.push_event(big_event(round));
                state.flush();
                let backpressured = state.clients.first().is_some_and(|c| !c.outbox.is_empty());
                let dropped = state.clients.is_empty();
                drop(state);
                if backpressured || dropped {
                    let _ = tx.send(true);
                    return;
                }
            }
            let _ = tx.send(false);
        });

        let progressed = rx
            .recv_timeout(std::time::Duration::from_secs(20))
            .unwrap_or_else(|_| {
                panic!(
                    "broadcasting stalled on a subscriber that never reads — \
                     the server is writing in blocking mode"
                )
            });
        assert!(
            progressed,
            "socket never applied backpressure; cannot distinguish buffering from blocking"
        );

        server.stop();
        drop(peer);
    }

    #[test]
    fn events_are_filtered_per_client_outbox() {
        let (mut state, _peer) = state_with_silent_client();
        state.clients[0].filters = vec![EventFilter::Exact("scene.change".into())];

        state.push_event(EngineEvent {
            event_type: "entity.spawn".into(),
            tick: 1,
            data: serde_json::json!({}),
        });
        state.flush();

        assert!(
            state.clients[0].outbox.is_empty(),
            "non-matching event should not be queued for this client"
        );
    }
}
