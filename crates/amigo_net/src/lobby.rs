//! Lobby system for pre-game player management.
//!
//! Manages rooms, player slots, ready state, and match lifecycle
//! before the game simulation begins.

use crate::PlayerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Lobby types
// ---------------------------------------------------------------------------

/// Unique room identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub u32);

/// A player slot in a lobby room.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub id: PlayerId,
    pub name: String,
    pub ready: bool,
    pub team: u8,
}

/// Current phase of a lobby room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomPhase {
    /// Waiting for players to join.
    Waiting,
    /// All players ready, countdown to start.
    Countdown,
    /// Game is in progress.
    InGame,
    /// Game finished, showing results.
    Results,
    /// Room closed.
    Closed,
}

/// Configuration for a lobby room.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomConfig {
    pub name: String,
    pub max_players: u8,
    pub map: String,
    pub game_mode: String,
    pub password: Option<String>,
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self {
            name: "Game Room".into(),
            max_players: 4,
            map: String::new(),
            game_mode: "coop".into(),
            password: None,
        }
    }
}

/// A lobby room that holds players before a match.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    pub id: RoomId,
    pub config: RoomConfig,
    pub host: PlayerId,
    pub players: Vec<LobbyPlayer>,
    pub phase: RoomPhase,
}

impl Room {
    pub fn new(id: RoomId, host: PlayerId, host_name: String, config: RoomConfig) -> Self {
        let host_player = LobbyPlayer {
            id: host,
            name: host_name,
            ready: false,
            team: 0,
        };
        Self {
            id,
            config,
            host,
            players: vec![host_player],
            phase: RoomPhase::Waiting,
        }
    }

    /// Try to add a player. Returns false if room is full or in-game.
    pub fn join(&mut self, id: PlayerId, name: String, password: Option<&str>) -> bool {
        if self.phase != RoomPhase::Waiting {
            return false;
        }
        if self.players.len() >= self.config.max_players as usize {
            return false;
        }
        if let Some(ref pw) = self.config.password {
            if password != Some(pw.as_str()) {
                return false;
            }
        }
        if self.players.iter().any(|p| p.id == id) {
            return false; // already in room
        }
        self.players.push(LobbyPlayer {
            id,
            name,
            ready: false,
            team: 0,
        });
        true
    }

    /// Remove a player from the room.
    /// If the host leaves, the next player becomes host.
    /// Returns true if the room is now empty and should be removed.
    pub fn leave(&mut self, id: PlayerId) -> bool {
        self.players.retain(|p| p.id != id);
        if self.players.is_empty() {
            self.phase = RoomPhase::Closed;
            return true;
        }
        // Migrate host
        if self.host == id {
            self.host = self.players[0].id;
        }
        // Cancel countdown if someone leaves
        if self.phase == RoomPhase::Countdown {
            self.phase = RoomPhase::Waiting;
        }
        false
    }

    /// Toggle a player's ready state.
    pub fn set_ready(&mut self, id: PlayerId, ready: bool) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == id) {
            p.ready = ready;
        }
    }

    /// Set a player's team.
    pub fn set_team(&mut self, id: PlayerId, team: u8) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == id) {
            p.team = team;
        }
    }

    /// Check if all players are ready.
    pub fn all_ready(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|p| p.ready)
    }

    /// Transition to countdown (host action, all must be ready).
    pub fn start_countdown(&mut self) -> bool {
        if self.phase != RoomPhase::Waiting || !self.all_ready() {
            return false;
        }
        self.phase = RoomPhase::Countdown;
        true
    }

    /// Transition to in-game.
    pub fn start_game(&mut self) -> bool {
        if self.phase != RoomPhase::Countdown {
            return false;
        }
        self.phase = RoomPhase::InGame;
        true
    }

    /// Transition to results.
    pub fn end_game(&mut self) {
        self.phase = RoomPhase::Results;
    }

    /// Number of players in the room.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Whether the room has space.
    pub fn is_full(&self) -> bool {
        self.players.len() >= self.config.max_players as usize
    }

    /// Whether a specific player is the host.
    pub fn is_host(&self, id: PlayerId) -> bool {
        self.host == id
    }

    /// Get a snapshot of the room for listing.
    pub fn info(&self) -> RoomInfo {
        RoomInfo {
            id: self.id,
            name: self.config.name.clone(),
            map: self.config.map.clone(),
            game_mode: self.config.game_mode.clone(),
            players: self.players.len() as u8,
            max_players: self.config.max_players,
            phase: self.phase,
            has_password: self.config.password.is_some(),
        }
    }
}

/// Lightweight room info for lobby listing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: RoomId,
    pub name: String,
    pub map: String,
    pub game_mode: String,
    pub players: u8,
    pub max_players: u8,
    pub phase: RoomPhase,
    pub has_password: bool,
}

// ---------------------------------------------------------------------------
// LobbyManager — manages multiple rooms
// ---------------------------------------------------------------------------

/// Manages all lobby rooms on the server.
pub struct LobbyManager {
    rooms: HashMap<RoomId, Room>,
    next_room_id: u32,
    /// Which room each player is in.
    player_rooms: HashMap<PlayerId, RoomId>,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            next_room_id: 1,
            player_rooms: HashMap::new(),
        }
    }

    /// Create a new room. Returns the room ID.
    pub fn create_room(&mut self, host: PlayerId, host_name: String, config: RoomConfig) -> RoomId {
        let id = RoomId(self.next_room_id);
        self.next_room_id += 1;
        let room = Room::new(id, host, host_name, config);
        self.rooms.insert(id, room);
        self.player_rooms.insert(host, id);
        id
    }

    /// Join an existing room.
    pub fn join_room(
        &mut self,
        room_id: RoomId,
        player: PlayerId,
        name: String,
        password: Option<&str>,
    ) -> bool {
        // Leave current room first
        self.leave_current_room(player);

        if let Some(room) = self.rooms.get_mut(&room_id) {
            if room.join(player, name, password) {
                self.player_rooms.insert(player, room_id);
                return true;
            }
        }
        false
    }

    /// Leave the player's current room.
    pub fn leave_current_room(&mut self, player: PlayerId) {
        if let Some(room_id) = self.player_rooms.remove(&player) {
            let empty = if let Some(room) = self.rooms.get_mut(&room_id) {
                room.leave(player)
            } else {
                false
            };
            if empty {
                self.rooms.remove(&room_id);
            }
        }
    }

    /// Get a reference to a room.
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.get(&id)
    }

    /// Get a mutable reference to a room.
    pub fn room_mut(&mut self, id: RoomId) -> Option<&mut Room> {
        self.rooms.get_mut(&id)
    }

    /// Get the room a player is in.
    pub fn player_room(&self, player: PlayerId) -> Option<RoomId> {
        self.player_rooms.get(&player).copied()
    }

    /// List all joinable rooms.
    pub fn list_rooms(&self) -> Vec<RoomInfo> {
        self.rooms
            .values()
            .filter(|r| r.phase == RoomPhase::Waiting)
            .map(|r| r.info())
            .collect()
    }

    /// Total number of rooms (including in-game).
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Remove closed rooms.
    pub fn cleanup(&mut self) {
        self.rooms.retain(|_, r| r.phase != RoomPhase::Closed);
    }
}

impl Default for LobbyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: u32) -> PlayerId {
        PlayerId(id)
    }

    #[test]
    fn create_room() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        assert_eq!(lobby.room_count(), 1);

        let room = lobby.room(rid).unwrap();
        assert_eq!(room.player_count(), 1);
        assert!(room.is_host(p(1)));
        assert_eq!(room.phase, RoomPhase::Waiting);
    }

    #[test]
    fn join_and_leave() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());

        assert!(lobby.join_room(rid, p(2), "Bob".into(), None));
        assert_eq!(lobby.room(rid).unwrap().player_count(), 2);

        lobby.leave_current_room(p(2));
        assert_eq!(lobby.room(rid).unwrap().player_count(), 1);
    }

    #[test]
    fn host_migration() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        lobby.join_room(rid, p(2), "Bob".into(), None);

        // Host leaves
        lobby.leave_current_room(p(1));
        let room = lobby.room(rid).unwrap();
        assert!(room.is_host(p(2)));
    }

    #[test]
    fn empty_room_removed() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        lobby.leave_current_room(p(1));
        // Room was marked closed and removed
        assert!(lobby.room(rid).is_none());
        assert_eq!(lobby.room_count(), 0);
    }

    #[test]
    fn room_full() {
        let mut lobby = LobbyManager::new();
        let config = RoomConfig {
            max_players: 2,
            ..Default::default()
        };
        let rid = lobby.create_room(p(1), "Alice".into(), config);
        assert!(lobby.join_room(rid, p(2), "Bob".into(), None));
        assert!(!lobby.join_room(rid, p(3), "Charlie".into(), None));
    }

    #[test]
    fn password_protection() {
        let mut lobby = LobbyManager::new();
        let config = RoomConfig {
            password: Some("secret".into()),
            ..Default::default()
        };
        let rid = lobby.create_room(p(1), "Alice".into(), config);

        // Wrong password
        assert!(!lobby.join_room(rid, p(2), "Bob".into(), Some("wrong")));
        // No password
        assert!(!lobby.join_room(rid, p(3), "Charlie".into(), None));
        // Correct password
        assert!(lobby.join_room(rid, p(4), "Dave".into(), Some("secret")));
    }

    #[test]
    fn ready_and_countdown() {
        let mut lobby = LobbyManager::new();
        let config = RoomConfig {
            max_players: 2,
            ..Default::default()
        };
        let rid = lobby.create_room(p(1), "Alice".into(), config);
        lobby.join_room(rid, p(2), "Bob".into(), None);

        let room = lobby.room_mut(rid).unwrap();
        assert!(!room.all_ready());

        // Can't start countdown unless all ready
        assert!(!room.start_countdown());

        room.set_ready(p(1), true);
        assert!(!room.all_ready());

        room.set_ready(p(2), true);
        assert!(room.all_ready());

        assert!(room.start_countdown());
        assert_eq!(room.phase, RoomPhase::Countdown);
    }

    #[test]
    fn game_lifecycle() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        let room = lobby.room_mut(rid).unwrap();
        room.set_ready(p(1), true);
        room.start_countdown();
        assert!(room.start_game());
        assert_eq!(room.phase, RoomPhase::InGame);

        room.end_game();
        assert_eq!(room.phase, RoomPhase::Results);
    }

    #[test]
    fn leave_during_countdown_resets() {
        let mut lobby = LobbyManager::new();
        let config = RoomConfig {
            max_players: 2,
            ..Default::default()
        };
        let rid = lobby.create_room(p(1), "Alice".into(), config);
        lobby.join_room(rid, p(2), "Bob".into(), None);

        let room = lobby.room_mut(rid).unwrap();
        room.set_ready(p(1), true);
        room.set_ready(p(2), true);
        room.start_countdown();
        assert_eq!(room.phase, RoomPhase::Countdown);

        // Someone leaves → back to waiting
        room.leave(p(2));
        assert_eq!(room.phase, RoomPhase::Waiting);
    }

    #[test]
    fn list_rooms_only_waiting() {
        let mut lobby = LobbyManager::new();
        lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        let rid2 = lobby.create_room(p(2), "Bob".into(), RoomConfig::default());

        // Start one game
        let room = lobby.room_mut(rid2).unwrap();
        room.set_ready(p(2), true);
        room.start_countdown();
        room.start_game();

        let listed = lobby.list_rooms();
        assert_eq!(listed.len(), 1); // Only the waiting room
    }

    #[test]
    fn team_assignment() {
        let mut lobby = LobbyManager::new();
        let rid = lobby.create_room(p(1), "Alice".into(), RoomConfig::default());
        lobby.join_room(rid, p(2), "Bob".into(), None);

        let room = lobby.room_mut(rid).unwrap();
        room.set_team(p(1), 1);
        room.set_team(p(2), 2);

        assert_eq!(room.players[0].team, 1);
        assert_eq!(room.players[1].team, 2);
    }
}
