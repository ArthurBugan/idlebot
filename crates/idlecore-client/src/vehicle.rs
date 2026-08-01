//! Client-side vehicle system.
//!
//! Handles vehicle inventory, purchase/equip requests to server,
//! and vehicle display UI.

use serde::{Deserialize, Serialize};

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
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Bicycle => "Bicycle",
            Self::Scooter => "Scooter",
            Self::Motorcycle => "Motorcycle",
            Self::Boat => "Boat",
            Self::Airplane => "Airplane",
        }
    }

    pub fn purchase_cost(&self) -> u64 {
        match self {
            Self::None => 0,
            Self::Bicycle => 500,
            Self::Scooter => 1000,
            Self::Motorcycle => 2500,
            Self::Boat => 2000,
            Self::Airplane => 10000,
        }
    }

    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::Bicycle => 2.0,
            Self::Scooter => 3.0,
            Self::Motorcycle => 5.0,
            Self::Boat => 4.0,
            Self::Airplane => 10.0,
        }
    }
}

/// Client-side vehicle inventory entry.
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

    /// Get the speed multiplier for this vehicle.
    pub fn speed_multiplier(&self) -> f32 {
        self.vehicle_type.speed_multiplier()
    }
}

/// Purchase a vehicle via server RPC.
pub fn client_purchase_vehicle(v_type: VehicleType) -> Result<(), String> {
    // In a real implementation, this would send an RPC to the server.
    // For now, we simulate the purchase locally.
    println!("[CLIENT] Purchase request sent for {}.", v_type.display_name());
    Ok(())
}

/// Equip a vehicle via server RPC.
pub fn client_equip_vehicle(v_type: VehicleType) -> Result<(), String> {
    // In a real implementation, this would send an RPC to the server.
    println!("[CLIENT] Equip request sent for {}.", v_type.display_name());
    Ok(())
}

/// Unequip the current vehicle via server RPC.
pub fn client_unequip_vehicle() -> Result<(), String> {
    // In a real implementation, this would send an RPC to the server.
    println!("[CLIENT] Unequip request sent.");
    Ok(())
}

/// Get all available vehicles for UI display.
pub fn available_vehicles() -> Vec<VehicleType> {
    vec![
        VehicleType::Bicycle,
        VehicleType::Scooter,
        VehicleType::Motorcycle,
        VehicleType::Boat,
        VehicleType::Airplane,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vehicle_types_available() {
        let vehicles = available_vehicles();
        assert_eq!(vehicles.len(), 5);
    }

    #[test]
    fn test_vehicle_speed_multipliers() {
        assert_eq!(VehicleType::Bicycle.speed_multiplier(), 2.0);
        assert_eq!(VehicleType::Scooter.speed_multiplier(), 3.0);
        assert_eq!(VehicleType::Motorcycle.speed_multiplier(), 5.0);
        assert_eq!(VehicleType::Boat.speed_multiplier(), 4.0);
        assert_eq!(VehicleType::Airplane.speed_multiplier(), 10.0);
    }

    #[test]
    fn test_vehicle_purchase_costs() {
        assert_eq!(VehicleType::Bicycle.purchase_cost(), 500);
        assert_eq!(VehicleType::Scooter.purchase_cost(), 1000);
        assert_eq!(VehicleType::Motorcycle.purchase_cost(), 2500);
        assert_eq!(VehicleType::Boat.purchase_cost(), 2000);
        assert_eq!(VehicleType::Airplane.purchase_cost(), 10000);
    }

    #[test]
    fn test_vehicle_new() {
        let v = Vehicle::new(VehicleType::Bicycle);
        assert_eq!(v.vehicle_type, VehicleType::Bicycle);
        assert!(!v.equipped);
        assert!(!v.purchased);
    }

    #[test]
    fn test_vehicle_display_names() {
        assert_eq!(VehicleType::Bicycle.display_name(), "Bicycle");
        assert_eq!(VehicleType::Scooter.display_name(), "Scooter");
        assert_eq!(VehicleType::Motorcycle.display_name(), "Motorcycle");
        assert_eq!(VehicleType::Boat.display_name(), "Boat");
        assert_eq!(VehicleType::Airplane.display_name(), "Airplane");
    }
}
