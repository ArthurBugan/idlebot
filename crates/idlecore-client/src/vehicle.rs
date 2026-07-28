//! Client-side vehicle stub.
//!
//! In a real implementation, this module would handle networking calls/RPCs to the
//! server. For now, it's a placeholder.

use serde::{Deserialize, Serialize};

/// Vehicle type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleType {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

impl VehicleType {
    pub fn to_string_name(&self) -> &'static str {
        match self {
            VehicleType::None => "None",
            VehicleType::Bicycle => "Bicycle",
            VehicleType::Scooter => "Scooter",
            VehicleType::Motorcycle => "Motorcycle",
            VehicleType::Boat => "Boat",
            VehicleType::Airplane => "Airplane",
        }
    }

    pub fn purchase_cost(&self) -> u64 {
        match self {
            VehicleType::None => 0,
            VehicleType::Bicycle => 500,
            VehicleType::Scooter => 1000,
            VehicleType::Motorcycle => 2500,
            VehicleType::Boat => 2000,
            VehicleType::Airplane => 10000,
        }
    }

    pub fn speed_multiplier(&self) -> f32 {
        match self {
            VehicleType::None => 1.0,
            VehicleType::Bicycle => 2.0,
            VehicleType::Scooter => 3.0,
            VehicleType::Motorcycle => 5.0,
            VehicleType::Boat => 4.0,
            VehicleType::Airplane => 10.0,
        }
    }
}

/// Client-side vehicle info representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub vehicle_type: VehicleType,
    pub equipped: bool,
    pub purchased: bool,
}

impl Vehicle {
    pub fn new(v_type: VehicleType) -> Self {
        Vehicle {
            vehicle_type: v_type,
            equipped: false,
            purchased: false,
        }
    }
}

/// Stub: Called when the player attempts to purchase a vehicle.
pub fn client_purchase_vehicle(v_type: VehicleType, _player_data_in_game: bool) -> Result<(), &'static str> {
    // In a real implementation, this would send an RPC to the server.
    println!("[CLIENT] Purchase request sent for {:?}. (stub)", v_type);
    Ok(())
}

/// Stub: Called when the player attempts to equip a vehicle.
pub fn client_equip_vehicle(v_type: VehicleType) -> Result<(), &'static str> {
    println!("[CLIENT] Sending equip request for {:?}... (stub)", v_type);
    Ok(())
}

/// Stub: Called when the player attempts to unequip a vehicle.
pub fn client_unequip_vehicle(v_type: VehicleType) -> Result<(), &'static str> {
    println!("[CLIENT] Sending unequip request for {:?}... (stub)", v_type);
    Ok(())
}
