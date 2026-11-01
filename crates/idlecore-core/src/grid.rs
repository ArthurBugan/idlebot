//! Square world grid — one square tile per gameplay cell.
//!
//! Replaces the old axial hex grid. Tiles are squares of [`TILE_SIZE`] world
//! units per side (the old hex footprint: √3·HEX_SIZE), centers aligned to
//! the world origin, `y` = north. Distance is Chebyshev (8-way movement:
//! a diagonal step costs one step, exactly like a hex neighbor count did).

use serde::{Deserialize, Serialize};

/// Square tile defined by integer lattice coordinates (x, y).
///
/// Linked gameplay cells keep the same u64 id packing the server persisted
/// (`(x as u32) << 32 | (y as u32)`), matching the server's `tile_id_of` and
/// `WorldCell::id_of` so ids agree across client, server and persisted rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

/// Tile edge length in world units: the old hex footprint width
/// (√3 · HEX_SIZE ≈ 17.32 for HEX_SIZE 10), so scale, chunk spans and the
/// claim-square metrics line up with the pre-square world.
pub const TILE_SIZE: f32 = crate::world_gen::WorldGenConfig::TILE_SIZE;

impl TileCoord {
    /// Create a tile from lattice coordinates.
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Center position of this tile in world units, y north.
    pub fn center(&self, tile_size: f32) -> [f32; 3] {
        let (x, y) = self.to_pixel(tile_size);
        [x, y, 0.0]
    }

    /// Convert the tile to world coordinates: its center.
    pub fn to_pixel(&self, tile_size: f32) -> (f32, f32) {
        WorldGrid::tile_center(self.x, self.y, tile_size)
    }

    /// Serialize the tile to a u64 id using the canonical encoding
    /// `(x as u32) << 32 | (y as u32)`.
    pub fn to_id(&self) -> u64 {
        ((self.x as u32 as u64) << 32) | (self.y as u32 as u64)
    }

    /// Parse a tile id back into a TileCoord (inverse of [`Self::to_id`]).
    ///
    /// Garbage/sentinel ids (e.g. `u64::MAX`) would decode to coordinates
    /// near ±i32::MAX; clamp to a band far wider than any real world
    /// (radius 1 000 000) so decoding can never panic.
    pub fn from_id(id: u64) -> Self {
        const LIMIT: i64 = 1_000_000_000;
        let x = ((id >> 32) as u32 as i32 as i64).clamp(-LIMIT, LIMIT);
        let y = ((id & 0xFFFF_FFFF) as u32 as i32 as i64).clamp(-LIMIT, LIMIT);
        Self::new(x as i32, y as i32)
    }

    /// Get the 8 neighboring tile coordinates (directions 0-7).
    pub fn neighbors(&self) -> [(i32, i32); 8] {
        let dirs = [
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
        ];
        dirs.map(|(dx, dy)| (self.x + dx, self.y + dy))
    }

    /// Get the neighbor at a specific direction (0-7).
    pub fn neighbor(&self, direction: i32) -> Self {
        let dirs = [
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
        ];
        let (dx, dy) = dirs[direction as usize];
        Self::new(self.x + dx, self.y + dy)
    }

    /// Chebyshev distance between two tiles (8-way steps).
    pub fn distance(&self, other: &TileCoord) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

/// Distance in 8-way steps between two tile coordinates.
pub fn tile_distance(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    (x1 - x2).abs().max((y1 - y2).abs())
}

/// Convert a world 2D position to integer tile coordinates (floor).
pub fn world_pos_to_tile(world_x: f32, world_z: f32, tile_size: f32) -> (i32, i32) {
    WorldGrid::world_to_tile(world_x, world_z, tile_size)
}

/// Square-grid world math. One namespace mirroring the old `WorldGrid`.
pub struct WorldGrid;

impl WorldGrid {
    /// Center of tile (x, y) in world units: aligned squares, so
    /// tile (0, 0) spans [0, TILE) on both axes.
    pub fn tile_center(x: i32, y: i32, tile_size: f32) -> (f32, f32) {
        (
            x as f32 * tile_size + tile_size * 0.5,
            y as f32 * tile_size + tile_size * 0.5,
        )
    }

    /// World position → tile coordinates (floor division, y = north).
    pub fn world_to_tile(world_x: f32, world_z: f32, tile_size: f32) -> (i32, i32) {
        (
            (world_x / tile_size).floor() as i32,
            (world_z / tile_size).floor() as i32,
        )
    }

    /// Calculate Chebyshev distance between two tiles.
    pub fn distance(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        (x1 - x2).unsigned_abs().max((y1 - y2).unsigned_abs()) as i32
    }

    /// All tiles whose Chebyshev distance from the center is ≤ radius —
    /// the square analogue of the old hex radius list.
    pub fn tiles_in_square(center_x: i32, center_y: i32, radius: i32) -> Vec<TileCoord> {
        let mut result = Vec::new();
        for x in -radius..=radius {
            for y in -radius..=radius {
                result.push(TileCoord::new(center_x + x, center_y + y));
            }
        }
        result
    }
}

impl core::ops::Add for TileCoord {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl core::ops::Sub for TileCoord {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_center_is_lattice() {
        let t = TileCoord::new(3, -2);
        let (x, y) = t.to_pixel(20.0);
        assert_eq!(x, 70.0);
        assert_eq!(y, -30.0);
    }

    #[test]
    fn center_hex_at_origin_maps_to_tile_zero() {
        // Tile (0, 0) centered at TILE/2, TILE/2; the origin sits inside it.
        let (x, y) = world_pos_to_tile(0.0, 0.0, 10.0);
        assert_eq!((x, y), (0, 0));
        let (x, y) = world_pos_to_tile(TILE_SIZE * 0.5, TILE_SIZE * 0.5, TILE_SIZE);
        assert_eq!((x, y), (0, 0));
    }

    #[test]
    fn to_id_roundtrip_all_quadrants() {
        for (x, y) in [(5, -3), (-3, 5), (5, -100), (-50, -40), (0, 0)] {
            let t = TileCoord::new(x, y);
            assert_eq!(TileCoord::from_id(t.to_id()), t);
        }
    }

    #[test]
    fn id_encoding_matches_canonical_world_cell_and_server_scheme() {
        use crate::world_gen::WorldCell;
        for x in -200..=200 {
            for y in -200..=200 {
                let t = TileCoord::new(x, y);
                assert_eq!(t.to_id(), WorldCell::id_of(x, y), "({x},{y})");
                assert_eq!(TileCoord::from_id(t.to_id()), t);
            }
        }
    }

    #[test]
    fn garbage_id_does_not_panic() {
        for id in [u64::MAX, 1, 0xFFFFFFFF, 0x80000000_00000000] {
            let t = TileCoord::from_id(id);
            assert!(t.x.abs() <= 1_000_000_000 && t.y.abs() <= 1_000_000_000);
        }
    }

    #[test]
    fn neighbors_count_and_reach() {
        let t = TileCoord::new(4, 3);
        let neighbors = t.neighbors();
        assert_eq!(neighbors.len(), 8);
        for (x, y) in &neighbors {
            assert_eq!(t.distance(&TileCoord::new(*x, *y)), 1);
        }
        // 8-way distance: diagonal is one step.
        assert_eq!(tile_distance(0, 0, 3, 3), 3);
        assert_eq!(tile_distance(0, 0, -2, 5), 5);
        assert_eq!(tile_distance(2, -1, 2, -1), 0);
    }

    #[test]
    fn tiles_in_square_is_square_and_inclusive() {
        let tiles = WorldGrid::tiles_in_square(10, -4, 2);
        assert_eq!(tiles.len(), 25);
        assert!(tiles.contains(&TileCoord::new(12, -6)));
        assert!(tiles.contains(&TileCoord::new(8, -2)));
        assert!(!tiles.contains(&TileCoord::new(13, -4)));
    }

    #[test]
    fn floor_division_roundtrip() {
        let mut x = -40.0_f32;
        while x < 40.0 {
            let mut y = -40.0_f32;
            while y < 40.0 {
                let (tx, ty) = WorldGrid::world_to_tile(x, y, TILE_SIZE);
                let (cx, cy) = WorldGrid::tile_center(tx, ty, TILE_SIZE);
                assert_eq!(WorldGrid::world_to_tile(cx, cy, TILE_SIZE), (tx, ty));
                y += 3.3;
            }
            x += 5.1;
        }
    }
}
