//! Hex grid generation with seeded terrain distribution.
//!
//! Generates a bounded hex grid of HexTiles with deterministic terrain assignment
//! using a seeded PRNG. Uses axial coordinates bounded by distance <= radius.

use rand::Rng;
use crate::hex::HexCoord;
use crate::terrain::TerrainType;

/// A tile on the hex grid, containing coordinate and terrain data.
#[derive(Debug, Clone, Copy, Default)]
pub struct HexTile {
    pub coord: HexCoord,
    pub terrain: TerrainType,
    pub elevation: f32,
}

/// Hex grid generation and querying.
#[derive(Debug, Default, Clone)]
pub struct HexGrid {
    pub hexes: std::collections::HashMap<u64, HexTile>,
}

impl HexGrid {
    /// Generate a deterministic hex grid using the given seed and radius.
    ///
    /// For radius=100, produces approximately 12,481 hexes (area-based counting).
    pub fn generate(seed: u64, radius: i32) -> Self {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut grid = Self::default();

        for q in -radius..=radius {
            for r in -radius..=radius {
                let s = -q - r;
                // Bounded by cube distance: q^2 + r^2 + s^2 <= radius^2
                if q.abs() <= radius && r.abs() <= radius && s.abs() <= radius {
                    if (q as i64).checked_mul(q as i64)
                        .map(|sq| {
                            sq
                                + (r as i64).checked_mul(r as i64)?
                                + (s as i64).checked_mul(s as i64)?
                                <= (radius as i64).checked_mul(radius as i64)?
                        })
                        .unwrap_or(false)
                    {
                        let hex_id = (q as u64) << 32 | (r as u64);
                        let terrain = TerrainType::from_random(&mut rng);
                        grid.hexes.insert(hex_id, HexTile {
                            coord: HexCoord::new(q, r),
                            terrain,
                            elevation: rng.gen(),
                        });
                    }
                }
            }
        }

        grid
    }

    /// Get a hex tile by ID.
    pub fn get(&self, hex_id: u64) -> Option<&HexTile> {
        self.hexes.get(&hex_id)
    }

    /// Get all hex IDs in the grid.
    pub fn ids(&self) -> Vec<u64> {
        self.hexes.keys().cloned().collect()
    }

    /// Get the number of hexes.
    pub fn len(&self) -> usize {
        self.hexes.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.hexes.is_empty()
    }

    /// Return the radius of this grid.
    pub fn radius(&self) -> i32 {
        if self.hexes.is_empty() {
            return 0;
        }
        // Find max |q| and |r| across all hexes
        let mut max_q = 0i32;
        let mut max_r = 0i32;
        let mut max_s = 0i32;
        for (_, tile) in &self.hexes {
            let s = tile.coord.s;
            max_q = max_q.max(tile.coord.q.abs());
            max_r = max_r.max(tile.coord.r.abs());
            max_s = max_s.max(s.abs());
        }
        max_q.max(max_r).max(max_s)
    }
}
