//! Hex math for IdleBot hex grid.
//! Uses axial coordinates (q, r, s) where q + r + s = 0.
//! Hex radius: 10.0 meters, Map radius: 64 hexes.

use bevy::prelude::*;

/// Hexagon defined by axial coordinates (q, r, s) where q + r + s = 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
    pub s: i32,
}

impl Hex {
    /// Create a hex from axial q,r (s = -q-r).
    pub fn new(q: i32, r: i32) -> Self {
        let s = -q - r;
        Self { q, r, s }
    }

    /// Get the center position of this hex in 3D space.
    /// Flat-top hexes: x = sqrt(3) * R * (q + r/2), y = 1.5 * R * r
    pub fn center(&self, hex_radius: f32) -> Vec3 {
        let sqrt3 = f32::sqrt(3.0);
        let x = hex_radius * sqrt3 * (self.q as f32 + self.r as f32 / 2.0);
        let y = hex_radius * 1.5 * self.r as f32;
        Vec3::new(x, y, 0.0)
    }

    /// Get the corner points of this hex (6 corners, flat-top orientation)
    pub fn corners(&self, hex_radius: f32) -> [Vec3; 6] {
        let center = self.center(hex_radius);
        let sqrt3 = f32::sqrt(3.0);
        let mut corners = [Vec3::ZERO; 6];
        for i in 0..6 {
            let angle = std::f32::consts::FRAC_PI_3 * i as f32;
            corners[i] = Vec3::new(
                center.x + hex_radius * angle.cos(),
                center.y + hex_radius * angle.sin(),
                center.z,
            );
        }
        corners
    }

    /// Distance between two hexes (in hex units)
    pub fn distance(&self, other: &Hex) -> i32 {
        (self.q - other.q).abs() + (self.r - other.r).abs() + (self.s - other.s).abs() / 2
    }

    /// Get neighbor in a given direction (0-5, where 0 is +q)
    pub fn neighbor(&self, direction: i32) -> Hex {
        let directions = [
            (1, -1, 0),  // +q
            (0, 1, -1),  // +r
            (-1, 0, 1),  // +s
            (-1, 1, 0),  // -q
            (0, -1, 1),  // -r
            (1, 0, -1),  // -s
        ];
        let (dq, dr, _ds) = directions[direction as usize];
        Hex::new(self.q + dq, self.r + dr)
    }

    /// Convert axial (q, r) to cube coordinates (x, y, z)
    pub fn to_cube(&self) -> (i32, i32, i32) {
        (self.q, self.r, self.s)
    }

    /// Create from cube coordinates
    pub fn from_cube(x: i32, y: i32, z: i32) -> Self {
        // Round to nearest cube coordinate
        let q = (x as f32 + y as f32 + z as f32) / 3.0;
        let r = (y as f32 + z as f32 - 2.0 * x as f32) / 3.0;
        let s = (z as f32 + x as f32 - 2.0 * y as f32) / 3.0;
        
        let q_round = q.round() as i32;
        let r_round = r.round() as i32;
        let s_round = s.round() as i32;
        
        let q_diff = (q_round as f32 - q).abs();
        let r_diff = (r_round as f32 - r).abs();
        let s_diff = (s_round as f32 - s).abs();
        
        if q_diff > r_diff && q_diff > s_diff {
            Self::new(-r_round - s_round, r_round)
        } else if r_diff > s_diff {
            Self::new(r_round, -q_round - r_round)
        } else {
            Self::new(s_round, -s_round - q_round)
        }
    }
}
