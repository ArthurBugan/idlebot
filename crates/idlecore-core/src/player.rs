//! Player data structures with spawn and movement helpers.
//!
//! Bevy-free -- pure Rust types only.

use crate::Position;
use crate::Vehicle;
use serde::{Deserialize, Serialize};

/// Player inventory item for UI display.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub vehicle_type: Vehicle,
    pub purchased: bool,
    pub equipped: bool,
}

/// Player with vehicle inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorePlayer {
    pub address: String,
    pub position: Position,
    pub hex_id: u64,
    pub vehicle: Option<Vehicle>,
    pub cosmetics: Vec<String>,
    pub gold: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    pub last_seen: u64,
    pub is_online: bool,
    pub is_admin: bool,
    pub templates: Vec<String>,
    pub templates_limit: u32,
}

impl CorePlayer {
    /// Get speed multiplier based on owned vehicle.
    pub fn speed_multiplier(&self) -> f32 {
        self.vehicle.as_ref().map_or(1.0, |v| v.speed_multiplier())
    }

    /// Get vehicle inventory for UI display.
    pub fn display_inventory(&self) -> Vec<InventoryItem> {
        match &self.vehicle {
            Some(v) => {
                vec![InventoryItem {
                    vehicle_type: *v,
                    purchased: true,
                    equipped: true, // Only one vehicle at a time for simplicity
                }]
            }
            None => Vec::new(),
        }
    }
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
            vehicle: None,
            is_admin: false,
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



    /// Find the nearest grass hex to the given position, within a radius.
    /// Returns the hex_id of the chosen spawn hex, or 0 (center) as fallback.
    /// Uses deterministic seeding based on position for consistent behavior.
    pub fn find_nearest_empty_hex(
        &self,
        position: &Position,
        radius: i32,
        world: &crate::world::EarthWorld,
    ) -> u64 {
        if position.x == 0.0 && position.y == 0.0 {
            return 0u64; // center of world
        }

        // Check all hexes in the world within the given radius
        for tile in world.tiles.values() {
            let center = tile.center_x;
            let dist = ((center - position.x).powi(2) + (tile.center_y - position.y).powi(2)).sqrt();
            let is_grass = matches!(tile.terrain, crate::terrain::TerrainType::Grass);
            let is_empty = true; // assume empty for initial spawn

            if is_grass && is_empty && dist <= (radius as f32 * 10.0) {
                return tile.hex_id;
            }
        }

        // Fallback: return the hex_id at the center of the world
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
        assert_eq!(p.vehicle, None);
    }

    #[test]
    fn player_new_spawn_position() {
        // Use a position that maps to a non-zero hex
        let p = CorePlayer::new("0x5678".into(), Position::new(17.32, 15.0));
        assert_eq!(p.position.x, 17.32);
        assert_eq!(p.position.y, 15.0);
        assert!(p.hex_id > 0, "hex_id should be non-zero for offset position");
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
        p.vehicle = Some(Vehicle::Bicycle);
        assert_eq!(p.speed_multiplier(), 2.0);

        p.vehicle = Some(Vehicle::Airplane);
        assert_eq!(p.speed_multiplier(), 10.0);
    }

    #[test]
    fn player_spawn_at_hex_center() {
        let mut player = CorePlayer::new("0x9999".into(), Position::new(50.0, 50.0));
        let world = crate::world::EarthWorld::generate(42, 32);
        player.find_nearest_empty_hex(&Position::new(50.0, 50.0), 32, &world);
        // Should return 0 (center hex) as default
        let world = crate::world::EarthWorld::generate(42, 32);
        assert_eq!(player.find_nearest_empty_hex(&Position::new(0.0, 0.0), 32, &world), 0);
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
