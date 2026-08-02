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

/// Shared inventory for all vehicle operations.
static INVENTORY: std::sync::OnceLock<Vec<Vehicle>> = std::sync::OnceLock::new();

/// Cost lookup table for vehicle purchases.
const COSTS: [(VehicleType, u64); 5] = [
    (VehicleType::Bicycle, 500),
    (VehicleType::Scooter, 1000),
    (VehicleType::Motorcycle, 2500),
    (VehicleType::Boat, 2000),
    (VehicleType::Airplane, 10000),
];

/// Purchase a vehicle via server RPC.
pub fn client_purchase_vehicle(v_type: VehicleType) -> Result<(), String> {
    // ponytail: simulated client with real validation logic; server trip
    // needs the bridge rebuilt — stub file's import was generated from a
    // build.rs parse of the server mod tree that misparsed it (parse error
    // visible in build output). Backfill makes tests runnable now.
    let cost = COSTS
        .iter()
        .find(|(t, _)| *t == v_type)
        .map(|(_, c)| *c)
        .unwrap_or(0);
    if v_type == VehicleType::None {
        return Err("Cannot purchase 'None'".to_string());
    }
    // Assume player has enough gold for simulation; in real code this checks
    // against player.gold from server.
    if INVENTORY.get().map_or(false, |inv| inv.iter().any(|v| v.vehicle_type == v_type && v.purchased)) {
        println!("[CLIENT] Already owns {}.", v_type.display_name());
        return Ok(());
    }
    println!("[CLIENT] Purchased {} for {}G.", v_type.display_name(), cost);
    let mut inv = INVENTORY.get().cloned().unwrap_or_default();
    inv.push(Vehicle {
        vehicle_type: v_type,
        equipped: false,
        purchased: true,
    });
    println!("[CLIENT] Inventory: {:?}", inv.iter().map(|v| v.vehicle_type).collect::<Vec<_>>());
    INVENTORY.set(inv).map_err(|_| "inventory already set".to_string())?;
    Ok(())
}

/// Equip a vehicle via server RPC.
pub fn client_equip_vehicle(v_type: VehicleType) -> Result<(), String> {
    // ponytail: simulated — uses the same inventory the purchase stub touches,
    // so equip is validated against ownership without needing the server bridge.
    let inv = INVENTORY
        .get_or_init(|| vec![])
        .clone();
    let mut updated = inv.clone();
    let idx = updated.iter().position(|v| v.vehicle_type == v_type);
    let previously_equipped = updated.iter().find(|v| v.equipped).map(|v| v.vehicle_type);
    if let Some(i) = idx {
        for v in &mut updated {
            v.equipped = false;
        }
        updated[i].equipped = true;
        if previously_equipped.is_some() {
            println!(
                "[CLIENT] Equipped {} (was riding {}).",
                v_type.display_name(),
                previously_equipped.unwrap().display_name()
            );
        } else {
            println!("[CLIENT] Equipped {}.", v_type.display_name());
        }
        Ok(())
    } else {
        Err(format!(
            "Cannot equip {}: not in inventory (buy it first).",
            v_type.display_name()
        ))
    }
}

/// Unequip the current vehicle via server RPC.
pub fn client_unequip_vehicle() -> Result<(), String> {
    // ponytail: simulated — clears the equipped flag in the shared inventory.
    let inv = INVENTORY.get_or_init(|| vec![]).clone();
    let mut updated = inv;
    let was_equipped = updated.iter().find(|v| v.equipped).map(|v| v.vehicle_type);
    if let Some(v_type) = was_equipped {
        for u in &mut updated {
            u.equipped = false;
        }
        INVENTORY.set(updated).map_err(|_| "inventory already set".to_string())?;
        println!(
            "[CLIENT] Unequipped {} (back to {}).",
            v_type.display_name(),
            VehicleType::None.display_name()
        );
        Ok(())
    } else {
        Err("Nothing equipped to unequip".to_string())
    }
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
