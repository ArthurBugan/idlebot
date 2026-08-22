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

/// Whether a natural node spawns in `slot` for this terrain, and its kind.
fn natural_spawn(terrain: TerrainType, slot: u8, h: u64) -> Option<&'static str> {
    let roll = h % 100;
    let kind = match terrain {
        TerrainType::Grass => {
            if slot == 5 {
                (roll < 10).then_some(OBJ_ROCK) // scattered boulders
            } else if slot <= 2 {
                (roll < 85).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Grassland => {
            if slot == 5 {
                (roll < 12).then_some(OBJ_ROCK)
            } else if slot <= 1 {
                (roll < 75).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Forest
        | TerrainType::Taiga
        | TerrainType::TropicalRainforest => {
            if slot == 5 {
                (roll < 8).then_some(OBJ_ROCK)
            } else if slot <= 2 {
                (roll < 70).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Desert | TerrainType::Tundra => {
            if slot == 5 {
                (roll < 15).then_some(OBJ_ROCK)
            } else if slot == 0 {
                (roll < 30).then_some(OBJ_GRASS)
            } else {
                None
            }
        }
        TerrainType::Mountain => {
            if slot == 4 {
                (roll < 35).then_some(OBJ_ROCK)
            } else if slot == 0 {
                (roll < 15).then_some(OBJ_GRASS)
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
pub fn ensure_objects(ctx: &ReducerContext, hex_id: u64, terrain: TerrainType) -> usize {
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
pub fn consume_slot(ctx: &ReducerContext, hex_id: u64, slot: u8) {
    let key = format!("{{hex_id}}:{{slot}}");
    if ctx.db.object_removed().key().find(key.clone()).is_none() {
        ctx.db.object_removed().insert(crate::types::ObjectRemoved { key, hex_id, slot });
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

/// Plant a tree on an adjacent land hex, consuming one seed.
pub fn plant_tree(ctx: &ReducerContext, address: &str, hex_id: u64) -> Result<(), String> {
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

    let h = obj_hash(hex_id, slot.wrapping_add(97));
    let offset_x = ((h >> 8) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
    let offset_y = ((h >> 16) & 0xFF) as f32 / 255.0 * 9.0 - 4.5;
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
    tracing::info!("PLANT-TREE {address} @hex {hex_id} slot {slot}");
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
}
