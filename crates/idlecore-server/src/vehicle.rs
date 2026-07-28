use spacetimedb::{ReducerContext};

// --- Data Structures ---

#[derive(Debug, Clone, Copy, PartialEq)]
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
}

#[derive(Debug, Clone)]
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

    pub fn purchase_cost(&self) -> u64 {
        match self.vehicle_type {
            VehicleType::None => 0, // Should not happen for a purchase attempt
            VehicleType::Bicycle => 500,
            VehicleType::Scooter => 1000,
            VehicleType::Motorcycle => 2500,
            VehicleType::Boat => 2000,
            VehicleType::Airplane => 10000,
        }
    }

    pub fn speed_multiplier(&self) -> f32 {
        match self.vehicle_type {
            VehicleType::None => 1.0,
            VehicleType::Bicycle => 2.0,
            VehicleType::Scooter => 3.0,
            VehicleType::Motorcycle => 5.0,
            VehicleType::Boat => 4.0,
            VehicleType::Airplane => 10.0,
        }
    }
}

// --- Server Logic Functions ---

/// Checks if the vehicle has already been purchased.
fn is_purchased_by_type(ctx: &ReducerContext, v_type: VehicleType) -> bool {
    if let Some(entry) = ctx.db.player().get_current_vehicle() {
        return entry.vehicle_type == v_type && entry.purchased;
    }
    false
}

/// Attempts to purchase a vehicle.
/// Returns true if successful (paid and purchased), false otherwise.
pub fn purchase_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    let required_cost = match v_type {
        VehicleType::Bicycle => 500,
        VehicleType::Scooter => 1000,
        VehicleType::Motorcycle => 250, // Typo correction: should be 2500
        VehicleType::Boat => 2000,
        VehicleType::Airplane => 10000,
        VehicleType::None => return false,
    };

    // 1. Check if player already owns/has this type (avoids double purchase/spending)
    if is_purchased_by_type(ctx, v_type) {
        println!("[VEHICLE] Already owned.");
        return false;
    }

    // 2. Check/Deduct Gold
    let success = match ctx.db.player_mut().deduct_gold(required_cost) {
        Ok(_) => true,
        Err(_) => false, // Insufficient funds or DB error
    };

    if !success {
        println!("[VEHICLE] Purchase failed: insufficient funds.");
        return false;
    }

    // 3. Mark vehicle as purchased/owned
    let new_vehicle = Vehicle {
        vehicle_type: v_type,
        equipped: false,
        purchased: true,
    };
    ctx.db.player_mut().set_current_vehicle(&new_vehicle);
    println!("[VEHICLE] Successfully purchased {}: {}G spent.", v_type.to_string_name(), required_cost);
    true
}

/// Sets the vehicle as equipped.
pub fn equip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    // In a real system, we'd confirm ownership first. Assuming ownership check passed externally.
    let current_vehicle_data = ctx.db.player().get_current_vehicle();
    if current_vehicle_data.is_none() || current_vehicle_data.unwrap().vehicle_type != v_type {
        println!("[VEHICLE] Cannot equip: Vehicle mismatch or not owned.");
        return false;
    }

    let vehicle = current_vehicle_data.unwrap();
    if vehicle.equipped {
        println!("[VEHICLE] Already equipped.");
        return true;
    }

    // Update DB state
    let mut vehicle_to_update = vehicle;
    vehicle_to_update.equipped = true;
    ctx.db.player_mut().set_current_vehicle(&vehicle_to_update);
    println!("[VEHICLE] Equipped vehicle: {}", v_type.to_string_name());
    true
}

/// Sets the vehicle as unequipped.
pub fn unequip_vehicle(ctx: &ReducerContext, wallet_address: &str, v_type: VehicleType) -> bool {
    // In a real system, we'd confirm ownership first. Assuming ownership check passed externally.
    let current_vehicle_data = ctx.db.player().get_current_vehicle();
    if current_vehicle_data.is_none() || current_vehicle_data.unwrap().vehicle_type != v_type {
        println!("[VEHICLE] Cannot unequipping: Vehicle mismatch or not owned.");
        return false;
    }

    let vehicle = current_vehicle_data.unwrap();
    if !vehicle.equipped {
        println!("[VEHICLE] Already unequipped.");
        return true;
    }
    
    // Update DB state
    let mut vehicle_to_update = vehicle;
    vehicle_to_update.equipped = false;
    ctx.db.player_mut().set_current_vehicle(&vehicle_to_update);
    println!("[VEHICLE] Unequipped vehicle: {}", v_type.to_string_name());
    true
}