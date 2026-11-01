//! Ground slot grid — square tiles (Stardew-style square placement cells).
//!
//! The floor renders as a grid of square tiles (one 16px art tile each,
//! `SLOT_SIZE` world units per side) aligned to the world origin. Gameplay
//! tiles (17.32 wu squares) govern terrain, walkability and actions: every
//! square slot belongs to the `SLOTS_PER_TILE`-divided gameplay tile
//! containing it, so actions on a slot route to exactly one tile. The
//! client selector snaps to slots; the server plants at the selected
//! slot's center inside its tile.

use crate::grid::world_pos_to_tile;
use crate::world_gen::WorldGenConfig;

/// Square slots per gameplay-tile edge (the floor art repeats the 16px
/// tile 4× per grid edge).
pub const SLOTS_PER_TILE: i32 = 4;

/// Slot edge length in world units (TILE_SIZE / 4 ≈ 4.33).
pub const SLOT_SIZE: f32 = crate::grid::TILE_SIZE / SLOTS_PER_TILE as f32;

/// World position → slot index (floor division; y = north).
pub fn world_pos_to_slot(x: f32, y: f32) -> (i32, i32) {
    (
        (x / SLOT_SIZE).floor() as i32,
        (y / SLOT_SIZE).floor() as i32,
    )
}

/// Slot index → world position of the slot's center.
pub fn slot_center(sx: i32, sy: i32) -> (f32, f32) {
    (
        (sx as f32 + 0.5) * SLOT_SIZE,
        (sy as f32 + 0.5) * SLOT_SIZE,
    )
}

/// Gameplay tile (tx, ty) that owns a slot: the `SLOTS_PER_TILE` block
/// containing the slot — an exact integer division because slot edges
/// align with tile edges (both anchored at the world origin).
pub fn slot_tile(sx: i32, sy: i32) -> (i32, i32) {
    (
        sx.div_euclid(SLOTS_PER_TILE),
        sy.div_euclid(SLOTS_PER_TILE),
    )
}

/// World position → the gameplay tile that owns the slot it stands on.
pub fn pos_slot_tile(x: f32, y: f32) -> (i32, i32) {
    let (sx, sy) = world_pos_to_slot(x, y);
    slot_tile(sx, sy)
}

/// Cross-check: the tile computed from slot centers equals the raw
/// world→tile lookup for every slot (kept as a test-level invariant).
pub fn slot_tile_via_world(sx: i32, sy: i32) -> (i32, i32) {
    let (cx, cy) = slot_center(sx, sy);
    world_pos_to_tile(cx, cy, WorldGenConfig::TILE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::TileCoord;

    #[test]
    fn world_slot_roundtrip_hits_slot_centers() {
        for sx in -50..=50 {
            for sy in -50..=50 {
                let (cx, cy) = slot_center(sx, sy);
                assert_eq!(world_pos_to_slot(cx, cy), (sx, sy));
                // Quarter-offsets stay inside the slot.
                for (dx, dy) in [(0.3, 0.0), (-0.45, 0.45), (0.49, -0.49)] {
                    assert_eq!(
                        world_pos_to_slot(cx + dx * SLOT_SIZE, cy + dy * SLOT_SIZE),
                        (sx, sy)
                    );
                }
            }
        }
    }

    #[test]
    fn slot_tile_equals_world_lookup_everywhere() {
        // Integer division block mapping must equal the raw world→tile
        // conversion on every slot center.
        for sx in -40..=40 {
            for sy in -40..=40 {
                assert_eq!(slot_tile(sx, sy), slot_tile_via_world(sx, sy));
            }
        }
    }

    #[test]
    fn each_tile_owns_exactly_sixteen_slots() {
        let (tx, ty) = slot_tile(10, -7);
        let mut count = 0;
        for sx in (tx * SLOTS_PER_TILE)..(tx * SLOTS_PER_TILE + SLOTS_PER_TILE) {
            for sy in (ty * SLOTS_PER_TILE)..(ty * SLOTS_PER_TILE + SLOTS_PER_TILE) {
                assert_eq!(slot_tile(sx, sy), (tx, ty));
                count += 1;
            }
        }
        assert_eq!(count, (SLOTS_PER_TILE * SLOTS_PER_TILE) as usize);
    }

    #[test]
    fn slot_tile_agrees_with_tile_coord() {
        for sx in -20..=20 {
            for sy in -20..=20 {
                let (tx, ty) = slot_tile(sx, sy);
                let t = TileCoord::new(tx, ty);
                assert_eq!(t.x, tx);
                assert_eq!(t.y, ty);
                assert_eq!(t.distance(&TileCoord::new(tx, ty)), 0);
            }
        }
    }

    #[test]
    fn negative_coords_floor_correctly() {
        // Floor, not truncation: -0.5 slots must land in slot -1.
        assert_eq!(world_pos_to_slot(-0.5, -0.5), (-1, -1));
        assert_eq!(world_pos_to_slot(0.0, 0.0), (0, 0));
        assert_eq!(slot_center(-1, -1).0, -0.5 * SLOT_SIZE);
        assert_eq!(slot_center(-1, -1).1, -0.5 * SLOT_SIZE);
        // -7 slots (world ≈ -30.3) are tile row -2 (block [-8,-5)).
        assert_eq!(slot_tile(0, -7), (0, -2));
    }
}
