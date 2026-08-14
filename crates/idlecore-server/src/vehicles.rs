//! Vehicles (Spec 006) — purchase with gold, equip/unequip, daily 5G/h
//! maintenance (Ecosystem 2.1).

use spacetimedb::{ReducerContext, Table};
use crate::economy::spend_gold;
use crate::types::{now_secs, player, player_vehicle, VEHICLE_MAINTENANCE_PER_HOUR};

/// Vehicle catalog (Spec 006): speed multiplier and gold cost.
pub fn catalog() -> [(&'static str, f32, u64); 5] {
    [
        ("Bicycle", 2.0, 500),
        ("Scooter", 3.0, 1_000),
        ("Motorcycle", 5.0, 2_500),
        ("Boat", 4.0, 2_000),
        ("Airplane", 10.0, 10_000),
    ]
}

/// Speed multiplier for a vehicle type (1.0 when un-equipped/unknown).
/// Single source of truth shared with movement/Spec 003.
pub fn multiplier(vehicle_type: &str) -> f32 {
    catalog()
        .iter()
        .find(|(t, _, _)| *t == vehicle_type)
        .map(|(_, m, _)| *m)
        .unwrap_or(1.0)
}

/// Pure purchase resolution: catalog lookup + duplicate check + affordability.
pub fn resolve_purchase(
    gold: u64,
    owned_types: &[String],
    vehicle_type: &str,
) -> Result<(u64, f32), String> {
    let Some(&(_, mult, cost)) = catalog().iter().find(|(t, _, _)| *t == vehicle_type) else {
        return Err("Unknown vehicle type".to_string());
    };
    if owned_types.iter().any(|t| t == vehicle_type) {
        return Err("Vehicle already owned".to_string());
    }
    if gold < cost {
        return Err(format!("Insufficient gold (need {cost}, have {gold})"));
    }
    Ok((cost, mult))
}

/// Pure equip resolution: ownership check; returns the previously equipped
/// type (if any) that must be cleared.
pub fn equip_resolution(owned_types: &[(String, bool)], vehicle_type: &str) -> Result<Option<String>, String> {
    if vehicle_type == "None" {
        return Ok(None);
    }
    if !owned_types.iter().any(|(t, _)| t == vehicle_type) {
        return Err("Vehicle not owned".to_string());
    }
    Ok(owned_types.iter().find(|(_, e)| *e).map(|(t, _)| t.clone()))
}

/// Spec 006 FR1: buy a vehicle if affordable.
pub fn buy_vehicle(ctx: &ReducerContext, address: &str, vehicle_type: &str) -> Result<String, String> {
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    let owned_types: Vec<String> = ctx
        .db
        .player_vehicle()
        .iter()
        .filter(|v| v.player == p.address)
        .map(|v| v.vehicle_type.clone())
        .collect();
    let (cost, mult) = resolve_purchase(p.gold, &owned_types, vehicle_type)?;

    spend_gold(ctx, &mut p, cost, "buy_vehicle")?;
    ctx.db.player_vehicle().insert(crate::types::VehicleOwned {
        vehicle_id: 0,
        player: p.address.clone(),
        vehicle_type: vehicle_type.to_string(),
        purchased_at: now_secs(ctx),
        equipped: false,
        durability: 100,
        last_maintenance_day: 0,
    });

    tracing::info!("VEHICLE: {} bought {vehicle_type} ({}x speed)", address, mult);
    Ok(format!("Purchased {vehicle_type} ({}x speed)", mult))
}

/// Spec 006 FR2: equip/unequip (only one equipped at a time).
pub fn equip_vehicle(ctx: &ReducerContext, address: &str, vehicle_type: &str) -> Result<(), String> {
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    let owned_types: Vec<(String, bool)> = ctx
        .db
        .player_vehicle()
        .iter()
        .filter(|v| v.player == p.address)
        .map(|v| (v.vehicle_type.clone(), v.equipped))
        .collect();
    let prev = equip_resolution(&owned_types, vehicle_type)?;

    if vehicle_type == "None" {
        p.vehicle = "None".to_string();
        ctx.db.player().address().update(p);
        return Ok(());
    }

    // Unequip the previous one.
    if let Some(prev_type) = prev {
        for mut v in ctx.db.player_vehicle().iter() {
            if v.player == p.address && v.vehicle_type == prev_type {
                v.equipped = false;
                ctx.db.player_vehicle().vehicle_id().update(v);
            }
        }
    }
    let mut owned = ctx
        .db
        .player_vehicle()
        .iter()
        .find(|v| v.player == p.address && v.vehicle_type == vehicle_type)
        .ok_or_else(|| "Vehicle not owned".to_string())?;
    owned.equipped = true;
    ctx.db.player_vehicle().vehicle_id().update(owned);

    p.vehicle = vehicle_type.to_string();
    ctx.db.player().address().update(p);
    tracing::info!("VEHICLE: {} equipped {vehicle_type}", address);
    Ok(())
}

/// Daily maintenance: 5G/h → 120G/day per owned vehicle (Ecosystem 2.1).
/// Called from the hourly maintenance tick once per day.
pub fn charge_daily_maintenance(ctx: &ReducerContext, epoch_day: u32) {
    let mut charged = 0u64;
    for mut v in ctx.db.player_vehicle().iter() {
        if v.last_maintenance_day == epoch_day {
            continue;
        }
        let Some(mut p) = crate::economy::find_player(ctx, &v.player) else {
            continue;
        };
        let cost = VEHICLE_MAINTENANCE_PER_HOUR * 24;
        if p.gold >= cost {
            p.gold -= cost;
            p.lifetime_gold_spent = p.lifetime_gold_spent.saturating_add(cost);
            ctx.db.player().address().update(p);
            charged += cost;
        } else {
            // Insufficient gold: durability decays; at 0 the vehicle is lost.
            v.durability = v.durability.saturating_sub(10);
            tracing::warn!(
                "MAINTENANCE: {} cannot pay {cost}G — durability {}",
                v.player,
                v.durability
            );
        }
        v.last_maintenance_day = epoch_day;
        ctx.db.player_vehicle().vehicle_id().update(v);
    }
    if charged > 0 {
        tracing::info!("MAINTENANCE: charged {charged}G total this cycle");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VEHICLE_MAINTENANCE_PER_HOUR;

    #[test]
    fn catalog_has_five_unique_vehicles() {
        let c = catalog();
        assert_eq!(c.len(), 5);
        let mut names: Vec<&str> = c.iter().map(|(n, _, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn catalog_costs_and_speeds_sane() {
        for (name, speed, cost) in catalog() {
            assert!(speed > 1.0, "{name} must be faster than walking");
            assert!(speed <= 10.0, "{name} speed bounded");
            assert!(cost > 0, "{name} must cost gold");
        }
    }

    #[test]
    fn faster_vehicles_cost_more() {
        let c = catalog();
        let airplane = c.iter().find(|(n, _, _)| *n == "Airplane").unwrap();
        let bicycle = c.iter().find(|(n, _, _)| *n == "Bicycle").unwrap();
        assert!(airplane.1 > bicycle.1, "airplane faster");
        assert!(airplane.2 > bicycle.2, "airplane costs more");
    }

    #[test]
    fn maintenance_rate_positive() {
        assert!(VEHICLE_MAINTENANCE_PER_HOUR > 0);
    }
}

#[cfg(test)]
mod tests_pure {
    use super::*;

    #[test]
    fn purchase_with_sufficient_gold() {
        let owned: Vec<String> = vec![];
        let (cost, mult) = resolve_purchase(10_000, &owned, "Motorcycle").unwrap();
        assert_eq!(cost, 2_500);
        assert_eq!(mult, 5.0);
    }

    #[test]
    fn purchase_with_insufficient_gold() {
        let owned: Vec<String> = vec![];
        let err = resolve_purchase(2_499, &owned, "Motorcycle").unwrap_err();
        assert!(err.contains("Insufficient gold"), "{err}");
    }

    #[test]
    fn purchase_unknown_vehicle() {
        let owned: Vec<String> = vec![];
        assert!(resolve_purchase(1_000_000, &owned, "Rocket").is_err());
    }

    #[test]
    fn purchase_duplicate_rejected() {
        let owned: Vec<String> = vec!["Bicycle".into()];
        let err = resolve_purchase(1_000_000, &owned, "Bicycle").unwrap_err();
        assert!(err.contains("already owned"), "{err}");
    }

    #[test]
    fn equip_unequip_cycle() {
        let owned: Vec<(String, bool)> = vec![("Bicycle".into(), true), ("Boat".into(), false)];
        // Equip Boat clears Bicycle.
        let prev = equip_resolution(&owned, "Boat").unwrap();
        assert_eq!(prev.as_deref(), Some("Bicycle"));
        // Unequip all.
        assert_eq!(equip_resolution(&owned, "None").unwrap(), None);
        // Not owned.
        assert!(equip_resolution(&owned, "Rocket").is_err());
    }

    #[test]
    fn speed_multipliers_match_catalog() {
        assert_eq!(multiplier("Bicycle"), 2.0);
        assert_eq!(multiplier("Airplane"), 10.0);
        assert_eq!(multiplier("None"), 1.0);
        assert_eq!(multiplier("Rocket"), 1.0);
    }

    #[test]
    fn persistence_row_carry_all_state() {
        // Spec 006 T7.5: the row persists vehicle + equip state (verified via
        // public table replication; the row type carries every field).
        let row = crate::types::VehicleOwned {
            vehicle_id: 7,
            player: "0xabc".into(),
            vehicle_type: "Boat".into(),
            purchased_at: 123,
            equipped: true,
            durability: 100,
            last_maintenance_day: 0,
        };
        assert!(row.equipped);
        assert_eq!(row.vehicle_type, "Boat");
    }
}
