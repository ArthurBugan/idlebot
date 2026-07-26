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

    /// Calculate a unique ID for this hex as u64.
    /// Format: (q << 32) | r
    pub fn hex_id(&self) -> u64 {
        ((self.q as u64) << 32) | (self.r as u64)
    }

    /// Return all 6 neighbors of this hex in axial coordinates.
    pub fn neighbors(&self) -> [Hex; 6] {
        let result = [
            Hex {
                q: self.q + 1,
                r: self.r,
                s: self.s - 1,
            },
            Hex {
                q: self.q,
                r: self.r + 1,
                s: self.s - 1,
            },
            Hex {
                q: self.q - 1,
                r: self.r + 1,
                s: self.s,
            },
            Hex {
                q: self.q - 1,
                r: self.r,
                s: self.s + 1,
            },
            Hex {
                q: self.q,
                r: self.r - 1,
                s: self.s + 1,
            },
            Hex {
                q: self.q + 1,
                r: self.r - 1,
                s: self.s,
            },
        ];
        result
    }

    /// Convert to flat coordinates (dx, dy) for lookups.
    pub fn to_flat(&self) -> (i64, i64) {
        (self.q as i64, self.r as i64)
    }

    /// Check if this hex is adjacent to another hex.
    pub fn is_adjacent_to(&self, other: &Hex) -> bool {
        for n in self.neighbors() {
            if n == *other {
                return true;
            }
        }
        false
    }
}

/// State struct tracking all generated hexes and their terrain.
pub struct HexWorld {
    pub hexes: std::collections::HashMap<u64, HexData>,
    pub flat_to_hex_id: std::collections::HashMap<(i64, i64), u64>,
    pub hex_radius: f32,
    pub map_radius: i32,
}

/// Data for a single hex with its terrain.
#[derive(Debug, Clone)]
pub struct HexData {
    pub q: i32,
    pub r: i32,
    pub s: i32,
    pub center: Vec3,
    pub terrain: crate::terrain::TerrainType,
}

impl HexWorld {
    /// Generate the world with a given seed.
    pub fn generate(seed: u64, map_radius: i32, hex_radius: f32) -> Self {
        let mut hexes = std::collections::HashMap::new();
        let mut flat_to_hex_id = std::collections::HashMap::new();

        for q in -map_radius..=map_radius {
            for r in -map_radius..=map_radius {
                let s = -q - r;
                if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                    let h = Hex::new(q, r);
                    let hex_id = h.hex_id();
                    flat_to_hex_id.insert((q as i64, r as i64), hex_id);

                    // Deterministic terrain from seed + position
                    let terrain_seed = (q as u64)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add((r as u64).wrapping_mul(1442695040888963407));
                    let val = ((terrain_seed >> 33) ^ terrain_seed) as f32 / u32::MAX as f32;

                    hexes.insert(hex_id, HexData {
                        q,
                        r,
                        s: -q - r,
                        center: Vec3::ZERO,
                        terrain: if val < 0.50 {
                            crate::terrain::TerrainType::Grass
                        } else if val < 0.70 {
                            crate::terrain::TerrainType::Forest
                        } else if val < 0.78 {
                            crate::terrain::TerrainType::Water
                        } else if val < 0.88 {
                            crate::terrain::TerrainType::City
                        } else if val < 0.95 {
                            crate::terrain::TerrainType::Desert
                        } else {
                            crate::terrain::TerrainType::Polluted
                        },
                    });
                }
            }
        }

        // Compute centers from flat coords
        let sqrt3 = f32::sqrt(3.0);
        for (_, hex_data) in hexes.iter_mut() {
            hex_data.center = Vec3::new(
                hex_radius * sqrt3 * (hex_data.q as f32 + hex_data.r as f32 / 2.0),
                hex_radius * 1.5 * hex_data.r as f32,
                0.0,
            );
        }

        HexWorld {
            hexes,
            flat_to_hex_id,
            hex_radius,
            map_radius,
        }
    }

    pub fn flat_to_id(&self, flat: (i64, i64)) -> Option<u64> {
        self.flat_to_hex_id.get(&flat).copied()
    }

    pub fn is_in_world(&self, flat: (i64, i64)) -> bool {
        self.flat_to_hex_id.contains_key(&flat)
    }

    pub fn hex_to_flat(&self, hex_id: u64) -> Option<(i64, i64)> {
        for (flat, id) in &self.flat_to_hex_id {
            if *id == hex_id {
                return Some(*flat);
            }
        }
        None
    }
}
