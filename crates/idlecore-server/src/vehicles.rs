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

/// Spec 006 FR1: buy a vehicle if affordable.
pub fn buy_vehicle(ctx: &ReducerContext, address: &str, vehicle_type: &str) -> Result<String, String> {
    let catalog = catalog();
    let Some(&(_, _, cost)) = catalog.iter().find(|(t, _, _)| *t == vehicle_type) else {
        return Err("Unknown vehicle type".to_string());
    };

    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    // Already owned?
    let owned = ctx
        .db
        .player_vehicle()
        .iter()
        .any(|v| v.player == p.address && v.vehicle_type == vehicle_type);
    if owned {
        return Err("Vehicle already owned".to_string());
    }

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

    let (_, mult, _) = catalog.iter().find(|(t, _, _)| *t == vehicle_type).unwrap();
    tracing::info!("VEHICLE: {} bought {vehicle_type} ({}x speed)", address, mult);
    Ok(format!("Purchased {vehicle_type} ({}x speed)", mult))
}

/// Spec 006 FR2: equip/unequip (only one equipped at a time).
pub fn equip_vehicle(ctx: &ReducerContext, address: &str, vehicle_type: &str) -> Result<(), String> {
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    if vehicle_type == "None" {
        p.vehicle = "None".to_string();
        ctx.db.player().address().update(p);
        return Ok(());
    }

    let owned = ctx
        .db
        .player_vehicle()
        .iter()
        .find(|v| v.player == p.address && v.vehicle_type == vehicle_type)
        .ok_or_else(|| "Vehicle not owned".to_string())?;

    // Unequip the previous one.
    for mut v in ctx.db.player_vehicle().iter() {
        if v.player == p.address && v.equipped {
            v.equipped = false;
            ctx.db.player_vehicle().vehicle_id().update(v);
        }
    }
    let mut owned = owned;
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