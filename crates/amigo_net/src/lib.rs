pub mod protocol;
pub mod server;
pub mod client;
pub mod replay;

use serde::{Deserialize, Serialize};

/// Player identifier for multiplayer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

/// Transport trait for sending and receiving serialized commands.
/// Abstracts over local (singleplayer) and network (multiplayer) transports.
pub trait Transport<C: Clone> {
    fn send(&mut self, commands: &[C]);
    fn receive(&mut self) -> Vec<(PlayerId, Vec<C>)>;
}

/// Local transport for singleplayer (zero overhead).
pub struct LocalTransport<C> {
    player_id: PlayerId,
    pending: Vec<C>,
}

impl<C: Clone> LocalTransport<C> {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            pending: Vec::new(),
        }
    }
}

impl<C: Clone> Transport<C> for LocalTransport<C> {
    fn send(&mut self, commands: &[C]) {
        self.pending.extend_from_slice(commands);
    }

    fn receive(&mut self) -> Vec<(PlayerId, Vec<C>)> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        let commands = std::mem::take(&mut self.pending);
        vec![(self.player_id, commands)]
    }
}
