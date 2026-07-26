//! Mock voice chat system — console-based mock of proximity voice chat.

use crate::economy;
use std::time::SystemTime;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Mock Player for voice simulation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MockPlayer {
    pub address: String,
    pub hex_id: u64,
    pub name: String,
    pub is_online: bool,
}

// ---------------------------------------------------------------------------
// Voice Channel State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VoiceChannel {
    pub hex_id: u64,
    pub players: Vec<String>,
    pub created_at: u64,
    pub last_activity: u64,
    pub timeout_secs: u64,
}

impl VoiceChannel {
    pub fn new(hex_id: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            hex_id,
            players: Vec::new(),
            created_at: now,
            last_activity: now,
            timeout_secs: 300,
        }
    }

    pub fn add_player(&mut self, address: &str, now: u64) -> bool {
        if !self.players.contains(&address.to_string()) {
            self.players.push(address.to_string());
            self.last_activity = now;
            true
        } else {
            false
        }
    }

    pub fn remove_player(&mut self, address: &str, now: u64) -> bool {
        if self.players.iter().any(|p| p == address) {
            self.players.retain(|p| p != address);
            self.last_activity = now;
            self.players.len() > 0
        } else {
            false
        }
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn is_full(&self) -> bool {
        self.player_count() >= economy::MAX_HEX_PLAYERS as usize
    }

    pub fn is_active(&self, now: u64) -> bool {
        now - self.last_activity < self.timeout_secs
    }

    pub fn members(&self) -> Vec<&str> {
        self.players.iter().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Hex Occupancy Tracking
// ---------------------------------------------------------------------------

/// VoiceChatManager tracks voice channels per hex and player positions
#[derive(Debug, Default)]
pub struct VoiceChatManager {
    pub channels: HashMap<u64, VoiceChannel>,
    pub players: Vec<MockPlayer>,
    pub last_cleanup_time: u64,
    pub cleanup_interval: u64,
}

impl VoiceChatManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            players: Vec::new(),
            last_cleanup_time: 0,
            cleanup_interval: 60,
        }
    }

    /// Initialize with a mock player
    pub fn init_player(&mut self, address: &str, hex_id: u64, name: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.players.push(MockPlayer {
            address: address.to_string(),
            hex_id,
            name: name.to_string(),
            is_online: true,
        });

        self.ensure_channel(hex_id, now);
    }

    fn ensure_channel(&mut self, hex_id: u64, now: u64) {
        if let Some(channel) = self.channels.get_mut(&hex_id) {
            if channel.is_active(now) {
                channel.add_player(
                    &self.get_player_name_at_hex(hex_id),
                    now,
                );
                println!("[VOICE] Player '{}' joined voice channel in hex {} ({} players)",
                    self.get_player_name_at_hex(hex_id), hex_id, channel.player_count());
                return;
            }
        }

        let channel = VoiceChannel::new(hex_id);
        channel.add_player(&self.get_player_name_at_hex(hex_id), now);
        self.channels.insert(hex_id, channel);
        println!("[VOICE] Voice channel created for hex {} ({} players)", hex_id, 1);
    }

    /// Process a player moving to a new hex
    pub fn player_moved(&mut self, player: &MockPlayer, new_hex_id: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        player.hex_id = new_hex_id;

        // Leave old hex's channel
        self.handle_channel_change(player, false, now);

        // Join new hex's channel
        self.handle_channel_change(player, true, now);

        // Periodic cleanup
        self.cleanup_channels(now);
    }

    fn handle_channel_change(&mut self, player: &MockPlayer, entering: bool, now: u64) {
        let old_hex = player.hex_id;

        if entering {
            if let Some(channel) = self.channels.get_mut(&new_hex_id) {
                let added = channel.add_player(&player.name, now);
                if added {
                    voice_log!("  Entering voice channel in hex {} ({}/{} players)",
                        new_hex_id, channel.player_count(), economy::MAX_HEX_PLAYERS);
                } else {
                    voice_log!("  Channel for hex {} at 8/8 player limit!", new_hex_id);
                }
            } else {
                let channel = VoiceChannel::new(new_hex_id);
                let added = channel.add_player(&player.name, now);
                if added {
                    self.channels.insert(new_hex_id, channel);
                    println!("[VOICE] Voice channel created for hex {} ({} player: {})",
                        new_hex_id, 1, player.name);
                }
            }
        } else {
            if let Some(channel) = self.channels.get_mut(&old_hex) {
                if !channel.remove_player(&player.name, now) {
                    voice_log!("  Voice channel destroyed for hex {} (all players left)", old_hex);
                    self.channels.remove(&old_hex);
                }
            }
        }
    }

    fn cleanup_channels(&mut self, now: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - self.last_cleanup_time < self.cleanup_interval {
            return;
        }

        self.last_cleanup_time = now;

        let mut to_remove = Vec::new();
        for (&hex_id, channel) in &self.channels {
            if !channel.is_active(now) {
                to_remove.push(hex_id);
                voice_log!("  Inactive voice channel destroyed for hex {} (timeout: {}s)",
                    hex_id, channel.timeout_secs);
            }
        }

        for remove_id in to_remove {
            self.channels.remove(&remove_id);
        }
    }

    pub fn hex_player_count(&self, hex_id: u64) -> usize {
        self.channels.get(&hex_id)
            .map(|c| c.player_count())
            .unwrap_or(0)
    }

    pub fn get_player_name_at_hex(&self, hex_id: u64) -> String {
        self.players.iter()
            .find(|p| p.hex_id == hex_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

// ---------------------------------------------------------------------------
// Console Logging
// ---------------------------------------------------------------------------

macro_rules! voice_log {
    ($($arg:tt)*) => {
        println!("[VOICE] {}", format!($($arg)*));
    };
}

// ---------------------------------------------------------------------------
// Test Simulation
// ---------------------------------------------------------------------------

/// Simulate 8 mock players on different hexes
pub fn simulate_8_players() -> Vec<MockPlayer> {
    let mut players = Vec::new();

    for i in 0..8 {
        let hex_id = (i as u64) << 32 | (i as u64);
        players.push(MockPlayer {
            address: format!("player_{}", i),
            hex_id,
            name: format!("Player {}", i),
            is_online: true,
        });
    }

    println!("[VOICE] Simulating 8 players on the map");
    players
}

/// Create voice channels for all mock players
pub fn setup_voice_channels_for_players(players: &[MockPlayer]) {
    println!("[VOICE] Setting up voice channels for {} players", players.len());

    // Group players by hex
    let mut hex_players: HashMap<u64, Vec<&MockPlayer>> = HashMap::new();
    for player in players {
        hex_players.entry(player.hex_id).or_default().push(player);
    }

    let mut count = 0;
    for (hex_id, players_in_hex) in &hex_players {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if *players_in_hex.len() as u32 > economy::MAX_HEX_PLAYERS {
            println!("[VOICE] Hex {} has {} players (limit: {}), newest queued",
                hex_id, players_in_hex.len(), economy::MAX_HEX_PLAYERS);
        }

        for (i, player) in players_in_hex.iter().enumerate() {
            let joined = if i == 0 {
                println!("[VOICE] {} joined voice channel in hex {} ({} player)",
                    player.name, hex_id, 1);
                true
            } else {
                println!("[VOICE] {} joined voice channel in hex {} ({} players)",
                    player.name, hex_id, i);
                true
            };
            if joined {
                count += 1;
            }
        }
    }

    println!("[VOICE] Voice channels established: {} players in {} channels",
        count, hex_players.len());
}
