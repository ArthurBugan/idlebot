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
    match_recipe, object_removed, player, player_item, world_object, BENCH_LOG_COST,
    CRAFT_XP, GATHER_XP, GRASS_GROWTH_SECS, GRASS_PER_TUFT, HARVEST_TREE_XP, ITEM_GRASS, ITEM_LOG,
    ITEM_SEED, ITEM_STONE, ITEM_WOOD, MAX_OBJECTS_PER_HEX, OBJ_CRAFT_BENCH, OBJ_GRASS, OBJ_LOG,
    OBJ_ROCK, OBJ_TREE, STONE_PER_ROCK, TREE_GROWTH_SECS, WOOD_PER_TREE,
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
                // Forest floors: grass tufts plus fallen logs (Spec 022) —
                // logs are the bootstrap resource for the first craft bench.
                if roll < 20 {
                    Some(OBJ_GRASS)
                } else if roll < 28 {
                    Some(OBJ_LOG)
                } else {
                    None
                }
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
/// Nodes sit at the CENTER of a ground slot owned by the hex (Stardew-style):
/// the client selector targets exactly the tile the sprite is on.
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

    // Ground-slot cells owned by this hex, plus the cells already visually
    // occupied by existing rows (planted trees snap to slot centers too).
    use idlecore_core::hex_grid::HexGrid;
    use idlecore_core::slots::{slot_center, slot_hex, world_pos_to_slot, SLOT_SIZE};
    use idlecore_core::world_gen::WorldGenConfig;
    let (hq, hr) = crate::types::hex_coords_of(hex_id);
    let (hx, hy) = HexGrid::axial_to_world(hq, hr, WorldGenConfig::HEX_SIZE);
    let span = 2.0 * WorldGenConfig::HEX_SIZE;
    let (sx0, sy0) = world_pos_to_slot(hx - span, hy - span);
    let (sx1, sy1) = world_pos_to_slot(hx + span, hy + span);
    let mut cells: Vec<(i32, i32)> = Vec::new();
    for sx in sx0..=sx1 {
        for sy in sy0..=sy1 {
            if slot_hex(sx, sy) == (hq, hr) {
                cells.push((sx, sy));
            }
        }
    }
    let existing: Vec<(f32, f32)> = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .map(|o| (o.offset_x, o.offset_y))
        .collect();
    let mut used_cells: std::collections::HashSet<(i32, i32)> = existing
        .iter()
        .map(|(ox, oy)| world_pos_to_slot(hx + ox, hy + oy))
        .collect();

    let mut created = 0usize;
    for slot in 0..MAX_OBJECTS_PER_HEX {
        if occupied.contains(&slot) || cells.is_empty() {
            continue;
        }
        let h = obj_hash(hex_id, slot);
        let Some(kind) = natural_spawn(terrain, slot, h) else { continue };
        // Deterministic cell pick with linear probing: no two objects in a
        // hex share a ground slot.
        let start = (h % cells.len() as u64) as usize;
        let mut picked = None;
        for j in 0..cells.len() {
            let cell = cells[(start + j) % cells.len()];
            if !used_cells.contains(&cell) {
                picked = Some(cell);
                break;
            }
        }
        let Some(cell) = picked else { continue };
        used_cells.insert(cell);
        let (cx, cy) = slot_center(cell.0, cell.1);
        ctx.db.world_object().insert(crate::types::WorldObject {
            object_id: 0,
            hex_id,
            slot,
            kind: kind.to_string(),
            offset_x: cx - hx,
            offset_y: cy - hy,
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

/// Pure growth gate (Spec 022): `Some(error)` while a growing node is still
/// immature. `mature_at == 0` means the kind has no growth phase (natural
/// spawns), so those always gather.
fn growth_block(kind: &str, mature_at: u64, now: u64) -> Option<String> {
    (mature_at != 0 && now < mature_at)
        .then(|| format!("{kind} still growing ({}s left)", mature_at - now))
}

fn touch(ctx: &ReducerContext, address: &str, now: u64) {
    if let Some(mut p) = find_player(ctx, address) {
        p.last_action_at = now;
        ctx.db.player().address().update(p);
    }
}

/// Destroy a grass tuft (drops seeds), gather a fallen log, or harvest a
/// mature tree (wood + seeds). Growing nodes (planted grass/trees) refuse
/// until `mature_at` (Spec 022 §1).
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
            if let Some(e) = growth_block("Grass", obj.mature_at, now) {
                return Err(e);
            }
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
        OBJ_LOG => {
            ctx.db.world_object().object_id().delete(object_id);
            consume_slot(ctx, obj.hex_id, obj.slot);
            // 1-2 logs per node, deterministic per object id.
            let logs = 1 + object_id % 2;
            add_item(ctx, &p.address, ITEM_LOG, logs);
            add_xp(ctx, &mut p, GATHER_XP + 1, "gather_log");
            tracing::info!("GATHER-LOG {address}: {logs} logs @hex {}", obj.hex_id);
        }
        OBJ_TREE => {
            if let Some(e) = growth_block("Tree", obj.mature_at, now) {
                return Err(e);
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

/// Pure cell-occupancy check (Spec 022): does any existing object offset sit
/// in the same ground cell? Offsets snap to slot centers, so a 0.1 tolerance
/// is exact for snapped rows.
fn cell_taken(existing_offsets: &[(f32, f32)], offset_x: f32, offset_y: f32) -> bool {
    existing_offsets
        .iter()
        .any(|&(x, y)| (x - offset_x).abs() < 0.1 && (y - offset_y).abs() < 0.1)
}

/// Shared placement preflight (Spec 022): the slot must be owned by the
/// targeted hex and its ground cell must be free of other objects. Returns
/// the cell center as an offset from the hex center.
fn require_empty_cell(
    ctx: &ReducerContext,
    hex_id: u64,
    hq: i32,
    hr: i32,
    slot_x: i32,
    slot_y: i32,
) -> Result<(f32, f32), String> {
    // No placing across hex borders by aiming at a shared edge slot.
    if idlecore_core::slots::slot_hex(slot_x, slot_y) != (hq, hr) {
        return Err("Slot is outside the selected hex".to_string());
    }
    let (hx, hy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
        hq,
        hr,
        idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
    );
    let (cx, cy) = idlecore_core::slots::slot_center(slot_x, slot_y);
    let offset = (cx - hx, cy - hy);
    let existing: Vec<(f32, f32)> = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .map(|o| (o.offset_x, o.offset_y))
        .collect();
    if cell_taken(&existing, offset.0, offset.1) {
        return Err("That spot is already taken".to_string());
    }
    Ok(offset)
}

/// First unused slot number in a hex, if any.
fn free_slot(ctx: &ReducerContext, hex_id: u64) -> Option<u8> {
    let used: std::collections::HashSet<u8> = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .map(|o| o.slot)
        .collect();
    (0..MAX_OBJECTS_PER_HEX).find(|s| !used.contains(s))
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
    let (offset_x, offset_y) = require_empty_cell(ctx, hex_id, hq, hr, slot_x, slot_y)?;
    let Some(slot) = free_slot(ctx, hex_id) else {
        return Err("Hex is full".to_string());
    };

    remove_item(ctx, &p.address, ITEM_SEED, 1)
        .then_some(())
        .ok_or_else(|| "No seeds — destroy tall grass first".to_string())?;

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

/// Plant a grass tuft on an empty plot, consuming one Grass item (Spec 022
/// §1). The tuft regrows: gathering it again only succeeds after
/// `GRASS_GROWTH_SECS`, closing the resource loop.
pub fn plant_grass(
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
    // Grass grows wherever trees do — every natural land terrain.
    let plantable = matches!(
        tile.terrain.as_str(),
        "Grass" | "Forest" | "Grassland" | "Taiga" | "TropicalRainforest" | "Tundra" | "Desert"
    );
    if !plantable {
        return Err(format!("Cannot grow grass on {} here", tile.terrain));
    }
    let (offset_x, offset_y) = require_empty_cell(ctx, hex_id, hq, hr, slot_x, slot_y)?;
    let Some(slot) = free_slot(ctx, hex_id) else {
        return Err("Hex is full".to_string());
    };

    remove_item(ctx, &p.address, ITEM_GRASS, 1)
        .then_some(())
        .ok_or_else(|| "No grass — gather a mature tuft first".to_string())?;

    ctx.db.world_object().insert(WorldObjectRow {
        object_id: 0,
        hex_id,
        slot,
        kind: OBJ_GRASS.to_string(),
        offset_x,
        offset_y,
        mature_at: now + GRASS_GROWTH_SECS,
        planted_by: Some(p.address.clone()),
    });
    add_xp(ctx, &mut p, 1, "plant_grass");
    touch(ctx, &p.address, now);
    tracing::info!("PLANT-GRASS {address} @hex {hex_id} slot {slot} cell ({slot_x},{slot_y})");
    Ok(())
}

/// Build a craft bench on an empty plot of an adjacent hex by consuming
/// `BENCH_LOG_COST` logs (Spec 022 §3). No bench item exists — placing IS
/// building the bench, the only recipe that works without a bench.
pub fn place_craft_bench(
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
    // A workbench on water would be optimistic; anywhere else on land works.
    if tile.terrain == "Water" {
        return Err("Cannot build a bench on water".to_string());
    }
    let (offset_x, offset_y) = require_empty_cell(ctx, hex_id, hq, hr, slot_x, slot_y)?;
    let Some(slot) = free_slot(ctx, hex_id) else {
        return Err("Hex is full".to_string());
    };

    if !remove_item(ctx, &p.address, ITEM_LOG, BENCH_LOG_COST) {
        return Err(format!(
            "Need {BENCH_LOG_COST} logs to build a craft bench — gather fallen logs in forests"
        ));
    }

    ctx.db.world_object().insert(WorldObjectRow {
        object_id: 0,
        hex_id,
        slot,
        kind: OBJ_CRAFT_BENCH.to_string(),
        offset_x,
        offset_y,
        mature_at: 0,
        planted_by: Some(p.address.clone()),
    });
    add_xp(ctx, &mut p, 2, "place_bench");
    touch(ctx, &p.address, now);
    tracing::info!("PLACE-BENCH {address} @hex {hex_id} slot {slot} cell ({slot_x},{slot_y})");
    Ok(())
}

/// Craft at the bench occupying the targeted plot (Spec 022 §4): the
/// order-insensitive 4-ingredient multiset (Log ≡ Wood) is matched against
/// the fixed recipe table. Unknown combinations fail with "nothing happened"
/// and consume nothing; the bench stays put either way.
pub fn craft(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
    ingredients: Vec<String>,
) -> Result<(), String> {
    use crate::types::{hex_coords_of, hex_distance, now_secs, CRAFT_INGREDIENTS};
    let now = now_secs(ctx);
    let mut p = find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    cooldown_left(&p, now)?;

    let (hq, hr) = hex_coords_of(hex_id);
    if hex_distance(p.hex_q, p.hex_r, hq, hr) > 1 {
        return Err("Out of range (1 hex)".to_string());
    }
    // The target plot must hold the bench: exact cell, exact kind.
    if idlecore_core::slots::slot_hex(slot_x, slot_y) != (hq, hr) {
        return Err("Slot is outside the selected hex".to_string());
    }
    let (hx, hy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
        hq,
        hr,
        idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
    );
    let (cx, cy) = idlecore_core::slots::slot_center(slot_x, slot_y);
    let (ox, oy) = (cx - hx, cy - hy);
    let has_bench = ctx
        .db
        .world_object()
        .object_by_hex()
        .filter(hex_id)
        .any(|o| {
            o.kind == OBJ_CRAFT_BENCH
                && (o.offset_x - ox).abs() < 0.1
                && (o.offset_y - oy).abs() < 0.1
        });
    if !has_bench {
        return Err("Craft at a craft bench".to_string());
    }

    // Validate + match before consuming anything.
    if ingredients.len() != 4 {
        return Err("Nothing happened.".to_string());
    }
    let mut ings: [&str; 4] = [""; 4];
    for (slot, item) in ingredients.iter().enumerate() {
        if !CRAFT_INGREDIENTS.contains(&item.as_str()) {
            return Err("Nothing happened.".to_string());
        }
        ings[slot] = item.as_str();
    }
    let Some(result) = match_recipe(ings) else {
        return Err("Nothing happened.".to_string());
    };

    // Consume each ingredient stack (multiplicity-aware) only after
    // verifying the player owns everything — a failed craft consumes nothing.
    let mut wanted: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for item in ings {
        *wanted.entry(item).or_insert(0) += 1;
    }
    for (item, count) in &wanted {
        if count_item(ctx, &p.address, item) < *count {
            return Err(format!("Not enough {item}"));
        }
    }
    for (item, count) in &wanted {
        remove_item(ctx, &p.address, item, *count);
    }
    add_item(ctx, &p.address, result, 1);
    add_xp(ctx, &mut p, CRAFT_XP, "craft");
    touch(ctx, &p.address, now);
    tracing::info!("CRAFT {address}: {result} @bench hex {hex_id}");
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
    fn logs_spawn_only_in_forested_terrains() {
        // Spec 022 §2: fallen logs are forest-floor nodes — never elsewhere.
        for slot in 0..MAX_OBJECTS_PER_HEX {
            for seed in 0..256u64 {
                let h = obj_hash(seed, slot);
                for terrain in [
                    TerrainType::Grass,
                    TerrainType::Grassland,
                    TerrainType::Desert,
                    TerrainType::Tundra,
                    TerrainType::Mountain,
                    TerrainType::Water,
                    TerrainType::City,
                    TerrainType::Polluted,
                ] {
                    assert_ne!(
                        natural_spawn(terrain, slot, h),
                        Some(OBJ_LOG),
                        "log spawned on non-forested {terrain:?}"
                    );
                }
                let forested = [
                    TerrainType::Forest,
                    TerrainType::Taiga,
                    TerrainType::TropicalRainforest,
                ];
                let any = forested
                    .iter()
                    .any(|t| natural_spawn(*t, slot, h) == Some(OBJ_LOG));
                if slot > 2 {
                    assert!(!any, "log spawned outside the grass/log slot band");
                }
            }
        }
        // The forest band must actually produce both grass and logs.
        let mut grass = 0;
        let mut logs = 0;
        for slot in 0..=2u8 {
            for seed in 0..256u64 {
                match natural_spawn(TerrainType::Forest, slot, obj_hash(seed, slot)) {
                    Some(k) if k == OBJ_GRASS => grass += 1,
                    Some(k) if k == OBJ_LOG => logs += 1,
                    _ => {}
                }
            }
        }
        assert!(grass > 10 && logs > 5, "forest band barren: grass {grass}, logs {logs}");
    }

    #[test]
    fn growth_gate_blocks_only_before_maturity() {
        // Spec 022 §1: immature planted grass refuses with "still growing".
        assert_eq!(growth_block("Grass", 1000, 999).as_deref(), Some("Grass still growing (1s left)"));
        assert_eq!(growth_block("Grass", 1000, 1000), None, "mature at exact time");
        // Natural spawns carry mature_at == 0 — no growth phase, always open.
        assert_eq!(growth_block("Grass", 0, 0), None);
        // Trees share the same gate.
        assert!(growth_block("Tree", 600, 1).is_some());
        assert_eq!(growth_block("Tree", 600, 600), None);
    }

    #[test]
    fn cell_taken_matches_only_nearby_offsets() {
        let existing = [(1.0, 2.0), (-3.5, 0.25)];
        assert!(cell_taken(&existing, 1.0, 2.0));
        assert!(cell_taken(&existing, 1.05, 2.0), "snapped rows are exact");
        assert!(!cell_taken(&existing, 1.5, 2.0));
        assert!(!cell_taken(&existing, 0.0, 0.0));
        assert!(!cell_taken(&[], 0.0, 0.0));
    }

    #[test]
    fn slot_centered_offsets_stay_inside_the_hex() {
        // The roller only places nodes on cells owned by the hex; every
        // owned cell's center must lie within the hex circumradius, so
        // snapped offsets can never escape the hex.
        use idlecore_core::slots::{slot_center, slot_hex, world_pos_to_slot};
        use idlecore_core::world_gen::WorldGenConfig;
        for hex_id in [0u64, 12345, u64::from(u32::MAX)] {
            let (hq, hr) = crate::types::hex_coords_of(hex_id);
            let (hx, hy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
                hq,
                hr,
                WorldGenConfig::HEX_SIZE,
            );
            let (sq, sr) = world_pos_to_slot(hx, hy);
            let mut owned = 0;
            for sx in sq - 5..=sq + 5 {
                for sy in sr - 5..=sr + 5 {
                    if slot_hex(sx, sy) != (hq, hr) {
                        continue;
                    }
                    owned += 1;
                    let (cx, cy) = slot_center(sx, sy);
                    let (ox, oy) = (cx - hx, cy - hy);
                    assert!(
                        ox * ox + oy * oy <= 10.0_f32 * 10.0 + 1e-3,
                        "owned cell center outside hex {hex_id}"
                    );
                }
            }
            assert!(owned >= 8, "hex {hex_id} should own several ground cells");
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
