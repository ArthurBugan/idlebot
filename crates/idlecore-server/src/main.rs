use crate::vehicle::{purchase_vehicle, equip_vehicle, unequip_vehicle, VehicleType};
use spacetimedb::{ReducerContext};

// --- Reducers ---

/// #[reducer] Pub fn buy_vehicle(ctx: &ReducerContext, wallet_address: String, vehicle_type: VehicleType, cost: u64)
#[reducer]
pub fn buy_vehicle(ctx: &ReducerContext, wallet_address: String, v_type: VehicleType, cost: u64) {
    let success = purchase_vehicle(ctx, &wallet_address, v_type);
    if success {
        // Server successfully processed payment and state change.
        println!("[SERVER] Successful purchase of {:?} for {}", v_type, cost);
    } else {
        println!("[SERVER] Purchase failed for {:?}. Check logs for reason.", v_type);
    }
}

/// #[reducer] Pub fn equip_vehicle(ctx: &ReducerContext, wallet_address: String, vehicle_type: VehicleType)
#[reducer]
pub fn equip_vehicle(ctx: &ReducerContext, wallet_address: String, v_type: VehicleType) {
    let success = equip_vehicle(ctx, &wallet_address, v_type);
    if success {
        println!("[SERVER] Successfully equipped {:?}.", v_type);
    } else {
        println!("[SERVER] Equip attempt failed for {:?}.", v_type);
    }
}

/// #[reducer] Pub fn unequip_vehicle(ctx: &ReducerContext, wallet_address: String, vehicle_type: VehicleType)
#[reducer]
pub fn unequip_vehicle(ctx: &ReducerContext, wallet_address: String, v_type: VehicleType) {
    let success = unequip_vehicle(ctx, &wallet_address, v_type);
    if success {
        println!("[SERVER] Successfully unequiped {:?}.", v_type);
    } else {
        println!("[SERVER] Unequip attempt failed for {:?}.", v_type);
    }
}