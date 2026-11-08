//! JSON protocol between the Godot client and the `idlebot-godot-bridge`.
//!
//! The bridge is a WebSocket **server**: Godot connects, sends commands, and
//! receives server events. All messages are JSON objects carrying a `type`
//! discriminator (snake_case) via `#[serde(tag = "type")]`.
//!
//! The bridge forwards commands to the SpacetimeDB `nestfeed` module over the
//! SpacetimeDB SDK, and relays the resulting table row changes + reducer
//! results back to the (Godot) client(s).

use serde::{Deserialize, Serialize};

/// A message sent by the Godot client to the bridge.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Sent on open; the bridge replies with a `connected` (or `error`).
    Connect,
    /// Bind this identity to a player account (lowercased by the server).
    Login {
        name: String,
    },
    /// Update the player profile (all fields optional; `None` = leave unchanged).
    UpdateProfile {
        display_name: Option<String>,
        avatar: Option<String>,
        bio: Option<String>,
    },
    /// Server-authoritative move (mirrors the Bevy client's `move_player`).
    Move {
        dir_x: f32,
        dir_y: f32,
        intended_speed: f32,
        dt: f32,
        to_x: f32,
        to_y: f32,
    },
    /// Keep the player online + refresh the hex lock.
    Heartbeat,
    /// Plant on a hex.
    Plant {
        hex_id: u64,
        plant_type: String,
    },
    /// Harvest a hex.
    Harvest {
        hex_id: u64,
    },
    /// Clean a hex.
    Clean {
        hex_id: u64,
    },
    /// Claim accrued idle gains.
    ClaimIdle,
    /// Teleport to a hex (server-validated).
    Teleport {
        q: i32,
        r: i32,
    },
}

/// A snapshot of the authoritative `player` row fields that a Godot client
/// needs. (The SpacetimeDB `Player` type uses a non-`serde` wire format, so we
/// project the fields we expose to the bridge protocol.)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PlayerSnapshot {
    pub address: String,
    pub identity: String,
    pub status: String,
    pub display_name: Option<String>,
    pub avatar: String,
    pub level: u32,
    pub total_xp: u64,
    pub gold: u64,
    pub position_x: f32,
    pub position_y: f32,
    pub hex_q: i32,
    pub hex_r: i32,
    pub hex_id: u64,
    pub vehicle: String,
    pub last_seen: u64,
}

impl PlayerSnapshot {
    /// Project a SpacetimeDB `player` row onto the bridge snapshot.
    #[allow(dead_code)]
    pub fn from_player(p: &crate::gen::player_type::Player) -> Self {
        Self {
            address: p.address.clone(),
            identity: p.identity.clone(),
            status: p.status.clone(),
            display_name: p.display_name.clone(),
            avatar: p.avatar.clone(),
            level: p.level,
            total_xp: p.total_xp,
            gold: p.gold,
            position_x: p.position_x,
            position_y: p.position_y,
            hex_q: p.hex_q,
            hex_r: p.hex_r,
            hex_id: p.hex_id,
            vehicle: p.vehicle.clone(),
            last_seen: p.last_seen,
        }
    }
}

/// A message sent by the bridge to the Godot client.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// The SpacetimeDB connection established; `identity` is our issued id.
    Connected {
        identity: String,
    },
    /// A fatal / recoverable bridge or SpacetimeDB error.
    Error {
        message: String,
    },
    /// An authoritative `player` table change (other players + us).
    Row {
        table: String,
        op: String,
        data: Option<PlayerSnapshot>,
    },
    /// The outcome of a reducer the client asked us to run.
    Reducer {
        name: String,
        ok: bool,
        message: String,
    },
    /// Server confirmed a teleport; the client must move the player.
    Teleport {
        q: i32,
        r: i32,
    },
}
