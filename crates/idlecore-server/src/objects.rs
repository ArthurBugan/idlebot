//! Server-authoritative world objects — grass tufts, rocks and player-grown
//! trees (Spec: resource nodes).
//!
//! Grass tufts spawn deterministically per hex+slot when a tile materializes.
//! Destroying a tuft drops seeds; planting a seed grows a tree; harvesting a
//! mature tree yields wood and more seeds. The client renders exactly what
//! the `world_object` table replicates — nothing is client-invented.

use spacetimedb::{ReducerContext, Table};
use crate::economy::{add_xp, find_player};
use crate::types::{
    object_removed, player, player_item, world_object, GATHER_XP, GRASS_PER_TUFT, HARVEST_TREE_XP, ITEM_GRASS,
    ITEM_SEED, ITEM_STONE, ITEM_WOOD, MAX_OBJECTS_PER_HEX, OBJ_GRASS, OBJ_ROCK, OBJ_TREE,
    STONE_PER_ROCK, TREE_GROWTH_SECS, WOOD_PER_TREE,
};
use crate::world::ensure_hex;
use idlecore_core::terrain::TerrainType;

/// Deterministic hash for an object slot — same inputs, same world, always.
fn obj_hash(hex_id: u64, slot: u8) -> u64 {
    let mut x = hex_id.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(slot).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ 0x6A09_E667_F3BC_C909;
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Axial neighbor offsets (pointy-top hex grid).
const AXIAL_NEIGHBORS: [(i32, i32); 6] =
    [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// Spacing radius (in axial hex distance) between natural resource nodes:
/// a hex only spawns when its priority hash beats every hex within this
/// radius, so two nodes are always at least SPACING_RADIUS + 1 hexes apart.
const SPACING_RADIUS: i32 = 3;

/// Scarcity spacing rule: a hex may host natural nodes only when its
/// priority hash beats every hex within `SPACING_RADIUS`. This keeps nodes
/// far apart, is a pure function of geometry (so the result never depends
/// on tile generation order or timing), and needs no table reads.
/// Water/city hexes compete too — they just never spawn — which only
/// thins coastlines further.
fn hex_wins_spacing(hex_id: u64) -> bool {
    let coord = idlecore_core::hex::HexCoord::from_id(hex_id);
    let my = obj_hash(hex_id, 0);
    for dq in -SPACING_RADIUS..=SPACING_RADIUS {
        for dr in -SPACING_RADIUS..=SPACING_RADIUS {
            if dq == 0 && dr == 0 {
                continue;
            }
            // Axial distance from the center hex to this offset.
            let dist = dq.abs().max(dr.abs()).max((dq + dr).abs());
            if dist > SPACING_RADIUS {
                continue;
            }
            let n = idlecore_core::hex::HexCoord::new(coord.q + dq, coord.r + dr);
            if obj_hash(n.to_id(), 0) >= my {
                return false;
            }
        }
    }
    true
}

/// Whether a natural node spawns in `slot` for this terrain, and its kind.
fn natural_spawn(terrain: TerrainType, slot: u8, h: u64) -> Option<&'static str> {
    let roll = h % 100;
    let kind = match terrain {
        // Spawn rates are intentionally very scarce: resources are meant to
        // be hunted, not farmed in place, and every replicated node costs the
        // client a sprite and draw-call budget.
        // Spawn rates are intentionally scarce, and the spacing rule already
        // caps density at ~1 eligible hex per 7 — rolls below are for the
        // hexes that win that competition.
        TerrainType::Grass => {
            if slot == 5 {
                (roll < 2).then_some(OBJ_ROCK) // scattered boulders
            } else if slot <= 2 {
                (roll < 22).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Grassland => {
            if slot == 5 {
                (roll < 2).then_some(OBJ_ROCK)
            } else if slot <= 1 {
                (roll < 24).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Forest
        | TerrainType::Taiga
        | TerrainType::TropicalRainforest => {
            if slot == 5 {
                (roll < 2).then_some(OBJ_ROCK)
            } else if slot <= 2 {
                (roll < 20).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Desert | TerrainType::Tundra => {
            if slot == 5 {
                (roll < 6).then_some(OBJ_ROCK)
            } else if slot == 0 {
                (roll < 6).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Mountain => {
            if slot == 4 {
                (roll < 45).then_some(OBJ_ROCK)
            } else if slot == 0 {
                (roll < 5).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::City | TerrainType::Water | TerrainType::Polluted => None,
    };
    kind
}

/// Roll any missing natural nodes for one hex. Idempotent per slot: existing
/// rows are never touched, so backfilling old tiles changes nothing visible.
/// Hexes that lose the spacing rule against a neighbor never spawn here —
/// resources keep a minimum distance from each other. Gathered slots stay
/// empty until their `respawn_at` (10-60 min) elapses, then roll again.
pub fn ensure_objects(ctx: &ReducerContext, hex_id: u64, terrain: TerrainType) -> usize {
    if !hex_wins_spacing(hex_id) {
        return 0;
    }
    let now = crate::types::now_secs(ctx);
    // Free slots whose respawn timer has elapsed: delete the stale record so
    // the deterministic roller can bring the node back.
    let expired: Vec<String> = ctx
        .db
        .object_removed()
        .removed_by_hex()
        .filter(hex_id)
        .filter(|r| now >= r.respawn_at)
        .map(|r| r.key.clone())
        .collect();
    for key in expired {
        if let Some(row) = ctx.db.object_removed().key().find(key.clone()) {
            let _ = row;
            ctx.db.object_removed().key().delete(key);
        }
    }
    let occupied: std::collections::HashSet<u8> = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .map(|o| o.slot)
        .chain(ctx.db.object_removed().removed_by_hex().filter(hex_id).map(|r| r.slot))
        .collect();
    let mut created = 0usize;
    for slot in 0..MAX_OBJECTS_PER_HEX {
        if occupied.contains(&slot) {
            continue;
        }
        let h = obj_hash(hex_id, slot);
        let Some(kind) = natural_spawn(terrain, slot, h) else { continue };
        let offset_x = ((h >> 8) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
        let offset_y = ((h >> 16) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
        ctx.db.world_object().insert(crate::types::WorldObject {
            object_id: 0,
            hex_id,
            slot,
            kind: kind.to_string(),
            offset_x,
            offset_y,
            mature_at: 0,
            planted_by: None,
        });
        created += 1;
    }
    created
}

/// Add `count` of an item to a player's inventory.
pub fn add_item(ctx: &ReducerContext, address: &str, item: &str, count: u64) {
    let key = format!("{address}|{item}");
    match ctx.db.player_item().key().find(key.clone()) {
        Some(mut row) => {
            row.count += count;
            ctx.db.player_item().key().update(row);
        }
        None => {
            ctx.db.player_item().insert(crate::types::PlayerItem {
                key,
                player: address.to_string(),
                item: item.to_string(),
                count,
            });
        }
    }
}

/// Remove `count` of an item; returns false when the player has fewer.
pub fn remove_item(ctx: &ReducerContext, address: &str, item: &str, count: u64) -> bool {
    let key = format!("{address}|{item}");
    let Some(mut row) = ctx.db.player_item().key().find(key) else { return false };
    if row.count < count {
        return false;
    }
    row.count -= count;
    if row.count == 0 {
        ctx.db.player_item().key().delete(row.key);
    } else {
        ctx.db.player_item().key().update(row);
    }
    true
}

/// Count of an item in a player's inventory (0 when absent).
#[allow(dead_code)]
pub fn count_item(ctx: &ReducerContext, address: &str, item: &str) -> u64 {
    ctx.db
        .player_item()
        .key()
        .find(format!("{address}|{item}"))
        .map(|r| r.count)
        .unwrap_or(0)
}

/// Record a consumed natural slot so the deterministic roller never
/// resurrects it (and so planted trees keep their slot reserved).
/// Deterministic respawn delay for one consumed slot: 10-60 minutes.
fn respawn_delay(hex_id: u64, slot: u8) -> u64 {
    crate::types::RESPAWN_MIN_SECS
        + obj_hash(hex_id, slot) % (crate::types::RESPAWN_MAX_SECS - crate::types::RESPAWN_MIN_SECS)
}

/// Consume a natural node: record a timed respawn (10-60 min, deterministic
/// per hex+slot) so the node stays gone until then.
pub fn consume_slot(ctx: &ReducerContext, hex_id: u64, slot: u8) {
    // NB: plain braces — the doubled `{{...}}` form used to emit the literal
    // "{hex_id}:{slot}" for every row, colliding on the PK so all but the
    // first tombstone were silently dropped and nodes respawned instantly.
    let key = format!("{}:{}", hex_id, slot);
    let respawn_at = crate::types::now_secs(ctx) + respawn_delay(hex_id, slot);
    match ctx.db.object_removed().key().find(key.clone()) {
        Some(mut row) => {
            row.respawn_at = respawn_at;
            ctx.db.object_removed().key().update(row);
        }
        None => {
            ctx.db.object_removed().insert(crate::types::ObjectRemoved {
                key,
                hex_id,
                slot,
                respawn_at,
            });
        }
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

fn touch(ctx: &ReducerContext, address: &str, now: u64) {
    if let Some(mut p) = find_player(ctx, address) {
        p.last_action_at = now;
        ctx.db.player().address().update(p);
    }
}

/// Destroy a grass tuft (drops seeds) or harvest a mature tree (wood + seeds).
pub fn gather_object(
    ctx: &ReducerContext,
    address: &str,
    object_id: u64,
) -> Result<(), String> {
    use crate::types::{hex_distance, now_secs};
    let now = now_secs(ctx);
    let mut p = find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    cooldown_left(&p, now)?;

    let obj = ctx
        .db
        .world_object()
        .object_id()
        .find(object_id)
        .ok_or_else(|| "Object not found".to_string())?;
    let (hq, hr) = crate::types::hex_coords_of(obj.hex_id);
    let dist = hex_distance(p.hex_q, p.hex_r, hq, hr);
    if dist > 1 {
        return Err("Out of range (1 hex)".to_string());
    }

    match obj.kind.as_str() {
        OBJ_GRASS => {
            ctx.db.world_object().object_id().delete(object_id);
            consume_slot(ctx, obj.hex_id, obj.slot);
            add_item(ctx, &p.address, ITEM_GRASS, GRASS_PER_TUFT);
            // "Maybe some seeds": deterministic per-object chance.
            if object_id % 100 < 55 {
                add_item(ctx, &p.address, ITEM_SEED, 1);
            }
            add_xp(ctx, &mut p, GATHER_XP, "gather");
            tracing::info!("GATHER {address}: grass @hex {}", obj.hex_id);
        }
        OBJ_ROCK => {
            ctx.db.world_object().object_id().delete(object_id);
            consume_slot(ctx, obj.hex_id, obj.slot);
            add_item(ctx, &p.address, ITEM_STONE, STONE_PER_ROCK);
            add_xp(ctx, &mut p, GATHER_XP + 1, "mine");
            tracing::info!("MINE {address}: rock @hex {}", obj.hex_id);
        }
        OBJ_TREE => {
            if now < obj.mature_at {
                return Err(format!(
                    "Tree still growing ({}s left)",
                    obj.mature_at - now
                ));
            }
            ctx.db.world_object().object_id().delete(object_id);
            consume_slot(ctx, obj.hex_id, obj.slot);
            add_item(ctx, &p.address, ITEM_WOOD, WOOD_PER_TREE);
            let seeds = 1 + object_id % 2;
            add_item(ctx, &p.address, ITEM_SEED, seeds);
            add_xp(ctx, &mut p, HARVEST_TREE_XP, "harvest_tree");
            tracing::info!("HARVEST-TREE {address} @hex {} (+{seeds} seeds)", obj.hex_id);
        }
        other => return Err(format!("{other} cannot be gathered")),
    }
    touch(ctx, &p.address, now);
    Ok(())
}

/// Plant a tree on an adjacent land hex, consuming one seed. The tree is
/// placed at the center of the requested ground slot (Stardew-style square
/// cell, see `idlecore_core::slots`); the slot must lie inside `hex_id`.
pub fn plant_tree(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
) -> Result<(), String> {
    use crate::types::{hex_coords_of, hex_distance, hex_tile, now_secs};
    let now = now_secs(ctx);
    let mut p = find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    cooldown_left(&p, now)?;

    let (hq, hr) = hex_coords_of(hex_id);
    ensure_hex(ctx, hq, hr);
    let tile = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;
    if hex_distance(p.hex_q, p.hex_r, hq, hr) > 1 {
        return Err("Out of range (1 hex)".to_string());
    }
    // Trees grow everywhere natural except shores, cities and mountains.
    let plantable = matches!(
        tile.terrain.as_str(),
        "Grass" | "Forest" | "Grassland" | "Taiga" | "TropicalRainforest" | "Tundra" | "Desert"
    );
    if !plantable {
        return Err(format!("Cannot grow trees on {} here", tile.terrain));
    }
    // The requested slot must be owned by the targeted hex — no planting
    // across hex borders by aiming at a shared edge slot.
    if idlecore_core::slots::slot_hex(slot_x, slot_y) != (hq, hr) {
        return Err("Slot is outside the selected hex".to_string());
    }

    let used: std::collections::HashSet<u8> = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .map(|o| o.slot)
        .collect();
    let slot = (0..MAX_OBJECTS_PER_HEX).find(|s| !used.contains(s));
    let Some(slot) = slot else { return Err("Hex is full".to_string()) };

    remove_item(ctx, &p.address, ITEM_SEED, 1)
        .then_some(())
        .ok_or_else(|| "No seeds — destroy tall grass first".to_string())?;

    // Snap the tree to the slot's center (offset relative to the hex center).
    let (hx, hy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
        hq,
        hr,
        idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
    );
    let (cx, cy) = idlecore_core::slots::slot_center(slot_x, slot_y);
    let offset_x = cx - hx;
    let offset_y = cy - hy;
    ctx.db.world_object().insert(WorldObjectRow {
        object_id: 0,
        hex_id,
        slot,
        kind: OBJ_TREE.to_string(),
        offset_x,
        offset_y,
        mature_at: now + TREE_GROWTH_SECS,
        planted_by: Some(p.address.clone()),
    });
    add_xp(ctx, &mut p, 1, "plant_tree");
    touch(ctx, &p.address, now);
    tracing::info!("PLANT-TREE {address} @hex {hex_id} slot {slot} cell ({slot_x},{slot_y})");
    Ok(())
}

use crate::types::WorldObject as WorldObjectRow;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_spawns_never_include_trees_and_respect_biomes() {
        for slot in 0..MAX_OBJECTS_PER_HEX {
            for seed in 0..64u64 {
                let h = obj_hash(seed, slot);
                for (terrain, allowed) in [
                    (TerrainType::Water, false),
                    (TerrainType::City, false),
                    (TerrainType::Polluted, false),
                    (TerrainType::Mountain, true),
                    (TerrainType::Grass, true),
                    (TerrainType::Desert, true),
                    (TerrainType::Tundra, true),
                    (TerrainType::Forest, true),
                ] {
                    if let Some(kind) = natural_spawn(terrain, slot, h) {
                        assert_ne!(kind, OBJ_TREE, "{terrain:?} must never auto-spawn trees");
                        assert!(allowed, "unexpected spawn on {terrain:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn offsets_stay_inside_the_hex() {
        for hex_id in [0u64, 12345, u64::from(u32::MAX)] {
            for slot in 0..MAX_OBJECTS_PER_HEX {
                let h = obj_hash(hex_id, slot);
                let ox = ((h >> 8) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
                let oy = ((h >> 16) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
                assert!((-4.5..=4.5).contains(&ox));
                assert!((-4.5..=4.5).contains(&oy));
            }
        }
    }

    #[test]
    fn respawn_delay_is_between_10_and_60_minutes() {
        for hex_id in [0u64, 7, 123_456_789, u64::MAX] {
            for slot in 0..MAX_OBJECTS_PER_HEX {
                let d = respawn_delay(hex_id, slot);
                assert!(
                    (crate::types::RESPAWN_MIN_SECS..crate::types::RESPAWN_MAX_SECS).contains(&d),
                    "delay {d} out of range for ({hex_id},{slot})"
                );
            }
        }
        // Deterministic: same inputs, same delay.
        assert_eq!(respawn_delay(42, 3), respawn_delay(42, 3));
    }

    #[test]
    fn natural_nodes_keep_distance_from_each_other() {
        use idlecore_core::hex::HexCoord;

        // Over a patch of hexes, no two hexes within SPACING_RADIUS of each
        // other may both win the spacing gate — every node keeps a wide,
        // guaranteed gap to the next resource.
        let wins: std::collections::HashSet<u64> = (-40..=40)
            .flat_map(|q| (-40..=40).map(move |r| (q, r)))
            .map(|(q, r)| HexCoord::new(q, r).to_id())
            .filter(|&id| hex_wins_spacing(id))
            .collect();

        let mut winners = 0usize;
        for q in -40..=40 {
            for r in -40..=40 {
                let id = HexCoord::new(q, r).to_id();
                if !wins.contains(&id) {
                    continue;
                }
                winners += 1;
                for dq in -SPACING_RADIUS..=SPACING_RADIUS {
                    for dr in -SPACING_RADIUS..=SPACING_RADIUS {
                        let dist = dq.abs().max(dr.abs()).max((dq + dr).abs());
                        if dist == 0 || dist > SPACING_RADIUS {
                            continue;
                        }
                        let neighbor = HexCoord::new(q + dq, r + dr).to_id();
                        assert!(
                            !wins.contains(&neighbor),
                            "hexes ({q},{r}) and ({},{}) are {dist} apart but both host nodes",
                            q + dq,
                            r + dr
                        );
                    }
                }
            }
        }
        // Sanity: the gate must actually pass sometimes (radius-3 disc has
        // 37 hexes, so ~1/37 of the patch should win).
        assert!(winners > 20, "spacing gate rejected everything ({winners} winners)");
    }

    #[test]
    fn spacing_gate_is_pure_and_stable() {
        use idlecore_core::hex::HexCoord;

        let ids: Vec<u64> = [(-13, 7), (0, 0), (512, -1024), (i32::MAX, i32::MIN)]
            .iter()
            .map(|&(q, r)| HexCoord::new(q, r).to_id())
            .collect();
        for id in ids {
            assert_eq!(hex_wins_spacing(id), hex_wins_spacing(id));
        }
    }
}
