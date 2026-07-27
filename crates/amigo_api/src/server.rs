use crate::handler::{handle_request, SharedState};
use crate::{RpcRequest, RpcResponse, INVALID_REQUEST, PARSE_ERROR};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, info, warn};

/// Maximum number of simultaneous client connections.
///
/// Each connection costs a thread, so an unbounded accept loop lets anything
/// that can reach the port exhaust the process.
const MAX_CONNECTIONS: usize = 16;

/// Maximum length of a single JSON-RPC request line, in bytes.
///
/// `BufRead::read_line` grows its buffer until it sees a newline, so without a
/// cap a client that sends an endless stream of non-newline bytes drives the
/// server out of memory.
const MAX_REQUEST_BYTES: usize = 1 << 20;

/// How long a client may take to deliver one complete request line.
///
/// The size cap alone bounds memory but not time: a client dribbling a byte at a
/// time would sit under the cap for days while holding one of the
/// [`MAX_CONNECTIONS`] slots. This bounds the wait instead.
const REQUEST_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether an in-progress request has taken longer than [`REQUEST_DEADLINE`].
///
/// `None` means no request is in flight, so an idle connection never expires.
fn exceeded_deadline(started: Option<std::time::Instant>) -> bool {
    started.is_some_and(|s| s.elapsed() > REQUEST_DEADLINE)
}

/// Send a JSON-RPC error and log why the connection is being closed.
fn reject(writer: &mut TcpStream, peer: Option<std::net::SocketAddr>, message: &str) {
    warn!("Closing API connection from {:?}: {}", peer, message);
    let response = RpcResponse::error(None, INVALID_REQUEST, message.to_string());
    if let Ok(mut json) = serde_json::to_string(&response) {
        json.push('\n');
        let _ = writer.write_all(json.as_bytes());
    }
}

/// TCP-based JSON-RPC server for AI agent control.
/// Runs on a background thread, communicates with the engine via SharedState.
pub struct ApiServer {
    /// The port actually bound. Differs from the requested port when 0 was
    /// passed, which asks the OS for an ephemeral port.
    pub port: u16,
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    /// Handles of the per-connection threads, so `stop` can wait for them.
    clients: Arc<Mutex<Vec<thread::JoinHandle<()>>>>,
}

impl ApiServer {
    /// Start the API server on the given port with the given shared state.
    pub fn start(port: u16, state: SharedState) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr)?;
        // Resolve the real port before spawning, so callers that passed 0 can
        // discover where to connect.
        let bound_port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        info!("API server listening on 127.0.0.1:{}", bound_port);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let clients = Arc::new(Mutex::new(Vec::new()));
        let clients_clone = clients.clone();

        let thread = thread::spawn(move || {
            run_server(listener, state, running_clone, clients_clone);
        });

        Ok(Self {
            port: bound_port,
            running,
            thread: Some(thread),
            clients,
        })
    }

    /// Stop the server gracefully.
    ///
    /// Waits for the accept loop *and* the per-connection threads: those hold a
    /// clone of the shared state, so returning while they still run would let
    /// them touch engine state after the caller believes the server is down.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        let handles: Vec<_> = std::mem::take(&mut *crate::lock_or_recover(&self.clients));
        for handle in handles {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_server(
    listener: TcpListener,
    state: SharedState,
    running: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<thread::JoinHandle<()>>>>,
) {
    let active = Arc::new(AtomicUsize::new(0));

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                    warn!(
                        "Rejecting API client from {}: {} connections already active",
                        addr, MAX_CONNECTIONS
                    );
                    // Dropping the stream closes it.
                    continue;
                }
                debug!("API client connected from {}", addr);
                let state = state.clone();
                let running = running.clone();
                let active = active.clone();
                active.fetch_add(1, Ordering::Relaxed);
                let handle = thread::spawn(move || {
                    handle_client(stream, state, running);
                    active.fetch_sub(1, Ordering::Relaxed);
                });
                // Keep the handle so `stop` can join it, and reap finished
                // threads so the list does not grow for the process lifetime.
                let mut guard = crate::lock_or_recover(&clients);
                guard.retain(|h| !h.is_finished());
                guard.push(handle);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connections, sleep briefly to avoid busy loop
                thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                warn!("API server accept error: {}", e);
            }
        }
    }
}

fn handle_client(stream: TcpStream, state: SharedState, running: Arc<AtomicBool>) {
    let peer = stream.peer_addr().ok();
    if let Err(e) = stream.set_nonblocking(false) {
        warn!("Failed to set blocking mode: {}", e);
        return;
    }
    // Set a read timeout so we can check `running` periodically
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));

    // A failed `try_clone` used to panic this connection's thread; a peer that
    // exhausts the fd limit should just lose its own connection.
    let mut reader = match stream.try_clone() {
        Ok(read_half) => BufReader::new(read_half),
        Err(e) => {
            warn!("Failed to split API connection from {:?}: {}", peer, e);
            return;
        }
    };
    let mut writer = stream;
    // Accumulates one request line across however many reads it takes. Never
    // cleared on a timeout: the size cap and the deadline below are both
    // per-request, and clearing here would reset them every read window.
    let mut line = String::new();
    // Set once the first byte of a request arrives, cleared when it completes.
    // `None` means the connection is idle between requests, which is allowed to
    // last indefinitely.
    let mut request_started: Option<std::time::Instant> = None;

    while running.load(Ordering::Relaxed) {
        // Budget each read so `line` can exceed the cap by at most one byte,
        // which is enough to notice that no newline arrived in time.
        let budget = (MAX_REQUEST_BYTES + 1 - line.len()) as u64;
        let read = reader.by_ref().take(budget).read_line(&mut line);

        match read {
            // Peer closed. Any partial request dies with it.
            Ok(0) => break,
            Ok(_) => {
                if !line.is_empty() && request_started.is_none() {
                    request_started = Some(std::time::Instant::now());
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                // Keep what already arrived and check the deadline: a client
                // that dribbles bytes forever must not hold this thread — and
                // one of the few connection slots — open indefinitely.
                if exceeded_deadline(request_started) {
                    reject(&mut writer, peer, "Request timed out before completing");
                    break;
                }
                continue;
            }
            Err(_) => break,
        }

        if line.len() > MAX_REQUEST_BYTES {
            reject(
                &mut writer,
                peer,
                &format!("Request exceeds {} byte limit", MAX_REQUEST_BYTES),
            );
            break;
        }

        if !line.ends_with('\n') {
            // Incomplete request: keep reading until the newline, the cap, or
            // the deadline.
            if exceeded_deadline(request_started) {
                reject(&mut writer, peer, "Request timed out before completing");
                break;
            }
            continue;
        }

        // A complete request line. Take it so `line` is empty for the next one.
        let request = std::mem::take(&mut line);
        request_started = None;

        if request.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RpcRequest>(&request) {
            Ok(req) => handle_request(&req, &state),
            Err(e) => RpcResponse::error(None, PARSE_ERROR, format!("Parse error: {}", e)),
        };

        let mut resp_json = serde_json::to_string(&response).unwrap_or_default();
        resp_json.push('\n');
        if writer.write_all(resp_json.as_bytes()).is_err() {
            break;
        }
    }

    debug!("API client disconnected: {:?}", peer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::new_shared_state;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    /// Connect, exchange one request/response, and return the reader so callers
    /// can keep talking on the same connection.
    fn connect(port: u16) -> TcpStream {
        let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        stream
    }

    fn round_trip(stream: &mut TcpStream, request: &str) -> RpcResponse {
        writeln!(stream, "{}", request).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }

    /// `ApiServer::start(0, ..)` asks the OS for a port; the server has to
    /// report the port it actually bound, not the 0 it was handed.
    #[test]
    fn ephemeral_port_is_reported() {
        let state = new_shared_state();
        let mut server = ApiServer::start(0, state).unwrap();
        assert_ne!(server.port, 0, "bound port should be resolved, not 0");

        let mut stream = connect(server.port);
        let resp = round_trip(
            &mut stream,
            r#"{"jsonrpc":"2.0","id":1,"method":"engine.status","params":null}"#,
        );
        assert!(resp.error.is_none());

        server.stop();
    }

    #[test]
    fn server_handles_rpc_over_tcp() {
        let state = new_shared_state();
        crate::lock_or_recover(&state).snapshot.tick = 99;

        let mut server = ApiServer::start(0, state).unwrap();
        let mut stream = connect(server.port);

        let resp = round_trip(
            &mut stream,
            r#"{"jsonrpc":"2.0","id":1,"method":"engine.status","params":null}"#,
        );
        assert!(resp.error.is_none(), "unexpected error: {:?}", resp.error);
        assert_eq!(
            resp.result.expect("status result")["tick"],
            99,
            "handler should see the state the test seeded"
        );

        server.stop();
        assert!(!server.is_running());
    }

    #[test]
    fn server_start_stop() {
        let state = new_shared_state();
        let mut server = ApiServer::start(0, state).unwrap();
        assert!(server.is_running());
        server.stop();
        assert!(!server.is_running());
    }

    /// Garbage and unknown methods must produce error responses, and the
    /// connection must stay usable afterwards.
    #[test]
    fn malformed_requests_do_not_kill_the_connection() {
        let state = new_shared_state();
        let mut server = ApiServer::start(0, state).unwrap();
        let mut stream = connect(server.port);

        let resp = round_trip(&mut stream, "this is not json");
        assert_eq!(resp.error.expect("parse error").code, PARSE_ERROR);

        let resp = round_trip(
            &mut stream,
            r#"{"jsonrpc":"2.0","id":2,"method":"no.such.method","params":null}"#,
        );
        assert!(resp.error.is_some(), "unknown method should error");

        // Still alive after both.
        let resp = round_trip(
            &mut stream,
            r#"{"jsonrpc":"2.0","id":3,"method":"engine.status","params":null}"#,
        );
        assert!(
            resp.error.is_none(),
            "connection should still serve requests"
        );

        server.stop();
    }

    /// A client that never sends a newline must not be able to grow the
    /// server's buffer without bound.
    #[test]
    fn oversized_request_is_rejected() {
        let state = new_shared_state();
        let mut server = ApiServer::start(0, state).unwrap();
        let mut stream = connect(server.port);

        // Write more than the cap, with no newline anywhere.
        let chunk = "x".repeat(64 * 1024);
        let mut written = 0usize;
        while written <= MAX_REQUEST_BYTES {
            if stream.write_all(chunk.as_bytes()).is_err() {
                break;
            }
            written += chunk.len();
        }
        let _ = stream.flush();

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        let resp: RpcResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(
            resp.error.expect("should report an oversized request").code,
            INVALID_REQUEST
        );

        server.stop();
    }

    /// The size cap has to apply per request, not per read window.
    ///
    /// `handle_client` clears its buffer only once a request completes. An
    /// earlier version cleared it at the top of every iteration, so bytes
    /// already consumed were dropped whenever the 1s read timeout fired — a
    /// client that paused between chunks could stream unbounded data while each
    /// individual read stayed under the cap, holding a connection slot open.
    #[test]
    fn oversized_request_split_across_read_timeouts_is_rejected() {
        let state = new_shared_state();
        let mut server = ApiServer::start(0, state).unwrap();
        let mut stream = connect(server.port);

        // Each pause is longer than the server's 1s read timeout, so every
        // chunk lands in a separate read.
        let chunk = "x".repeat(400 * 1024);
        for _ in 0..3 {
            if stream.write_all(chunk.as_bytes()).is_err() {
                break;
            }
            let _ = stream.flush();
            thread::sleep(std::time::Duration::from_millis(1200));
        }

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        let resp: RpcResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(
            resp.error
                .expect("cap must trip on the accumulated request")
                .code,
            INVALID_REQUEST
        );

        server.stop();
    }

    /// The flip side of keeping the buffer: a request that arrives in pieces
    /// must still be assembled. Discarding partial bytes on timeout turned a
    /// slow but legitimate request into a parse error.
    #[test]
    fn request_split_across_a_read_timeout_is_assembled() {
        let state = new_shared_state();
        crate::lock_or_recover(&state).snapshot.tick = 7;

        let mut server = ApiServer::start(0, state).unwrap();
        let mut stream = connect(server.port);

        stream
            .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"engine."#)
            .unwrap();
        stream.flush().unwrap();
        // Force the server's read to time out mid-request.
        thread::sleep(std::time::Duration::from_millis(1300));
        stream.write_all(b"status\",\"params\":null}\n").unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        let resp: RpcResponse = serde_json::from_str(&response).unwrap();

        assert!(
            resp.error.is_none(),
            "split request should parse, got {:?}",
            resp.error
        );
        assert_eq!(resp.result.expect("status result")["tick"], 7);

        server.stop();
    }
}
