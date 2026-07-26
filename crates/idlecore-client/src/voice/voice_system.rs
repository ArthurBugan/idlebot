//! Sistema de voice chat — placeholder para future integração
//!
//! Currently a stub — no actual WebRTC or voice functionality yet.

/// Event for voice channel changes
#[derive(Debug, Clone)]
pub enum VoiceChannelEvent {
    Joined { hex_id: u64, player: String },
    Left { hex_id: u64, player: String },
}

/// Voice channel component
#[derive(bevy::ecs::component::Component, Debug, Clone)]
pub struct VoiceChannel {
    pub hex_id: u64,
    pub players: Vec<String>,
}

impl VoiceChannel {
    /// Create a new voice channel with a single player
    pub fn new(hex_id: u64, player: String) -> Self {
        Self {
            hex_id,
            players: vec![player],
        }
    }

    /// Add a player to the channel
    pub fn add_peer(&mut self, peer: String) {
        if !self.players.contains(&peer) {
            self.players.push(peer);
        }
    }

    /// Remove a player from the channel
    pub fn remove_peer(&mut self, peer: &str) {
        self.players.retain(|p| p != peer);
    }

    /// Check if a player is in the channel
    pub fn has_player(&self, peer: &str) -> bool {
        self.players.contains(&peer.to_string())
    }
}
