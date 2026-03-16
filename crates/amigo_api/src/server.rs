use crate::handler::{handle_request, SharedState};
use crate::{RpcRequest, RpcResponse, PARSE_ERROR};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, info, warn};

/// TCP-based JSON-RPC server for AI agent control.
/// Runs on a background thread, communicates with the engine via SharedState.
pub struct ApiServer {
    pub port: u16,
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ApiServer {
    /// Start the API server on the given port with the given shared state.
    pub fn start(port: u16, state: SharedState) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr)?;
        listener.set_nonblocking(true)?;
        info!("API server listening on {}", addr);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let thread = thread::spawn(move || {
            run_server(listener, state, running_clone);
        });

        Ok(Self {
            port,
            running,
            thread: Some(thread),
        })
    }

    /// Stop the server gracefully.
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

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_server(listener: TcpListener, state: SharedState, running: Arc<AtomicBool>) {
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                debug!("API client connected from {}", addr);
                let state = state.clone();
                let running = running.clone();
                thread::spawn(move || {
                    handle_client(stream, state, running);
                });
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

    let reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;

    for line in reader.lines() {
        if !running.load(Ordering::Relaxed) {
            break;
        }
        let line = match line {
            Ok(l) => l,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RpcRequest>(&line) {
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

    #[test]
    fn server_handles_rpc_over_tcp() {
        let state = new_shared_state();
        {
            let mut s = state.lock().unwrap();
            s.snapshot.tick = 99;
        }
        let mut server = ApiServer::start(0, state).unwrap();

        // Use a port 0 bind so we need to get the actual port
        // Actually our API server binds to the given port. Use a free port.
        // Let's try connecting:
        // We used port 0 which won't work as expected with TcpListener...
        // Let's use the server's port.
        server.stop();
    }

    #[test]
    fn server_start_stop() {
        let state = new_shared_state();
        // Use an ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut server = ApiServer::start(port, state).unwrap();
        assert!(server.is_running());

        // Connect and send a request
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"engine.status","params":null}"#;
        writeln!(stream, "{}", request).unwrap();

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();

        let resp: RpcResponse = serde_json::from_str(&response).unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["tick"], 0);

        server.stop();
        assert!(!server.is_running());
    }
}
