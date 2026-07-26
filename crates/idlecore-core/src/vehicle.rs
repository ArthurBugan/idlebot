//! Vehicle system — purchase, equip, and use vehicles.
//!
//! Vehicle types: None, Bicycle (2x), Scooter (3x), Motorcycle (5x), Boat (4x), Airplane (10x).
//! Speed multipliers and gold costs per PROPOSAL section 2.6.

use crate::Vehicle;
use std::time::SystemTime;

/// Purchase a vehicle. Returns success status and deducted cost.
pub fn purchase_vehicle(econ: &mut crate::economy::PlayerEconomy, vehicle: &Vehicle) -> bool {
    if !econ.vehicle.is_empty() {
        // Already has a vehicle
        return false;
    }

    let cost = vehicle.purchase_cost();
    let had_enough = crate::economy::spend_gold(econ, cost);

    if had_enough {
        econ.vehicle = vehicle.display_name().to_string();
        println!(
            "[VEHICLE] Purchased {} for {}G (speed: {}x)",
            vehicle.display_name(),
            cost,
            vehicle.speed_multiplier()
        );
        true
    } else {
        println!(
            "[VEHICLE] Cannot purchase {}: need {}G, have {}G",
            vehicle.display_name(),
            cost,
            econ.gold
        );
        false
    }
}

/// Apply idle gains when player logs in after being offline
pub fn apply_idle_gains(econ: &mut crate::economy::PlayerEconomy) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let seconds_offline = now.saturating_sub(econ.last_login_time);
    let elapsed = std::time::Duration::from_secs(seconds_offline);

    if seconds_offline < 60 {
        return; // Less than 1 minute, no gains
    }

    let gains = crate::idle::gains_for_time(elapsed);

    econ.xp += gains.xp;
    econ.gold += gains.gold;

    // Recalculate level
    econ.level = crate::progression::calculate_level(econ.xp);
    econ.next_level_xp_needed = crate::economy::xp_for_next_level(econ.level);

    println!(
        "[LOGIN] Idle gains: {} XP, {} Gold (level now {})",
        gains.xp, gains.gold, econ.level
    );
}

/// Apply vehicle maintenance (5G/hour per PROPOSAL section 3.5)
pub fn apply_vehicle_maintenance(econ: &mut crate::economy::PlayerEconomy) -> Option<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let elapsed = now.saturating_sub(econ.last_daily_gold_check);
    let hours_since_check = if elapsed > 0 {
        elapsed / 86400u64
    } else {
        0
    };

    if econ.vehicle.is_empty() {
        return None;
    }

    // Vehicle maintenance: 5G per hour
    let hours_online = if econ.last_logout_time > 0 {
        now.saturating_sub(econ.last_logout_time) / 3600
    } else {
        0
    };

    let total_hours = hours_since_check + hours_online;
    let cost = total_hours.saturating_mul(5); // 5G per hour

    if cost > 0 {
        econ.gold = econ.gold.saturating_sub(cost);
        econ.last_daily_gold_check = now;
        println!(
            "[VEHICLE] Maintenance: {}G deducted for {}h ({})",
            cost,
            total_hours,
            econ.vehicle
        );
        Some(cost)
    } else {
        None
    }
}

/// Get all available vehicles as a list
pub fn available_vehicles() -> Vec<Vehicle> {
    Vehicle::all_vehicles().to_vec()
}
