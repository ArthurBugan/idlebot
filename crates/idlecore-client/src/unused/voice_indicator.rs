//! Client-side voice chat structures for FR5 (Proximity) and FR6 (Audio Stub).
//!
//! This file contains the necessary ECS components and data structures for the client to render
//! the voice chat state, including the proximity indicators (FR5) and audio metadata (FR6).

use bevy::prelude::*;

// --- FR6: Audio Quality Stub ---
/// FR6: Audio Configuration settings structure.
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { sample_rate: 48000, channels: 1, bitrate: 48000 }
    }
}

/// Component indicating this entity is producing/receiving voice audio.
#[derive(Component, Debug, Clone)]
pub struct AudioStream;

/// Component holding the specific audio decoding/encoding settings for an entity.
#[derive(Component, Debug, Clone)]
pub struct AudioSettings {
    pub config: AudioConfig,
    pub bits_per_sample: u32,
}

// --- FR5: Proximity Indicator ---

/// Component representing the local player's real-time view of the voice channel.
/// This drives the UI visualization (e.g., pulsing, player name display).
#[derive(Component, Debug, Clone)]
pub struct VoiceIndicator {
    /// The ID of the hex where the channel is located.
    pub hex_id: u64,
    /// Visual cue: True if *any* player in the channel is actively transmitting audio.
    pub is_channel_busy: bool,
    /// List of names of all players detected in the channel (including self).
    pub present_players: Vec<String>,
    /// A frame tick count used by the UI to animate visuals like pulsing waves.
    pub presence_tick: u32,
}

impl VoiceIndicator {
    pub fn new(local_player_id: String, hex_id: u64) -> Self {
        Self {
            hex_id,
            is_channel_busy: false,
            present_players: vec![local_player_id],
            presence_tick: 0,
        }
    }

    /// Updates the indicator when server-side state changes.
    /// `added` and `removed` are lists of player IDs whose presence has changed.
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

// Legacy/Server-Side State Projection (Base State synced from Server)
#[derive(Clone)]
pub struct VoiceChannelState {
    pub hex_id: u64,
    pub players: Vec<String>,
    pub is_active: bool,
    pub my_turn_to_speak: bool,
}

impl VoiceChannelState {
    pub fn new(hex_id: u64, player_name: String) -> Self {
        Self {
            hex_id,
            players: vec![player_name],
            is_active: false,
            my_turn_to_speak: false,
        }
    }
    // Keeping add/remove methods for compatibility during transition
    pub fn add_player(&mut self, player: String) {
        if !self.players.contains(&player) {
            self.players.push(player);
            if self.players.len() >= 2 {
                self.is_active = true;
            }
        }
    }
    pub fn remove_player(&mut self, player: &str) {
        self.players.retain(|p| p != player);
        if self.players.is_empty() {
            self.is_active = false;
        }
    }
}