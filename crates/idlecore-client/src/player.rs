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

