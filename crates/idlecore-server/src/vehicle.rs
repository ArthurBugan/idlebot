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
}

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

// --- Server Logic Functions ---

/// Helper: find player and get current vehicle string.
fn get_player_vehicle(ctx: &ReducerContext, wallet_address: &str) -> Option<Vehicle> {
    let key = wallet_address.to_string();
    let player = ctx.db.player().address().find(&key)?;
    if player.vehicle.is_empty() {
        return None;
    }
    serde_json::from_str(&player.vehicle).ok()
}

/// Helper: update a player's vehicle field with a serialised Vehicle.
fn set_player_vehicle(ctx: &ReducerContext, wallet_address: &str, vehicle: &Vehicle) {
    let key = wallet_address.to_string();
    let mut player = ctx.db.player().address().find(&key).expect("Player exists");
    player.vehicle = serde_json::to_string(vehicle).unwrap_or_default();
    ctx.db.player().address().update(player);
}

/// Helper: deduct gold from a player.
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

/// Attempts to purchase a vehicle.
/// Returns true if successful (paid and purchased), false otherwise.
pub fn purchase_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let required_cost = v_type.purchase_cost();

    // 1. Check if player already owns this type
    if let Some(existing) = get_player_vehicle(ctx, wallet_address) {
        if existing.vehicle_type == v_type && existing.purchased {
            println!("[VEHICLE] Already owned.");
            return false;
        }
    }

    // 2. Check/Deduct Gold
    if deduct_player_gold(ctx, wallet_address, required_cost).is_err() {
        println!("[VEHICLE] Purchase failed: insufficient funds.");
        return false;
    }

    // 3. Mark vehicle as purchased/owned
    let new_vehicle = Vehicle {
        vehicle_type: v_type,
        equipped: false,
        purchased: true,
    };
    set_player_vehicle(ctx, wallet_address, &new_vehicle);
    println!("[VEHICLE] Successfully purchased {}: {}G spent.", v_type.to_string_name(), required_cost);
    true
}

/// Sets the vehicle as equipped.
pub fn equip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let current_vehicle = match get_player_vehicle(ctx, wallet_address) {
        Some(v) => v,
        None => {
            println!("[VEHICLE] Cannot equip: Vehicle mismatch or not owned.");
            return false;
        }
    };

    if current_vehicle.vehicle_type != v_type {
        println!("[VEHICLE] Cannot equip: Vehicle mismatch or not owned.");
        return false;
    }

    if current_vehicle.equipped {
        println!("[VEHICLE] Already equipped.");
        return true;
    }

    let mut vehicle_to_update = current_vehicle;
    vehicle_to_update.equipped = true;
    set_player_vehicle(ctx, wallet_address, &vehicle_to_update);
    println!("[VEHICLE] Equipped vehicle: {}", v_type.to_string_name());
    true
}

/// Sets the vehicle as unequipped.
pub fn unequip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let current_vehicle = match get_player_vehicle(ctx, wallet_address) {
        Some(v) => v,
        None => {
            println!("[VEHICLE] Cannot unequip: Vehicle mismatch or not owned.");
            return false;
        }
    };

    if current_vehicle.vehicle_type != v_type {
        println!("[VEHICLE] Cannot unequip: Vehicle mismatch or not owned.");
        return false;
    }

    if !current_vehicle.equipped {
        println!("[VEHICLE] Already unequipped.");
        return true;
    }

    let mut vehicle_to_update = current_vehicle;
    vehicle_to_update.equipped = false;
    set_player_vehicle(ctx, wallet_address, &vehicle_to_update);
    println!("[VEHICLE] Unequipped vehicle: {}", v_type.to_string_name());
    true
}
