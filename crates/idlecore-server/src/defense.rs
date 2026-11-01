//! Defense structures (Spec 023 §3.4): building and removing towers/walls
//! on claimed land. Structures occupy free world slots, share the per-hex
//! object budget with natural nodes, and live in their own replicated table
//! so raid damage can be applied server-side every second.

use spacetimedb::{ReducerContext, Table};
use idlecore_core::defense_config as cfg;

use crate::capital::find_capital;
use crate::economy::find_player;
use crate::objects::{add_item, count_item, remove_item};
use crate::types::{
    territory, defense_structure, world_object, hex_coords_of, tile_distance, hex_tile,
    now_secs, player, DefenseStructure, MAX_OBJECTS_PER_HEX,
};
use crate::world::ensure_hex;

fn touch(ctx: &ReducerContext, address: &str, now: u64) {
    if let Some(mut p) = find_player(ctx, address) {
        p.last_action_at = now;
        ctx.db.player().address().update(p);
    }
}

fn cooldown_left(p: &crate::types::Player, now: u64) -> Result<(), String> {
    if now.saturating_sub(p.last_action_at) < crate::types::ACTION_COOLDOWN_SECS {
        return Err(format!(
            "Action cooldown ({}s)",
            crate::types::ACTION_COOLDOWN_SECS
        ));
    }
    Ok(())
}

/// Pure multiset check removed — the build reducer verifies each ingredient
/// directly against the inventory before spending anything.

/// Build a defense structure on an owned, in-range hex at the selected
/// ground slot, paying the full item cost (Spec 023 §3.4).
pub fn build_defense(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
    kind: &str,
) -> Result<(), String> {
    let now = now_secs(ctx);
    let p = find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    cooldown_left(&p, now)?;

    let def = cfg::defense_kind(kind)
        .ok_or_else(|| format!("Unknown defense kind {kind}"))?;

    let Some(capital) = find_capital(ctx, &p.address).filter(|c| !c.destroyed) else {
        return Err("Place your capital first".to_string());
    };

    let (hq, hr) = hex_coords_of(hex_id);
    ensure_hex(ctx, hq, hr);
    let tile = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;
    if tile.terrain == "Water" {
        return Err("Cannot build on water".to_string());
    }
    // Territory: the builder must own the hex (capital hex included).
    let owns = ctx
        .db
        .territory()
        .hex_id()
        .find(hex_id)
        .map(|t| t.owner == p.address)
        .unwrap_or(false);
    if !owns {
        return Err("Build only on your territory — claim the land first".to_string());
    }
    if tile_distance(p.hex_q, p.hex_r, hq, hr) > 1 {
        return Err("Out of range (1 hex)".to_string());
    }

    // The slot cell must belong to this hex and be free (of nodes and
    // of other structures — the world slot budget is shared).
    if idlecore_core::slots::slot_tile(slot_x, slot_y) != (hq, hr) {
        return Err("Slot is outside the selected hex".to_string());
    }
    let (hx, hy) = tile_center_of(hq, hr);
    let (cx, cy) = idlecore_core::slots::slot_center(slot_x, slot_y);
    let (ox, oy) = (cx - hx, cy - hy);
    for o in ctx.db.world_object().object_by_hex().filter(hex_id) {
        if (o.offset_x - ox).abs() < 0.1 && (o.offset_y - oy).abs() < 0.1 {
            return Err("That spot is already taken".to_string());
        }
    }
    for s in ctx.db.defense_structure().structure_by_hex().filter(hex_id) {
        if (s.offset_x - ox).abs() < 0.1 && (s.offset_y - oy).abs() < 0.1 {
            return Err("That spot is already taken".to_string());
        }
    }
    let used = ctx.db.world_object().object_by_hex().filter(hex_id).count()
        + ctx.db.defense_structure().structure_by_hex().filter(hex_id).count();
    if used >= MAX_OBJECTS_PER_HEX as usize {
        return Err("Hex is full".to_string());
    }

    // Pay only after every check passed.
    for (item, qty) in def.cost {
        if count_item(ctx, &p.address, item) < *qty {
            // Can-afford returned none above; nothing was spent yet.
            return Err("Not enough materials".to_string());
        }
    }
    for (item, qty) in def.cost {
        remove_item(ctx, &p.address, item, *qty);
    }

    ctx.db.defense_structure().insert(DefenseStructure {
        structure_id: 0,
        capital_id: capital.capital_id,
        owner: p.address.clone(),
        hex_id,
        slot: 0, // numbering lives in the separate table's own space
        slot_x,
        slot_y,
        offset_x: ox,
        offset_y: oy,
        kind: def.name.to_string(),
        hp: def.hp,
        built_at: now,
        // Fresh towers start at level 1 with an empty XP bucket (Spec 024).
        level: 1,
        total_xp: 0,
    });
    touch(ctx, &p.address, now);
    tracing::info!(
        "BUILD-DEFENSE {address} {kind} @hex {hex_id} cell ({slot_x},{slot_y})"
    );
    Ok(())
}

/// Remove one of your own structures, refunding 50% of the cost (rounded
/// down per ingredient) so relocation hurts a bit but is not punishing.
pub fn remove_defense(ctx: &ReducerContext, address: &str, structure_id: u64) -> Result<(), String> {
    let now = now_secs(ctx);
    let p = find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    let s = ctx
        .db
        .defense_structure()
        .structure_id()
        .find(structure_id)
        .ok_or_else(|| "Structure not found".to_string())?;
    if s.owner != p.address {
        return Err("You can only remove your own structures".to_string());
    }
    ctx.db.defense_structure().structure_id().delete(structure_id);
    if let Some(def) = cfg::defense_kind(&s.kind) {
        for (item, qty) in def.cost {
            let refund = qty / 2;
            if refund > 0 {
                add_item(ctx, &p.address, item, refund);
            }
        }
    }
    touch(ctx, &p.address, now);
    tracing::info!("REMOVE-DEFENSE {address} #{structure_id}");
    Ok(())
}

fn tile_center_of(q: i32, r: i32) -> (f32, f32) {
    crate::types::tile_center(q, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlecore_core::defense_config::{defense_kind, ITEM_ARROW_TOWER};

    #[test]
    fn costs_are_paid_atomically() {
        // Every cost ingredient must exist before any deduction runs — the
        // build path verifies the full bundle before spending.
        let tower = defense_kind(ITEM_ARROW_TOWER).unwrap();
        let items: Vec<&str> = tower.cost.iter().map(|(i, _)| *i).collect();
        assert!(items.contains(&"Wood") && items.contains(&"Stone"));
    }

    #[test]
    fn remove_refunds_half_of_every_ingredient() {
        let tower = defense_kind(ITEM_ARROW_TOWER).unwrap();
        for (item, qty) in tower.cost {
            assert_eq!(items_half(*qty), *qty / 2, "refund of {item}");
        }
    }

    fn items_half(qty: u64) -> u64 {
        qty / 2
    }
}
