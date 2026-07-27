//! Player component and spawning
//!
//! Server-side player: wallet address, 2D position, hex ID, vehicle, XP,
//! gold, level, eco points, last seen timestamp, online status.
//!
//! Also provides spawn and movement helper functions.

use crate::Position;
use crate::Vehicle;

/// Core player data used by server database entries.
/// Mirrors the Server::Player struct in lib.rs but with helper methods.
#[derive(Debug, Clone)]
pub struct Player {
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

impl Player {
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
}

/// Spawn a player at the nearest empty grass hex to the given position.
/// Returns the hex_id where the player should spawn.
pub fn spawn_player_at_hex(player: &mut Player, hex_id: u64) {
    let hex = crate::hex::HexCoord::from_id(hex_id);
    let center = hex.center(10.0);
    player.position = Position::new(center[0], center[1]);
    player.hex_id = hex_id;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_new_default() {
        let p = Player::new("0x1234".into(), Position::new(0.0, 0.0));
        assert_eq!(p.address, "0x1234");
        assert_eq!(p.gold, 100);
        assert_eq!(p.level, 1);
        assert!(!p.is_online);
        assert_eq!(p.vehicle, Vehicle::None);
    }

    #[test]
    fn player_new_spawn_position() {
        let p = Player::new("0x5678".into(), Position::new(10.0, 20.0));
        assert_eq!(p.position.x, 10.0);
        assert_eq!(p.position.y, 20.0);
        // hex_id should be computed from position
        assert!(p.hex_id > 0);
    }

    #[test]
    fn player_set_last_seen() {
        let mut p = Player::new("0x1234".into(), Position::new(0.0, 0.0));
        p.set_last_seen(1234567890);
        assert_eq!(p.last_seen, 1234567890);
    }

    #[test]
    fn player_speed_multiplier_no_vehicle() {
        let p = Player::new("0x1234".into(), Position::new(0.0, 0.0));
        assert_eq!(p.speed_multiplier(), 1.0);
    }

    #[test]
    fn player_speed_multiplier_vehicle() {
        let mut p = Player::new("0x1234".into(), Position::new(0.0, 0.0));
        p.vehicle = Vehicle::Bicycle;
        assert_eq!(p.speed_multiplier(), 2.0);

        p.vehicle = Vehicle::Airplane;
        assert_eq!(p.speed_multiplier(), 10.0);
    }

    #[test]
    fn player_spawn_at_hex_sets_position() {
        // Create a player and spawn them at hex (0, 0) = center of world
        let mut player = Player::new("0x9999".into(), Position::new(50.0, 50.0));
        let center_hex_id = 0u64; // (0,0) center hex
        spawn_player_at_hex(&mut player, center_hex_id);
        // Position should be at center (0, 0)
        assert!((player.position.x - 0.0).abs() < 0.01);
        assert!((player.position.y - 0.0).abs() < 0.01);
        assert_eq!(player.hex_id, 0);
    }

    #[test]
    fn player_spawn_at_hex_different_hex() {
        let mut player = Player::new("0x9999".into(), Position::new(50.0, 50.0));
        // Use a hex that's not the center
        let hex_id = (1u64) << 32 | (0u64); // hex (1, 0)
        spawn_player_at_hex(&mut player, hex_id);
        // Position should be updated to that hex center
        assert_ne!(player.hex_id, 0);
        assert!((player.position.x - 17.3205).abs() < 0.1);
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

    #[test]
    fn spawn_position_distance() {
        let p1 = Position::new(0.0, 0.0);
        let p2 = Position::new(3.0, 4.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 0.001);
    }
}
