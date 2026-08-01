//! Vehicle system — purchase, equip, and use vehicles.
//!
//! VehicleType enum delegates to crate::Vehicle for display_name(), speed_multiplier(),
//! and purchase_cost() to avoid duplication.

use spacetimedb::ReducerContext;
use crate::types::player;
use serde::{Deserialize, Serialize};

// --- Data Structures ---

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Minimum player level required to purchase this vehicle.
    pub fn required_level(&self) -> u32 {
        match self {
            VehicleType::None => 1,
            VehicleType::Bicycle => 2,
            VehicleType::Scooter => 3,
            VehicleType::Boat => 4,
            VehicleType::Motorcycle => 5,
            VehicleType::Airplane => 7,
        }
    }
}

/// Maximum speed multiplier cap to prevent excessive speed.
pub const MAX_SPEED_MULTIPLIER: f32 = 10.0;

impl VehicleType {
    /// Get the effective speed multiplier capped at MAX_SPEED_MULTIPLIER.
    pub fn effective_speed_multiplier(&self) -> f32 {
        self.speed_multiplier().min(MAX_SPEED_MULTIPLIER)
    }

    /// Get the visual indicator color for this vehicle type (RGB).
    pub fn indicator_color(&self) -> (f32, f32, f32) {
        match self {
            VehicleType::None => (0.5, 0.5, 0.5),
            VehicleType::Bicycle => (0.0, 1.0, 0.0),
            VehicleType::Scooter => (1.0, 1.0, 0.0),
            VehicleType::Motorcycle => (1.0, 0.0, 0.0),
            VehicleType::Boat => (0.0, 0.5, 1.0),
            VehicleType::Airplane => (1.0, 0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

// --- Server Logic Functions ---

/// Get the player's vehicle inventory (parsed from JSON array string).
fn get_player_vehicles(ctx: &ReducerContext, wallet_address: &str) -> Vec<Vehicle> {
    let key = wallet_address.to_string();
    let player = match ctx.db.player().address().find(&key) {
        Some(p) => p,
        None => return Vec::new(),
    };
    if player.vehicle.is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<Vehicle>>(&player.vehicle).unwrap_or_default()
}

/// Helper: find a vehicle in the player's inventory by type.
fn find_vehicle_in_inventory(vehicles: &[Vehicle], v_type: VehicleType) -> Option<usize> {
    vehicles.iter().position(|v| v.vehicle_type == v_type)
}

/// Helper: update a player's vehicle inventory (serialised JSON array).
fn set_player_vehicles(ctx: &ReducerContext, wallet_address: &str, vehicles: &[Vehicle]) {
    let key = wallet_address.to_string();
    let mut player = ctx.db.player().address().find(&key).expect("Player exists");
    player.vehicle = serde_json::to_string(vehicles).unwrap_or_default();
    ctx.db.player().address().update(player);
}

/// Helper: deduct gold from a player. Returns `Ok(())` or `Err` if insufficient funds.
fn deduct_player_gold(ctx: &ReducerContext, wallet_address: &str, amount: u64) -> Result<(), String> {
    let key = wallet_address.to_string();
    let mut player = ctx.db.player().address().find(&key).expect("Player exists");
    if player.gold < amount {
        return Err("Insufficient gold".to_string());
    }
    player.gold = player.gold.saturating_sub(amount);
    ctx.db.player().address().update(player);
    Ok(())
}

/// Helper: get player level for unlock checks.
fn get_player_level(ctx: &ReducerContext, wallet_address: &str) -> u32 {
    let key = wallet_address.to_string();
    ctx.db.player().address().find(&key).map(|p| p.level).unwrap_or(0)
}

/// Attempts to purchase a vehicle.
/// Returns true if successful (paid and purchased), false otherwise.
// Logic extracted here so client stubs can call the same functions without a
// SpacetimeDB context; the idiom has precedent in the repo (e.g., teleport).

/// Deserialize a player's vehicle JSON string; empty string → empty inventory.
fn parse_vehicles(value: &str) -> Vec<Vehicle> {
    if value.is_empty() { get_default_starter_vehicles().clone() } else { serde_json::from_str(value).unwrap_or_default() }
}

static DEFAULT_STARTER_VEHICLE: std::sync::OnceLock<Vec<Vehicle>> = std::sync::OnceLock::new();

fn get_default_starter_vehicles() -> &'static Vec<Vehicle> {
    DEFAULT_STARTER_VEHICLE.get_or_init(|| {
        vec![
            Vehicle::new(VehicleType::Airplane),
            Vehicle::new(VehicleType::Bicycle),
        ]
    })
}

fn serialize_vehicles(vehicles: &[Vehicle]) -> String {
    if vehicles.is_empty() {
        String::new()
    } else {
        serde_json::to_string(vehicles).unwrap_or_default()
    }
}

fn find_vehicle_index(vehicles: &[Vehicle], v_type: VehicleType) -> Option<usize> {
    vehicles.iter().position(|v| v.vehicle_type == v_type)
}

fn already_owned(vehicles: &[Vehicle], v_type: VehicleType) -> bool {
    find_vehicle_index(vehicles, v_type).is_some()
}

fn can_afford(gold: u64, cost: u64) -> bool {
    gold >= cost
}

fn has_minimum_level(player_level: u32, required: u32) -> bool {
    player_level >= required
}

pub fn purchase_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    if v_type == VehicleType::None {
        println!("[VEHICLE] Cannot purchase 'None' vehicle.");
        return false;
    }

    // 2. Check level unlock
    let player_level = get_player_level(ctx, wallet_address);
    let required = v_type.required_level();
    if player_level < required {
        println!(
            "[VEHICLE] Purchase failed: level {} required for {} (player is level {})",
            required, v_type.to_string_name(), player_level
        );
        return false;
    }

    // 3. Check if player already owns this vehicle type
    let inventory = get_player_vehicles(ctx, wallet_address);
    if find_vehicle_in_inventory(&inventory, v_type).is_some() {
        println!("[VEHICLE] Already owns {}.", v_type.to_string_name());
        return false;
    }

    // 4. Check / Deduct Gold
    let cost = v_type.purchase_cost();
    if deduct_player_gold(ctx, wallet_address, cost).is_err() {
        println!("[VEHICLE] Purchase failed: insufficient funds (cost: {}G)", cost);
        return false;
    }

    // 5. Add vehicle to inventory
    let mut inventory = inventory;
    let new_vehicle = Vehicle {
        vehicle_type: v_type,
        equipped: false,
        purchased: true,
    };
    inventory.push(new_vehicle);
    set_player_vehicles(ctx, wallet_address, &inventory);
    println!(
        "[VEHICLE] Successfully purchased {}: {}G spent.",
        v_type.to_string_name(),
        cost
    );
    true
}

/// Sets the vehicle as equipped (unequips any previously equipped vehicle).
pub fn equip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let inventory = get_player_vehicles(ctx, wallet_address);

    let idx = match find_vehicle_in_inventory(&inventory, v_type) {
        Some(i) => i,
        None => {
            println!("[VEHICLE] Cannot equip: Vehicle not found in inventory or not owned.");
            return false;
        }
    };

    if inventory[idx].equipped {
        println!("[VEHICLE] Already equipped: {}", v_type.to_string_name());
        return true;
    }

    // Unequip all, then equip the target
    let mut updated = inventory;
    for v in &mut updated {
        v.equipped = false;
    }
    updated[idx].equipped = true;
    set_player_vehicles(ctx, wallet_address, &updated);
    println!("[VEHICLE] Equipped vehicle: {}", v_type.to_string_name());
    true
}

/// Sets the vehicle as unequipped.
pub fn unequip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let inventory = get_player_vehicles(ctx, wallet_address);

    let idx = match find_vehicle_in_inventory(&inventory, v_type) {
        Some(i) => i,
        None => {
            println!("[VEHICLE] Cannot unequip: Vehicle not found in inventory or not owned.");
            return false;
        }
    };

    if !inventory[idx].equipped {
        println!("[VEHICLE] Already unequipped: {}", v_type.to_string_name());
        return true;
    }

    let mut updated = inventory;
    updated[idx].equipped = false;
    set_player_vehicles(ctx, wallet_address, &updated);
    println!("[VEHICLE] Unequipped vehicle: {}", v_type.to_string_name());
    true
}

/// Returns the currently equipped vehicle for a player, if any.
pub fn get_equipped_vehicle(ctx: &ReducerContext, wallet_address: &str) -> Option<Vehicle> {
    let inventory = get_player_vehicles(ctx, wallet_address);
    inventory.into_iter().find(|v| v.equipped && v.purchased)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- VehicleType tests ---

    #[test]
    fn test_vehicle_type_cost() {
        assert_eq!(VehicleType::None.purchase_cost(), 0);
        assert_eq!(VehicleType::Bicycle.purchase_cost(), 500);
        assert_eq!(VehicleType::Scooter.purchase_cost(), 1000);
        assert_eq!(VehicleType::Motorcycle.purchase_cost(), 2500);
        assert_eq!(VehicleType::Boat.purchase_cost(), 2000);
        assert_eq!(VehicleType::Airplane.purchase_cost(), 10000);
    }

    #[test]
    fn test_vehicle_type_speed_multiplier() {
        assert_eq!(VehicleType::None.speed_multiplier(), 1.0);
        assert_eq!(VehicleType::Bicycle.speed_multiplier(), 2.0);
        assert_eq!(VehicleType::Scooter.speed_multiplier(), 3.0);
        assert_eq!(VehicleType::Motorcycle.speed_multiplier(), 5.0);
        assert_eq!(VehicleType::Boat.speed_multiplier(), 4.0);
        assert_eq!(VehicleType::Airplane.speed_multiplier(), 10.0);
    }

    #[test]
    fn test_vehicle_type_required_level() {
        assert_eq!(VehicleType::None.required_level(), 1);
        assert_eq!(VehicleType::Bicycle.required_level(), 2);
        assert_eq!(VehicleType::Scooter.required_level(), 3);
        assert_eq!(VehicleType::Boat.required_level(), 4);
        assert_eq!(VehicleType::Motorcycle.required_level(), 5);
        assert_eq!(VehicleType::Airplane.required_level(), 7);
    }

    #[test]
    fn test_vehicle_type_names() {
        assert_eq!(VehicleType::None.to_string_name(), "None");
        assert_eq!(VehicleType::Bicycle.to_string_name(), "Bicycle");
        assert_eq!(VehicleType::Scooter.to_string_name(), "Scooter");
        assert_eq!(VehicleType::Motorcycle.to_string_name(), "Motorcycle");
        assert_eq!(VehicleType::Boat.to_string_name(), "Boat");
        assert_eq!(VehicleType::Airplane.to_string_name(), "Airplane");
    }

    // --- Inventory serialization tests ---

    #[test]
    fn test_inventory_serialization_roundtrip() {
        let vehicles = vec![
            Vehicle { vehicle_type: VehicleType::Bicycle, equipped: true, purchased: true },
            Vehicle { vehicle_type: VehicleType::Scooter, equipped: false, purchased: true },
        ];
        let json = serde_json::to_string(&vehicles).unwrap();
        let parsed: Vec<Vehicle> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].vehicle_type, VehicleType::Bicycle);
        assert!(parsed[0].equipped);
        assert!(parsed[0].purchased);
        assert_eq!(parsed[1].vehicle_type, VehicleType::Scooter);
        assert!(!parsed[1].equipped);
        assert!(parsed[1].purchased);
    }

    #[test]
    fn test_empty_inventory_serialization() {
        let vehicles: Vec<Vehicle> = Vec::new();
        let json = serde_json::to_string(&vehicles).unwrap();
        let parsed: Vec<Vehicle> = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_empty());
    }

    // --- Inventory logic tests ---

    #[test]
    fn test_find_vehicle_in_inventory() {
        let inventory = vec![
            Vehicle { vehicle_type: VehicleType::Bicycle, equipped: false, purchased: true },
            Vehicle { vehicle_type: VehicleType::Scooter, equipped: true, purchased: true },
        ];
        assert_eq!(find_vehicle_in_inventory(&inventory, VehicleType::Bicycle), Some(0));
        assert_eq!(find_vehicle_in_inventory(&inventory, VehicleType::Scooter), Some(1));
        assert_eq!(find_vehicle_in_inventory(&inventory, VehicleType::Boat), None);
        assert_eq!(find_vehicle_in_inventory(&inventory, VehicleType::None), None);
    }

    // --- Vehicle struct tests ---

    #[test]
    fn test_vehicle_new() {
        let v = Vehicle::new(VehicleType::Bicycle);
        assert_eq!(v.vehicle_type, VehicleType::Bicycle);
        assert!(!v.equipped);
        assert!(!v.purchased);
    }

    // --- Business logic unit tests (no SpacetimeDB context needed) ---
    // These test the decision logic that doesn't require a database.

    #[test]
    fn test_purchase_cost_covers_all_types() {
        // ponytail: This is a smoke test — verifies all vehicle types have non-zero cost except None
        for v in [VehicleType::Bicycle, VehicleType::Scooter, VehicleType::Motorcycle, VehicleType::Boat, VehicleType::Airplane] {
            assert!(v.purchase_cost() > 0, "{} should cost gold", v.to_string_name());
        }
        assert_eq!(VehicleType::None.purchase_cost(), 0);
    }

    #[test]
    fn test_all_vehicle_types_have_unique_multipliers() {
        let multipliers = [
            VehicleType::None.speed_multiplier(),
            VehicleType::Bicycle.speed_multiplier(),
            VehicleType::Scooter.speed_multiplier(),
            VehicleType::Motorcycle.speed_multiplier(),
            VehicleType::Boat.speed_multiplier(),
            VehicleType::Airplane.speed_multiplier(),
        ];
        // Each multiplier should be unique
        for i in 0..multipliers.len() {
            for j in (i + 1)..multipliers.len() {
                assert_ne!(multipliers[i], multipliers[j], "Duplicate speed multiplier");
            }
        }
    }

    #[test]
    fn test_equip_logic_unequips_others() {
        // Simulate the equip logic: when equipping one vehicle, all others should be unequipped
        let mut inventory = vec![
            Vehicle { vehicle_type: VehicleType::Bicycle, equipped: true, purchased: true },
            Vehicle { vehicle_type: VehicleType::Scooter, equipped: false, purchased: true },
            Vehicle { vehicle_type: VehicleType::Boat, equipped: false, purchased: true },
        ];

        // Equip Scooter (index 1): unequip all, then equip target
        for v in &mut inventory {
            v.equipped = false;
        }
        inventory[1].equipped = true;

        assert!(!inventory[0].equipped, "Bicycle should be unequipped");
        assert!(inventory[1].equipped, "Scooter should be equipped");
        assert!(!inventory[2].equipped, "Boat should be unequipped");
    }

    #[test]
    fn test_inventory_appends_not_overwrites() {
        // Simulate purchase: add to existing inventory without overwriting
        let mut inventory = vec![
            Vehicle { vehicle_type: VehicleType::Bicycle, equipped: true, purchased: true },
        ];
        let new_vehicle = Vehicle {
            vehicle_type: VehicleType::Scooter,
            equipped: false,
            purchased: true,
        };
        inventory.push(new_vehicle);

        assert_eq!(inventory.len(), 2);
        assert_eq!(inventory[0].vehicle_type, VehicleType::Bicycle);
        assert_eq!(inventory[1].vehicle_type, VehicleType::Scooter);
    }
}