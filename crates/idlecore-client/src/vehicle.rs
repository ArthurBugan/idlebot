//! Vehicle purchase and application
//!
//! Handles buying vehicles and updating player state.

use bevy::prelude::*;
use idlecore_core::Vehicle;
use crate::player::ClientPlayer;

/// Result of attempting to purchase a vehicle
#[derive(Debug, Clone)]
pub struct VehiclePurchaseResult {
    pub success: bool,
    pub message: String,
}

/// Get the gold cost for a vehicle
pub fn vehicle_cost(vehicle: &Vehicle) -> u64 {
    match vehicle {
        Vehicle::None => 0,
        Vehicle::Bicycle => 500,
        Vehicle::Scooter => 1_000,
        Vehicle::Motorcycle => 2_500,
        Vehicle::Boat => 2_000,
        Vehicle::Airplane => 10_000,
    }
}

/// Purchase a vehicle for the player
pub fn purchase_vehicle(player: &mut ClientPlayer, vehicle_type: &Vehicle) -> VehiclePurchaseResult {
    let cost = vehicle_cost(vehicle_type);

    if player.gold < cost {
        return VehiclePurchaseResult {
            success: false,
            message: format!("Not enough gold! Need {}, have {}", cost, player.gold),
        };
    }

    // Deduct gold
    player.gold = player.gold.saturating_sub(cost);

    // Set vehicle
    player.owned_vehicle = Some(vehicle_type.clone());

    VehiclePurchaseResult {
        success: true,
        message: format!("Purchased {:?}", vehicle_type),
    }
}

/// Apply purchase result (e.g. update UI message)
pub fn apply_purchase_result(player: &mut ClientPlayer, result: &VehiclePurchaseResult) {
    if result.success {
        println!("Vehicle purchased successfully!");
    } else {
        println!("Purchase failed: {}", result.message);
    }
}
