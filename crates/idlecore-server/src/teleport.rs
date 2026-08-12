//! Teleport — cost scaling `100 × √level`, 60 s cooldown, instant warp to a
//! valid in-map hex (Spec 008 FR2-FR5, Ecosystem 2.3).

use spacetimedb::ReducerContext;
use crate::economy::spend_gold;
use crate::types::{player, hex_tile, 
    hex_center, hex_id_of, now_secs, TELEPORT_BASE_COST, TELEPORT_COOLDOWN_SECS,
};

/// Teleport cost per Ecosystem spec 2.3: `floor(100 × √level)`.
pub fn teleport_cost(level: u32) -> u64 {
    let l = level.max(1) as f64;
    (TELEPORT_BASE_COST as f64 * l.sqrt()).floor() as u64
}

/// Execute the teleport. Returns Ok((x, y)) with the new world position.
pub fn teleport(
    ctx: &ReducerContext,
    address: &str,
    target_q: i32,
    target_r: i32,
) -> Result<(f32, f32), String> {
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    // Spec 008 FR5: cooldown display + enforcement.
    if now.saturating_sub(p.last_action_at) < TELEPORT_COOLDOWN_SECS {
        return Err(format!(
            "Teleport on cooldown ({}s left)",
            TELEPORT_COOLDOWN_SECS - now.saturating_sub(p.last_action_at)
        ));
    }

    // Spec 008 FR3: the destination hex must exist in the generated world.
    let hex = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id_of(target_q, target_r))
        .ok_or_else(|| "Destination hex does not exist".to_string())?;
    let _ = hex; // exists check only

    let cost = teleport_cost(p.level);
    if let Err(e) = spend_gold(ctx, &mut p, cost, "teleport") {
        return Err(e);
    }

    let (x, y) = hex_center(target_q, target_r);
    p.last_action_at = now;
    p.hex_q = target_q;
    p.hex_r = target_r;
    p.hex_id = crate::types::hex_id_of(target_q, target_r);
    p.position_x = x;
    p.position_y = y;
    ctx.db.player().address().update(p);

    tracing::info!("TELEPORT player={} to ({},{}) cost={}G", address, target_q, target_r, cost);
    Ok((x, y))
}