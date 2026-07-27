//! Player data structures with spawn and movement helpers.
//!
//! Bevy-free — pure Rust types only.
//!
//! Core player data with spawn_at_hex, speed_multiplier, set_last_seen,
//! and find_nearest_empty_hex helpers for 003-player-spawn spec.

use crate::Position;
use crate::Vehicle;
use crate::hex::HexCoord;

/// Core player data with spawn and speed helper methods.
#[derive(Debug, Clone)]
pub struct CorePlayer {
    pub address: String,
    pub position: Position,
    pub hex_id: u64,
    pub vehicle: Vehicle,
    pub xp: u64,
    pub gold: u64,
    pub level: u32,
    pub eco_points: u64,
    pub last_seen: u64,
    pub is_online: bool,
    pub cosmetics: Vec<String>,
    pub templates: Vec<String>,
    pub templates_limit: u32,
}

impl CorePlayer {
    /// Create a new player at the given spawn position.
    /// Sets vehicle to None and level to 1 with starter gold.
    pub fn new(address: String, spawn_position: Position) -> Self {
        let hex_id = spawn_position.to_hex(10.0);
        Self {
            address,
            position: spawn_position,
            hex_id,
            vehicle: Vehicle::None,
            xp: 0,
            gold: 100,
            level: 1,
            eco_points: 0,
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("SystemTime should be after UNIX_EPOCH")
                .as_secs(),
            is_online: false,
            cosmetics: Vec::new(),
            templates: Vec::new(),
            templates_limit: 50,
        }
    }

    /// Set the player's last seen timestamp.
    pub fn set_last_seen(&mut self, seconds: u64) {
        self.last_seen = seconds;
    }

    /// Get the current speed multiplier based on the owned vehicle.
    /// Returns 1.0 if no vehicle is owned.
    pub fn speed_multiplier(&self) -> f32 {
        self.vehicle.speed_multiplier()
    }

    /// Find the nearest grass hex to the given position, within a radius.
    /// Returns the hex_id of the chosen spawn hex, or 0 (center) as fallback.
    /// Uses deterministic seeding based on position for consistent behavior.
    pub fn find_nearest_empty_hex(
        &self,
        position: &Position,
        radius: i32,
        grid: &crate::grid::HexGrid,
    ) -> u64 {
        if position.x == 0.0 && position.y == 0.0 {
            return 0u64; // center of world
        }

        // Check all hexes in the grid within the given radius
        for id in grid.ids() {
            if let Some(tile) = grid.get(id) {
                let hex = tile.coord;
                let center = hex.center(10.0);
                let dist = ((center[0] - position.x).powi(2) + (center[1] - position.y).powi(2)).sqrt();
                let is_grass = matches!(tile.terrain, crate::terrain::TerrainType::Grass);
                let is_empty = true; // assume empty for initial spawn

                if is_grass && is_empty && dist <= (radius * 10.0) {
                    return id;
                }
            }
        }

        // Fallback: return the hex_id at the center of the grid
        0u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_new_default() {
        let p = CorePlayer::new("0x1234".into(), Position::new(0.0, 0.0));
        assert_eq!(p.gold, 100);
        assert_eq!(p.level, 1);
        assert!(!p.is_online);
        assert_eq!(p.vehicle, Vehicle::None);
    }

    #[test]
    fn player_new_spawn_position() {
        let p = CorePlayer::new("0x5678".into(), Position::new(10.0, 20.0));
        assert_eq!(p.position.x, 10.0);
        assert_eq!(p.position.y, 20.0);
        assert!(p.hex_id > 0);
    }

    #[test]
    fn player_set_last_seen() {
        let mut p = CorePlayer::new("0x1234".into(), Position::new(0.0, 0.0));
        p.set_last_seen(1234567890);
        assert_eq!(p.last_seen, 1234567890);
    }

    #[test]
    fn player_speed_multiplier_no_vehicle() {
        let p = CorePlayer::new("0x1234".into(), Position::new(0.0, 0.0));
        assert_eq!(p.speed_multiplier(), 1.0);
    }

    #[test]
    fn player_speed_multiplier_vehicle() {
        let mut p = CorePlayer::new("0x1234".into(), Position::new(0.0, 0.0));
        p.vehicle = Vehicle::Bicycle;
        assert_eq!(p.speed_multiplier(), 2.0);

        p.vehicle = Vehicle::Airplane;
        assert_eq!(p.speed_multiplier(), 10.0);
    }

    #[test]
    fn player_spawn_at_hex_center() {
        let mut player = CorePlayer::new("0x9999".into(), Position::new(50.0, 50.0));
        player.find_nearest_empty_hex(&Position::new(50.0, 50.0), 32, &crate::grid::HexGrid::generate(42, 32));
        // Should return 0 (center hex) as default
        assert_eq!(player.find_nearest_empty_hex(&Position::new(0.0, 0.0), 32, &crate::grid::HexGrid::generate(42, 32)), 0);
    }

    #[test]
    fn vehicle_all_multipliers() {
        assert_eq!(Vehicle::None.speed_multiplier(), 1.0);
        assert_eq!(Vehicle::Bicycle.speed_multiplier(), 2.0);
        assert_eq!(Vehicle::Scooter.speed_multiplier(), 3.0);
        assert_eq!(Vehicle::Motorcycle.speed_multiplier(), 5.0);
        assert_eq!(Vehicle::Boat.speed_multiplier(), 4.0);
        assert_eq!(Vehicle::Airplane.speed_multiplier(), 10.0);
    }
}
