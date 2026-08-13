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
    pub q: i32,
    pub r: i32,
}

/// Main player component — tracks all player state
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
    pub equipped_cosmetics: Vec<String>,
    pub last_login_time: u64,
    pub time_offline: Option<u64>,
}

impl ClientPlayer {
    /// Create a new ClientPlayer at spawn with defaults
    pub fn new_spawn(
        vehicle: Option<idlecore_core::Vehicle>,
        position: Vec3,
        level: u32,
        xp: u64,
        gold: u64,
        eco_points: u64,
        equipped_cosmetics: Vec<String>,
    ) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            current_hex: None,
            gold,
            usdt: 0,
            xp,
            level,
            eco_points,
            owned_vehicle: vehicle,
            equipped_cosmetics,
            last_login_time: 0,
            time_offline: None,
        }
    }
}

/// Player transform resource for camera/minimap follow
#[derive(Resource, Default)]
pub struct PlayerTransform {
    pub translation: Vec3,
}

/// Player's facing direction (rotation angle in radians around Y axis).
/// 0.0 = +X axis, increases counterclockwise.
#[derive(Resource, Default)]
pub struct PlayerOrientation {
    pub facing_angle: f32,
}

/// Marker for spawn point (visible indicator at world center)
#[derive(Component)]
pub struct SpawnMarker;
