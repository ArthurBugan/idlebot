//! Player component and spawning
//!
//! Client-side player: position, velocity, current hex, gold, XP, level,
//! eco points, owned vehicle, equipped cosmetics, last login time.
//! Avatar: orange tetrahedron (placeholder for Tamagotchi).

use bevy::prelude::*;
use crate::terrain::TerrainType;
use crate::economy::PlayerEconomy;

/// Marker component at world center (0, 0, 0)
#[derive(Component)]
pub struct PlayerSpawnMarker;

/// Client-side player component (attached to the player entity)
#[derive(Component)]
pub struct Player {
    /// World position (x, y, z) in Bevy space
    pub position: Vec3,
    /// Current velocity (x, z) — updated by input system each frame
    pub velocity: Vec2,
    /// Current hex coordinates (q, r) — axial
    pub current_hex: Option<(i32, i32)>,
    /// Player's economy state
    pub economy: PlayerEconomy,
    /// Whether this is a local client player (not synced from server)
    pub is_local: bool,
}

impl Player {
    /// Create a default player at world center
    pub fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: None,
            economy: PlayerEconomy::default(),
            is_local: true,
        }
    }

    /// Create a new player with initial values
    pub fn new(gold: u64, xp: u64, eco_points: u64) -> Self {
        let mut econ = PlayerEconomy::default();
        econ.gold = gold;
        econ.xp = xp;
        econ.eco_points = eco_points;

        Self {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: None,
            economy: econ,
            is_local: true,
        }
    }
}
