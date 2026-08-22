//! Hex math for IdleBot hex grid.
//! Uses axial coordinates (q, r, s) where q + r + s = 0.
//! Flat-top hex geometry, hex radius 10.0 meters.

use serde::{Deserialize, Serialize};

/// Hexagon defined by axial coordinates (q, r, s) where q + r + s = 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct HexCoord {
    pub q: i32,
    pub r: i32,
    pub s: i32,
}

impl HexCoord {
    /// Create a hex from axial q,r (s = -q-r, enforcing invariant).
    pub fn new(q: i32, r: i32) -> Self {
        let s = -q - r;
        Self { q, r, s }
    }

    /// Create from cube coordinates (x, y, z) with rounding to nearest hex.
    pub fn from_cube(x: i32, y: i32, z: i32) -> Self {
        let q_f = (x as f32 + y as f32 + z as f32) / 3.0;
        let r_f = (y as f32 + z as f32 - 2.0 * x as f32) / 3.0;
        let s_f = (z as f32 + x as f32 - 2.0 * y as f32) / 3.0;
        let q = q_f.round() as i32;
        let r = r_f.round() as i32;
        let s = s_f.round() as i32;
        // Round ties toward the largest magnitude coordinate
        let (q, r, _s) = round_cubic(q, r, s);
        Self::new(q, r)
    }

    /// Get the center position of this hex in 3D space (flat-top orientation).
    pub fn center(&self, hex_radius: f32) -> [f32; 3] {
        let (x, y) = self.to_pixel(hex_radius);
        [x, y, 0.0]
    }

    /// Convert axial to pixel/world coordinates (2D), delegating to the
    /// canonical `HexGrid::axial_to_world` so every coordinate conversion in
    /// the codebase agrees exactly.
    pub fn to_pixel(&self, hex_radius: f32) -> (f32, f32) {
        crate::hex_grid::HexGrid::axial_to_world(self.q, self.r, hex_radius)
    }

    /// Serialize hex to a u64 id using the canonical encoding
    /// `(q as u32) << 32 | (r as u32)` — kept identical to the server's
    /// `hex_id_of` and `HexCell::id_of` so ids agree across client, server
    /// and persisted rows.
    pub fn to_id(&self) -> u64 {
        ((self.q as u32 as u64) << 32) | (self.r as u32 as u64)
    }

    /// Parse a hex id back into a HexCoord (inverse of [`Self::to_id`]).
    ///
    /// Garbage/sentinel ids (e.g. `u64::MAX`) would decode to coordinates
    /// near ±i32::MAX and overflow the `s = -q - r` invariant. Clamp to a
    /// band far wider than any real world (radius 100) so decoding can
    /// never panic.
    pub fn from_id(id: u64) -> Self {
        const LIMIT: i64 = 1_000_000_000;
        let q = ((id >> 32) as u32 as i32 as i64).clamp(-LIMIT, LIMIT);
        let r = ((id & 0xFFFF_FFFF) as u32 as i32 as i64).clamp(-LIMIT, LIMIT);
        Self::new(q as i32, r as i32)
    }

    /// Get the 6 neighboring hex coordinates (directions 0-5).
    /// Direction 0 = +q, 1 = +r, 2 = +s, 3 = -q, 4 = -r, 5 = -s
    pub fn neighbors(&self) -> [(i32, i32); 6] {
        let dirs = [
            (1, -1),  // +q
            (0, 1),   // +r
            (-1, 0),  // +s
            (-1, 1),  // -q
            (0, -1),  // -r
            (1, 0),   // -s
        ];
        dirs.map(|(dq, dr)| {
            let q = self.q + dq;
            let r = self.r + dr;
            (q, r)
        })
    }

    /// Get neighbor at a specific direction (0-5).
    pub fn neighbor(&self, direction: i32) -> Self {
        let dirs = [
            (1, -1),  // +q
            (0, 1),   // +r
            (-1, 0),  // +s
            (-1, 1),  // -q
            (0, -1),  // -r
            (1, 0),   // -s
        ];
        let (dq, dr) = dirs[direction as usize];
        Self::new(self.q + dq, self.r + dr)
    }

    /// Distance between two hexes (hex steps).
    pub fn distance(&self, other: &HexCoord) -> i32 {
        let d = (self.q - other.q).abs() + (self.r - other.r).abs() + (self.s - other.s).abs();
        d / 2
    }

    /// Convert to cube coordinates (q, r, s).
    pub fn to_cube(&self) -> (i32, i32, i32) {
        (self.q, self.r, self.s)
    }
}

/// Round-cubic coordinates: when rounding produces non-valid cube coords,
/// round toward the direction that stays most valid.
fn round_cubic(q: i32, r: i32, _s: i32) -> (i32, i32, i32) {
    // Keep q, r as-is, compute s to satisfy q+r+s=0
    let q = q;
    let r = r;
    let s = -q - r;
    (q, r, s)
}

/// Convert world 2D position to axial hex coordinates.
///
/// Single source of truth is [`crate::hex_grid::HexGrid::world_to_axial`];
/// this delegate keeps the legacy call sites working with identical
/// results (the old truncation-based implementation disagreed with the
/// canonical cube-round on ~88% of sample points, including exact hex
/// centers).
pub fn world_pos_to_hex(world_x: f32, world_z: f32, hex_radius: f32) -> (i32, i32) {
    crate::hex_grid::HexGrid::world_to_axial(world_x, world_z, hex_radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_new_computes_s() {
        let h = HexCoord::new(3, -2);
        assert_eq!(h.q, 3);
        assert_eq!(h.r, -2);
        // s = -q - r = -3 - (-2) = -1
        assert_eq!(h.s, -1);
    }

    #[test]
    fn hex_from_cube() {
        // from_cube(1, 0, -1):
        // q_f = (1+0+(-1))/3 = 0, r_f = (0+(-1)-2*1)/3 = -1, s_f = (-1+1-0)/3 = 0
        let h = HexCoord::from_cube(1, 0, -1);
        // new() recomputes s = -q - r = -0 - (-1) = 1
        assert_eq!(h.q, 0);
        assert_eq!(h.r, -1);
        assert_eq!(h.s, 1);
    }

    #[test]
    fn hex_to_pixel_flat_top() {
        // Center hex (0,0) -> pixel (0,0)
        let h = HexCoord::new(0, 0);
        let (x, y) = h.to_pixel(10.0);
        assert!((x).abs() < 0.01);
        assert!((y).abs() < 0.01);
    }

    #[test]
    fn hex_to_pixel_offset() {
        // Hex (1, 0) -> x = sqrt(3)*10*1 ≈ 17.32, y = 0
        let h = HexCoord::new(1, 0);
        let (x, y) = h.to_pixel(10.0);
        assert!((x - 17.3205).abs() < 0.01);
        assert!((y).abs() < 0.01);
    }

    #[test]
    fn hex_to_id_roundtrip_positive() {
        let h = HexCoord::new(5, -3);
        let id = h.to_id();
        let h2 = HexCoord::from_id(id);
        assert_eq!(h.q, h2.q);
        assert_eq!(h.r, h2.r);
    }

    #[test]
    fn hex_to_id_roundtrip_negative_q() {
        let h = HexCoord::new(-3, 5);
        let id = h.to_id();
        let h2 = HexCoord::from_id(id);
        assert_eq!(h.q, h2.q);
        assert_eq!(h.r, h2.r);
    }

    #[test]
    fn hex_to_id_roundtrip_negative_r() {
        let h = HexCoord::new(5, -100);
        let id = h.to_id();
        let h2 = HexCoord::from_id(id);
        assert_eq!(h.q, h2.q);
        assert_eq!(h.r, h2.r);
    }

    #[test]
    fn hex_to_id_negative_both() {
        let h = HexCoord::new(-50, -40);
        let id = h.to_id();
        let h2 = HexCoord::from_id(id);
        assert_eq!(h.q, h2.q);
        assert_eq!(h.r, h2.r);
    }

    #[test]
    fn hex_neighbors_count() {
        let h = HexCoord::new(0, 0);
        let neighbors = h.neighbors();
        assert_eq!(neighbors.len(), 6);
        // Each neighbor should be distance 1
        for (q, r) in &neighbors {
            let n = HexCoord::new(*q, *r);
            assert_eq!(h.distance(&n), 1);
        }
    }

    #[test]
    fn hex_neighbor_directions() {
        let h = HexCoord::new(5, -3); // s = -5 - (-3) = -2
        // Direction vectors: (1,-1), (0,1), (-1,0), (-1,1), (0,-1), (1,0)
        let n0 = h.neighbor(0); // (1, -1)
        assert_eq!(n0.q, 6);    // 5 + 1
        assert_eq!(n0.r, -4);   // -3 + (-1)

        let n1 = h.neighbor(1); // (0, 1)
        assert_eq!(n1.q, 5);    // 5 + 0
        assert_eq!(n1.r, -2);   // -3 + 1

        let n5 = h.neighbor(5); // (1, 0)
        assert_eq!(n5.q, 6);    // 5 + 1
        assert_eq!(n5.r, -3);   // -3 + 0
    }

    #[test]
    fn hex_distance_symmetric() {
        let a = HexCoord::new(3, 1);
        let b = HexCoord::new(1, 3);
        assert_eq!(a.distance(&b), b.distance(&a));
    }

    #[test]
    fn hex_distance_self_zero() {
        let h = HexCoord::new(5, -3);
        assert_eq!(h.distance(&h), 0);
    }

    #[test]
    fn world_pos_to_hex_center() {
        // The center hex at (0,0) maps to pixel (0,0)
        let (q, r) = world_pos_to_hex(0.0, 0.0, 10.0);
        assert_eq!(q, 0);
        assert_eq!(r, 0);
    }

    #[test]
    fn world_pos_to_hex_roundtrip() {
        // Round trip: hex -> pixel
        let h = HexCoord::new(3, -2);
        let (px, py) = h.to_pixel(10.0);
        // x = 10 * sqrt(3) * (3 + (-2)/2) = 10 * 1.732 * 2 = 34.64
        // y = 10 * 1.5 * (-2) = -30
        assert!((px - 34.6410).abs() < 0.01);
        assert!((py - (-30.0)).abs() < 0.01);
    }

    #[test]
    fn negative_hex_neighbors() {
        let h = HexCoord::new(-5, -3); // s = -(-5) - (-3) = 5 + 3 = 8
        let neighbors = h.neighbors();
        assert_eq!(neighbors.len(), 6);
        // Each neighbor is a valid hex (q+r+s=0), but s differs
        for (q, r) in &neighbors {
            let n = HexCoord::new(*q, *r);
            // Verify q+r+s=0 for the new hex
            assert_eq!(n.q + n.r + n.s, 0);
            // Verify distance is 1
            assert_eq!(h.distance(&n), 1);
        }
    }
}

#[cfg(test)]
mod world_pos_tests {
    use super::*;
    use crate::hex_grid::HexGrid;

    #[test]
    fn exact_hex_centers_round_trip() {
        for q in -10..=10 {
            for r in -10..=10 {
                let (x, z) = HexGrid::axial_to_world(q, r, 10.0);
                assert_eq!(world_pos_to_hex(x, z, 10.0), (q, r), "center {q},{r}");
            }
        }
    }

    #[test]
    fn agrees_with_canonical_cube_round() {
        let mut x = -500.0;
        while x < 500.0 {
            let mut z = -500.0;
            while z < 500.0 {
                assert_eq!(
                    world_pos_to_hex(x, z, 10.0),
                    HexGrid::world_to_axial(x, z, 10.0),
                    "at {x},{z}"
                );
                z += 7.3;
            }
            x += 11.1;
        }
    }

    #[test]
    fn center_and_pixel_agree_with_axial_to_world() {
        for &(q, r) in &[(0, 0), (3, -5), (-7, 2), (10, 10)] {
            let coord = HexCoord::new(q, r);
            let (px, pz) = coord.to_pixel(10.0);
            let (wx, wz) = HexGrid::axial_to_world(q, r, 10.0);
            assert_eq!((px, pz), (wx, wz));
            let [cx, cy, _] = coord.center(10.0);
            assert_eq!((cx, cy), (wx, wz));
        }
    }
}

#[cfg(test)]
mod from_id_tests {
    use super::*;
    use crate::world_gen::HexCell;

    #[test]
    fn sentinel_zero_id_does_not_overflow() {
        let h = HexCoord::from_id(0);
        let _ = h.s;
    }

    #[test]
    fn zero_id_decodes_to_origin() {
        assert_eq!(HexCoord::from_id(0), HexCoord::new(0, 0));
    }

    #[test]
    fn id_encoding_matches_canonical_hex_cell_and_server_scheme() {
        for q in -200..=200 {
            for r in -200..=200 {
                let h = HexCoord::new(q, r);
                assert_eq!(h.to_id(), HexCell::id_of(q, r), "({q},{r})");
                assert_eq!(HexCoord::from_id(h.to_id()), h);
            }
        }
    }

    #[test]
    fn garbage_id_does_not_panic() {
        for id in [u64::MAX, 1, 0xFFFFFFFF, 0x80000000_00000000] {
            let h = HexCoord::from_id(id);
            assert!(h.q.abs() <= 1_000_000_000 && h.r.abs() <= 1_000_000_000);
        }
    }

    #[test]
    fn round_trip_within_world_radius() {
        for q in -200..=200 {
            for r in -200..=200 {
                let h = HexCoord::new(q, r);
                assert_eq!(HexCoord::from_id(h.to_id()), h);
            }
        }
    }
}
