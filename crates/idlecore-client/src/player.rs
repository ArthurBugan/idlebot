//! Player component and spawning
//!
//! Reuses types from idlecore-core where possible.

use bevy::prelude::*;

/// Player marker component
#[derive(Component)]
pub struct Player;

/// Hex coordinate pair (axial coordinates q, r)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentHex {
    pub x: i32,
    pub y: i32,
}

/// Main player component — tracks all player state.
///
/// 2D world: `position.x` = east, `position.y` = north, `position.z` = 0.
#[derive(Component)]
pub struct ClientPlayer {
    pub position: Vec3,
    pub velocity: Vec2,
    pub current_hex: Option<CurrentHex>,
    pub gold: u64,
    pub usdt: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    pub owned_vehicle: Option<idlecore_core::Vehicle>,
    /// Server-side avatar name; a Toon Characters name (assets/models/Toon
    /// Characters/*) or a legacy assets/skins/*.png file name.
    pub avatar: String,
    /// True once the spawn position has been restored from the server row.
    pub position_restored: bool,
    /// Last mirrored `player.last_action_at` — a newer value means the
    /// server accepted a new action and its 5 s cooldown just started.
    pub last_action_seen: u64,
    /// Local mirror of the cooldown end (server more `last_action_at + 5 s`,
    /// compared against local unix time by the hotbar timer).
    pub action_cd_until_utc: u64,
    // Spec 027 §3 — combat mirror. hp 0 on a fresh session means "unseeded"
    // on the server too (renders full), NOT dead: death is death_until_utc.
    pub hp: u32,
    pub weapon: u8,
    /// Bitmask of purchased weapon tiers (bit i = tier i).
    pub weapons_owned: u16,
    pub attack_cd_until_utc: u64,
    pub dash_cd_until_utc: u64,
    pub dash_until_utc: u64,
    pub hostile_until_utc: u64,
    pub death_until_utc: u64,
    pub grace_until_utc: u64,
    // Spec 037 — fine combat clock mirror (unix ms, server-stamped).
    /// Movement locked until this ms (the swing owns the body).
    pub move_lock_until_ms: u64,
    /// i-frames until this ms (0.6 s window — the client burst matches).
    pub dodge_until_ms: u64,
    /// Fine attack cadence until this ms (weapon cd exactly).
    pub attack_cd_until_ms: u64,
    /// Spec 036 — dodge-step burst deadline (unix secs): fires on the Space
    /// press (0.6 s ×3 local), trimmed by the authoritative row a beat
    /// later. The i-frame is the server's `dash_until` (1 s window).
    pub dash_burst_secs: f64,
}

impl ClientPlayer {
    /// Currently equipped weapon def (Spec 027 §4 + Spec 033 matrix).
    pub fn weapon_def(&self) -> &'static idlecore_core::player_combat::Weapon {
        idlecore_core::player_combat::weapon(self.weapon).unwrap_or_else(|| {
            // Fists are the head of the matrix (id 0).
            &idlecore_core::player_combat::weapons()[0]
        })
    }

    /// Is the dodge burst/i-frame live right now (local press + server
    /// stamp)? Restored Spec 036: the dodge is a real verb again.
    pub fn dashing(&self, unix_now: f64) -> bool {
        self.dash_until_utc as f64 > unix_now || self.dash_burst_secs > unix_now
    }
}

/// Player transform resource for camera/minimap follow.
/// `translation.x` = east, `translation.y` = north, `translation.z` = 0.
#[derive(Resource, Default)]
pub struct PlayerTransform {
    pub translation: Vec3,
}

/// Player's facing direction (rotation angle in radians).
/// 0.0 = +X (east), increases counterclockwise (north = +PI/2).
#[derive(Resource, Default)]
pub struct PlayerOrientation {
    pub facing_angle: f32,
}

/// Side of the player sprite in world units (art is square, 1024x1024).
pub const PLAYER_SIZE: f32 = 5.5;