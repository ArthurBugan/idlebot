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