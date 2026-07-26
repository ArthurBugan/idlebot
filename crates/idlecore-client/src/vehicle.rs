//! Vehicle System
//!
//! Speed multipliers and purchase logic for electric vehicles.
//! Vehicles: None, Bicycle (2x), Scooter (3x), Motorcycle (5x), Boat (4x), Airplane (10x).

use idlebot_core::Vehicle;

/// Types de veículos com multiplicadores de velocidade
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedVehicle {
    None,
    Bicycle,      // 2x speed, 500 gold
    Scooter,      // 3x speed, 1000 gold
    Motorcycle,   // 5x speed, 2500 gold
    Boat,         // 4x speed, 2000 gold
    Airplane,     // 10x speed, 10000 gold
}

impl OwnedVehicle {
    /// Get speed multiplier (1.0 means no vehicle)
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            OwnedVehicle::None => 1.0,
            OwnedVehicle::Bicycle => 2.0,
            OwnedVehicle::Scooter => 3.0,
            OwnedVehicle::Motorcycle => 5.0,
            OwnedVehicle::Boat => 4.0,
            OwnedVehicle::Airplane => 10.0,
        }
    }

    /// Gold cost to purchase this vehicle (from PROPOSAL 2.6)
    pub fn cost(&self) -> u64 {
        match self {
            OwnedVehicle::None => 0,
            OwnedVehicle::Bicycle => 500,
            OwnedVehicle::Scooter => 1_000,
            OwnedVehicle::Motorcycle => 2_500,
            OwnedVehicle::Boat => 2_000,
            OwnedVehicle::Airplane => 10_000,
        }
    }

    /// Get the display name of the vehicle
    pub fn display_name(&self) -> &'static str {
        match self {
            OwnedVehicle::None => "None",
            OwnedVehicle::Bicycle => "Electric Bicycle",
            OwnedVehicle::Scooter => "Electric Scooter",
            OwnedVehicle::Motorcycle => "Electric Motorcycle",
            OwnedVehicle::Boat => "Electric Boat",
            OwnedVehicle::Airplane => "Electric Airplane",
        }
    }

    /// Vehicle name as a string for the client
    pub fn as_str(&self) -> &'static str {
        match self {
            OwnedVehicle::None => "None",
            OwnedVehicle::Bicycle => "Bicycle",
            OwnedVehicle::Scooter => "Scooter",
            OwnedVehicle::Motorcycle => "Motorcycle",
            OwnedVehicle::Boat => "Boat",
            OwnedVehicle::Airplane => "Airplane",
        }
    }
}

impl From<&Vehicle> for OwnedVehicle {
    fn from(v: &Vehicle) -> Self {
        match v {
            Vehicle::None => OwnedVehicle::None,
            Vehicle::Bicycle => OwnedVehicle::Bicycle,
            Vehicle::Scooter => OwnedVehicle::Scooter,
            Vehicle::Motorcycle => OwnedVehicle::Motorcycle,
            Vehicle::Boat => OwnedVehicle::Boat,
            Vehicle::Airplane => OwnedVehicle::Airplane,
        }
    }
}

/// Purchase a vehicle if the player has enough gold.
/// Returns: (new_vehicle, deducted_gold, message)
pub fn purchase_vehicle(player: &mut ClientPlayer, vehicle_type: &Vehicle) -> VehiclePurchaseResult {
    // Deduplicate — player can only have one vehicle
    if player.owned_vehicle.is_some() {
        return VehiclePurchaseResult {
            success: false,
            message: "Already owns a vehicle".to_string(),
            gold_deducted: 0,
        };
    }

    // Calculate cost
    let cost = vehicle.cost_gold();

    if player.gold >= cost {
        // Deduct gold
        player.gold -= cost;
        player.owned_vehicle = Some(Vehicle::from(vehicle_type));
        player.owned_vehicle = OwnedVehicle::from(vehicle_type);

        VehiclePurchaseResult {
            success: true,
            message: format!("Purchased {}", vehicle_type.display_name()).to_string(),
            gold_deducted: cost,
        }
    } else {
        VehiclePurchaseResult {
            success: false,
            message: format!("Need {} gold, have {}", cost, player.gold).to_string(),
            gold_deducted: 0,
        }
    }
}

/// Apply vehicle purchase result to update level (if gold changed, level may recalculate)
pub fn apply_purchase_result(player: &mut ClientPlayer, result: &VehiclePurchaseResult) {
    if result.success {
        // Recalculate level after gold change
        player.level = crate::progression::calculate_level(player.xp);
        tracing::info!(
            "Player now has vehicle '{}', {} gold remaining, level {}",
            result.message,
            player.gold,
            player.level
        );
    }
}

/// Get list of available vehicles for the shop (with name, speed, cost)
pub fn available_vehicles() -> Vec<VehicleShopItem> {
    vec![
        VehicleShopItem {
            vehicle: Vehicle::None,
            name: "None".to_string(),
            speed: 1.0,
            cost: 0,
        },
        VehicleShopItem {
            vehicle: Vehicle::Bicycle,
            name: "Electric Bicycle".to_string(),
            speed: 2.0,
            cost: 500,
        },
        VehicleShopItem {
            vehicle: Vehicle::Scooter,
            name: "Electric Scooter".to_string(),
            speed: 3.0,
            cost: 1_000,
        },
        VehicleShopItem {
            vehicle: Vehicle::Motorcycle,
            name: "Electric Motorcycle".to_string(),
            speed: 5.0,
            cost: 2_500,
        },
        VehicleShopItem {
            vehicle: Vehicle::Boat,
            name: "Electric Boat".to_string(),
            speed: 4.0,
            cost: 2_000,
        },
        VehicleShopItem {
            vehicle: Vehicle::Airplane,
            name: "Electric Airplane".to_string(),
            speed: 10.0,
            cost: 10_000,
        },
    ]
}

#[derive(Debug)]
pub struct VehiclePurchaseResult {
    pub success: bool,
    pub message: String,
    pub gold_deducted: u64,
}

#[derive(Debug, Clone)]
pub struct VehicleShopItem {
    pub vehicle: Vehicle,
    pub name: String,
    pub speed: f32,
    pub cost: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_multipliers() {
        assert_eq!(OwnedVehicle::None.speed_multiplier(), 1.0);
        assert_eq!(OwnedVehicle::Bicycle.speed_multiplier(), 2.0);
        assert_eq!(OwnedVehicle::Scooter.speed_multiplier(), 3.0);
        assert_eq!(OwnedVehicle::Motorcycle.speed_multiplier(), 5.0);
        assert_eq!(OwnedVehicle::Boat.speed_multiplier(), 4.0);
        assert_eq!(OwnedVehicle::Airplane.speed_multiplier(), 10.0);
    }

    #[test]
    fn vehicle_costs() {
        assert_eq!(OwnedVehicle::None.cost(), 0);
        assert_eq!(OwnedVehicle::Bicycle.cost(), 500);
        assert_eq!(OwnedVehicle::Scooter.cost(), 1_000);
        assert_eq!(OwnedVehicle::Motorcycle.cost(), 2_500);
        assert_eq!(OwnedVehicle::Boat.cost(), 2_000);
        assert_eq!(OwnedVehicle::Airplane.cost(), 10_000);
    }

    #[test]
    fn purchase_vehicle_affordable() {
        let mut player = ClientPlayer::new_spawn(None, Vec3::ZERO, 1, 0, 600, 0, vec![]);
        let result = purchase_vehicle(&mut player, &Vehicle::Bicycle);
        assert!(result.success);
        assert_eq!(player.owned_vehicle, Some(Vehicle::Bicycle));
        assert_eq!(player.gold, 100); // 600 - 500
        assert_eq!(result.gold_deducted, 500);
    }

    #[test]
    fn purchase_vehicle_too_expensive() {
        let mut player = ClientPlayer::new_spawn(None, Vec3::ZERO, 1, 0, 400, 0, vec![]);
        let result = purchase_vehicle(&mut player, &Vehicle::Bicycle);
        assert!(!result.success);
        assert_eq!(player.owned_vehicle, None);
        assert_eq!(player.gold, 400); // Should not change
    }

    #[test]
    fn purchase_vehicle_already_owned() {
        let mut player = ClientPlayer::new_spawn(Some(Vehicle::Bicycle), Vec3::ZERO, 1, 0, 600, 0, vec![]);
        let result = purchase_vehicle(&mut player, &Vehicle::Scooter);
        assert!(!result.success);
        assert_eq!(player.owned_vehicle, Some(Vehicle::Bicycle));
    }

    #[test]
    fn display_names() {
        assert_eq!(OwnedVehicle::Bicycle.display_name(), "Electric Bicycle");
        assert_eq!(OwnedVehicle::Scooter.display_name(), "Electric Scooter");
        assert_eq!(OwnedVehicle::Motorcycle.display_name(), "Electric Motorcycle");
        assert_eq!(OwnedVehicle::Boat.display_name(), "Electric Boat");
        assert_eq!(OwnedVehicle::Airplane.display_name(), "Electric Airplane");
    }

    #[test]
    fn available_vehicles_count() {
        let vehicles = available_vehicles();
        assert_eq!(vehicles.len(), 6);
    }
}
