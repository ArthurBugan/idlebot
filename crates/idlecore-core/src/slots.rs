//! Ground slot grid — Stardew-style square placement cells.
//!
//! The floor renders as a grid of square tiles (one 16px art tile each,
//! `SLOT_SIZE` world units per side) aligned to the world origin. Hexes only
//! govern gameplay (terrain, walkability, actions): every square tile belongs
//! to the hex containing its center, so actions on a tile route to exactly
//! one hex. The client selector snaps to tiles; the server plants at the
//! selected tile's center inside its hex.

use crate::hex::world_pos_to_hex;
use crate::world_gen::WorldGenConfig;

/// Square tiles per hex width (the floor art repeats the 16px tile 4×).
pub const SLOTS_PER_HEX: i32 = 4;

/// Slot edge length in world units (√3·HEX_SIZE / 4 ≈ 4.33 for size 10).
pub const SLOT_SIZE: f32 = 1.7320508075688772 * WorldGenConfig::HEX_SIZE / SLOTS_PER_HEX as f32;

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

/// Axial hex (q, r) that owns a slot: the hex containing the slot's center.
/// A slot straddling a hex edge belongs to whichever hex its center falls in,
/// so exactly one hex owns each slot.
pub fn slot_hex(sx: i32, sy: i32) -> (i32, i32) {
    let (cx, cy) = slot_center(sx, sy);
    world_pos_to_hex(cx, cy, WorldGenConfig::HEX_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_grid::HexGrid;

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
    fn slot_hex_is_consistent_and_single_valued() {
        // Every slot center maps back to its own hex, and neighboring slots
        // never claim two different hexes for the same center.
        for h in HexGrid::hexes_in_radius(0, 0, 4) {
            let (hx, hy) = HexGrid::axial_to_world(h.q, h.r, WorldGenConfig::HEX_SIZE);
            let (sx0, sy0) = world_pos_to_slot(hx, hy);
            for sx in sx0 - 4..=sx0 + 4 {
                for sy in sy0 - 4..=sy0 + 4 {
                    let (cx, cy) = slot_center(sx, sy);
                    if world_pos_to_hex(cx, cy, WorldGenConfig::HEX_SIZE) == (h.q, h.r) {
                        assert_eq!(slot_hex(sx, sy), (h.q, h.r));
                    }
                }
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
    }
}
