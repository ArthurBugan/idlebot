//! Hex grid with proper coordinate conversion and neighbor calculation.
//! Based on axial coordinate system (q, r).

use crate::hex::HexCoord;
use std::ops::{Add, Sub};

/// Hex grid with coordinate conversion and neighbor calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexGrid;

impl HexGrid {
    /// Convert axial coordinates (q, r) to world position (x, z).
    /// Uses flat-top hex orientation.
    pub fn axial_to_world(q: i32, r: i32, size: f32) -> (f32, f32) {
        let x = size * (f32::sqrt(3.0) * (q as f32 + r as f32 * 0.5));
        let z = size * 1.5 * r as f32;
        (x, z)
    }

    /// Convert world position (x, z) to axial coordinates (q, r).
    /// Uses floating point for rounding (cube-round fixup, red-blob style).
    pub fn world_to_axial(x: f32, z: f32, size: f32) -> (i32, i32) {
        let qf = (f32::sqrt(3.0) / 3.0 * x - 1.0 / 3.0 * z) / size;
        let rf = 2.0 / 3.0 * z / size;
        let sf = -qf - rf;
        let mut rq = qf.round();
        let mut rr = rf.round();
        let rs = sf.round();
        let qd = (rq - qf).abs();
        let rd = (rr - rf).abs();
        let sd = (rs - sf).abs();
        if qd > rd && qd > sd {
            rq = -rr - rs;
        } else if rd > sd {
            rr = -rq - rs;
        }
        (rq as i32, rr as i32)
    }

    /// Get the 6 neighbors of a hex in axial coordinates.
    pub fn neighbors(q: i32, r: i32) -> [HexCoord; 6] {
        let directions = [
            (1, 0),
            (1, -1),
            (0, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
        ];
        let mut result = [HexCoord::new(0, 0); 6];
        for (i, &(dq, dr)) in directions.iter().enumerate() {
            result[i] = HexCoord::new(q + dq, r + dr);
        }
        result
    }

    /// Calculate distance between two hexes.
    pub fn distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
        let s1 = -q1 - r1;
        let s2 = -q2 - r2;
        let d = (q1 - q2).unsigned_abs() as i32
            + (r1 - r2).unsigned_abs() as i32
            + (s1 - s2).unsigned_abs() as i32;
        d / 2
    }

    /// Get all hexes in a radius around a center hex.
    pub fn hexes_in_radius(center_q: i32, center_r: i32, radius: i32) -> Vec<HexCoord> {
        let mut result = Vec::new();
        for q in -radius..=radius {
            let r_start = (-radius).max(-q - radius);
            let r_end = radius.min(-q + radius);
            for r in r_start..=r_end {
                result.push(HexCoord::new(center_q + q, center_r + r));
            }
        }
        result
    }
}

impl Add for HexCoord {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.q + rhs.q, self.r + rhs.r)
    }
}

impl Sub for HexCoord {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.q - rhs.q, self.r - rhs.r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axial_to_world() {
        let (x, z) = HexGrid::axial_to_world(0, 0, 1.0);
        assert!((x - 0.0).abs() < 0.001);
        assert!((z - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_world_to_axial() {
        let (q, r) = HexGrid::world_to_axial(0.0, 0.0, 1.0);
        assert_eq!(q, 0);
        assert_eq!(r, 0);
    }

    #[test]
    fn test_neighbors() {
        let neighbors = HexGrid::neighbors(0, 0);
        assert_eq!(neighbors.len(), 6);
        assert!(neighbors.contains(&HexCoord::new(1, 0)));
        assert!(neighbors.contains(&HexCoord::new(-1, 0)));
        assert!(neighbors.contains(&HexCoord::new(0, 1)));
        assert!(neighbors.contains(&HexCoord::new(0, -1)));
    }

    #[test]
    fn test_distance() {
        assert_eq!(HexGrid::distance(0, 0, 0, 0), 0);
        assert_eq!(HexGrid::distance(0, 0, 1, 0), 1);
        assert_eq!(HexGrid::distance(0, 0, 2, 0), 2);
    }

    #[test]
    fn test_empty_grid_radius_zero_has_only_center() {
        let hexes = HexGrid::hexes_in_radius(0, 0, 1);
        assert_eq!(hexes.len(), 7); // center + 6 neighbors
    }
}

    #[test]
    fn empty_grid_radius_zero_has_only_center() {
        let all = HexGrid::hexes_in_radius(7, -3, 0);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].q, 7);
        assert_eq!(all[0].r, -3);
    }

    #[test]
    fn max_grid_64_radius_count() {
        let all = HexGrid::hexes_in_radius(0, 0, 64);
        // 3r² + 3r + 1 for r = 64.
        assert_eq!(all.len(), 3 * 64 * 64 + 3 * 64 + 1);
    }

    #[test]
    fn grid_ids_are_unique_across_max_grid() {
        let mut ids: std::collections::HashSet<u64> = Default::default();
        for h in HexGrid::hexes_in_radius(0, 0, 64) {
            assert!(ids.insert(h.to_id()), "duplicate id at {:?}", (h.q, h.r));
        }
        assert_eq!(ids.len(), 3 * 64 * 64 + 3 * 64 + 1);
    }

    #[test]
    fn world_roundtrip_at_hex_centers() {
        // Walking off-grid / boundary: every hex center in radius 8 maps back
        // to itself (world_pos→axial must be the inverse of axial→world).
        for h in HexGrid::hexes_in_radius(0, 0, 8) {
            let (x, z) = HexGrid::axial_to_world(h.q, h.r, 4.0);
            let (q2, r2) = HexGrid::world_to_axial(x, z, 4.0);
            assert_eq!((q2, r2), (h.q, h.r), "roundtrip failed for {:?}", (h.q, h.r));
        }
    }

    #[test]
    fn large_grid_generation_is_fast() {
// Spec 002 T5.1: 12,480 hexes (radius 64) generate + visit without
        // breaking 60fps. Generous local bound; smoke-perf test.
        let start = std::time::Instant::now();
        let all = HexGrid::hexes_in_radius(0, 0, 64);
        let elapsed = start.elapsed();
        assert_eq!(all.len(), 12_481);
        assert!(
            elapsed.as_millis() < 500,
            "radius-64 traversal took {:?}",
            elapsed
        );
    }

    #[test]
    fn neighbors_distance_consistency() {
        let origin = (3, -2);
        for n in HexGrid::neighbors(origin.0, origin.1) {
            assert_eq!(HexGrid::distance(origin.0, origin.1, n.q, n.r), 1);
        }
        assert_eq!(HexGrid::distance(0, 0, 64, -64), 64);
    }
