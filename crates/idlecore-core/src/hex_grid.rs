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
    /// Uses floating point for rounding.
    pub fn world_to_axial(x: f32, z: f32, size: f32) -> (i32, i32) {
        let q = (f32::sqrt(3.0) / 3.0 * x - 1.0 / 3.0 * z) / size;
        let r = 2.0 / 3.0 * z / size;
        let q_round = q.round() as i32;
        let r_round = r.round() as i32;
        let s_round = (-q - r).round() as i32;
        let q_frac = (q_round as f32 - q).abs();
        let r_frac = (r_round as f32 - r).abs();
        let s_frac = (s_round as f32 + q_round as f32 + r_round as f32).abs();

        if q_frac > r_frac && q_frac > s_frac {
            (-r_round, -q_round)
        } else if r_frac > s_frac {
            (-q_round, -r_round)
        } else {
            (q_round, r_round)
        }
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
        (q1 - q2).unsigned_abs() as i32
            + (r1 - r2).unsigned_abs() as i32
            + (s1 - s2).unsigned_abs() as i32
    }

    /// Get all hexes in a radius around a center hex.
    pub fn hexes_in_radius(center_q: i32, center_r: i32, radius: i32) -> Vec<HexCoord> {
        let mut result = Vec::new();
        for q in -radius..=radius {
            let r_start = (-radius - q).max(-radius);
            let r_end = (-radius - q).min(radius);
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
    fn test_hexes_in_radius() {
        let hexes = HexGrid::hexes_in_radius(0, 0, 1);
        assert_eq!(hexes.len(), 7); // center + 6 neighbors
    }
}
