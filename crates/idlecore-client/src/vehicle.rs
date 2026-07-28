use crate::vehicle::{VehicleType, purchase_vehicle, equip_vehicle, unequip_vehicle};

// This is a client-side stub for the mobile/gameplay representation.
// In a real implementation, this would handle networking calls/RPCs to the server.

#[derive(Debug)]
pub struct VehicleInfo {
    pub vehicle_type: VehicleType,
    pub is_equipped: bool,
    pub is_purchased: bool,
}

/// Stub: Called when the player attempts to purchase a vehicle.
pub fn client_purchase_vehicle(v_type: VehicleType, player_data_in_game: bool) -> Result<(), &'static str> {
    if !player_data_in_game {
        return Err("Not connected to server context.");
    }
    
    // Simulate calling the server logic with a mock context/address
    // Success depends on successful server-side deduction/purchase.
    let success = purchase_vehicle(
        &MockReducerContext::new(), // Mock context needed for compile check
        "PLAYER_WALLET_ADDRESS", 
        v_type
    );
    
    if success {
        println!("[CLIENT] Purchase request sent for {:?}. Success.", v_type);
        Ok(())
    } else {
        Err("Purchase failed: Check funds or if already owned.")
    }
}

/// Stub: Called when the player attempts to equip a vehicle.
pub fn client_equip_vehicle(v_type: VehicleType) -> Result<(), &'static str> {
    println!("[CLIENT] Sending equip request for {:?}...", v_type);
    // Server call here...
    Ok(())
}

/// Stub: Called when the player attempts to unequip a vehicle.
pub fn client_unequip_vehicle(v_type: VehicleType) -> Result<(), &'static str> {
    println!("[CLIENT] Sending unequip request for {:?}...", v_type);
    // Server call here...
    Ok(())
}

// Mock structs/impls needed to satisfy compilation constraints during implementation phase.
pub struct MockReducerContext;
impl MockReducerContext {
    pub fn new() -> Self { MockReducerContext }
    pub fn db(&self) -> MockPlayerDb { MockPlayerDb {} }
}

pub struct MockPlayerDb;
impl MockPlayerDb {
    pub fn get_current_vehicle(&self) -> Option<Vehicle> { 
        // Return a default owned vehicle to allow successful compile path for equip/unequip testing
        Some(Vehicle { vehicle_type: VehicleType::Bicycle, equipped: false, purchased: true }) 
    }
    pub fn player_mut(&self) -> MockPlayerDb {}
}

impl MockPlayerDb {
    pub fn deduct_gold(&self, cost: u64) -> Result<(), ()> {
        println!("MOCK: Deducting {} gold.", cost);
        Ok(())
    }
    pub fn set_current_vehicle(&self, vehicle: &Vehicle) {
        println!("MOCK: Setting current vehicle to {:?}.", vehicle.vehicle_type);
    }
}