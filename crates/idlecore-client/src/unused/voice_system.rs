//! Sistema de voice chat — placeholder para future integração
//!
//! Currently a stub — no actual WebRTC or voice functionality yet.

/// Event for voice channel changes
#[derive(Debug, Clone)]
pub enum VoiceChannelEvent {
    /// Player joined the channel with the specified hex ID.
    Joined { hex_id: u64, player: String },
    /// Player left the channel.
    Left { hex_id: u64, player: String },
}

/// ECS Component representing the local player's real-time view of the voice channel (FR5).
/// This drives the UI visualization.
#[derive(bevy::ecs::component::Component, Debug, Clone)]
pub struct VoiceIndicator {
    pub hex_id: u64,
    /// Visual cue: True if *any* player in the channel is actively transmitting audio.
    pub is_channel_busy: bool,
    /// List of names of all players detected in the channel (including self).
    pub present_players: Vec<String>,
    /// A frame tick count used by the UI to animate visuals like pulsing waves.
    pub presence_tick: u32,
}

impl VoiceIndicator {
    /// Creates a new indicator when a player joins a channel.
    pub fn new(local_player_id: String, hex_id: u64) -> Self {
        Self {
            hex_id,
            is_channel_busy: false,
            present_players: vec![local_player_id],
            presence_tick: 0,
        }
    }

    /// Updates the indicator when server-side state changes (e.g., via server events).
    pub fn update_state(&mut self, added: &[&str], removed: &[&str]) {
        // Handle removals
        for &player_to_remove in removed {
            if let Some(index) = self.present_players.iter().position(|p| p == player_to_remove) {
                self.present_players.remove(index);
            }
        }
        // Handle additions
        for &player_added in added {
            if !self.present_players.contains(&player_added.to_string()) {
                self.present_players.push(player_added.to_string());
            }
        }

        // Determine busy status (FR5 detail: 2+ players implies activity)
        self.is_channel_busy = self.present_players.len() >= 2;

        // Cycle tick for animation consistency
        self.presence_tick = self.presence_tick.wrapping_add(1);
    }
}

/// Legacy/Server-Side State Projection (Base State synced from Server)
/// This represents the canonical server state before it's mapped to the local ECS component.
#[derive(Clone)]
pub struct VoiceChannelState {
    pub hex_id: u64,
    pub players: Vec<String>,
    pub is_active: bool,
    pub my_turn_to_speak: bool,
}
