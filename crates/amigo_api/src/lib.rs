/// AI/IPC interface (Phase 5, currently a stub).
/// Provides JSON-RPC interface for AI agents.
pub struct ApiServer {
    pub enabled: bool,
    pub port: u16,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        Self {
            enabled: false,
            port,
        }
    }
}

impl Default for ApiServer {
    fn default() -> Self {
        Self::new(9999)
    }
}
