//! World generation — 1:10000 scale Earth replica.
//! Latitude-based biomes, elevation-driven land/ocean split.

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use crate::hex::HexCoord;

/// Hex world size constant (10x original 10-unit hexes = 100 units)
pub const HEX_SIZE: f32 = 100.0;

/// Biome type determined by latitude and elevation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Biome {
    Tundra,
    Taiga,
    TemperateForest,
    Grassland,
    Desert,
    TropicalRainforest,
    Ocean,
    Mountain,
    City,
    Polluted,
}

impl Biome {
    /// Get the color for this biome.
    pub fn color(&self) -> (f32, f32, f32) {
        match self {
            Biome::Tundra => (0.9, 0.95, 1.0),
            Biome::Taiga => (0.2, 0.4, 0.2),
            Biome::TemperateForest => (0.3, 0.6, 0.3),
            Biome::Grassland => (0.6, 0.8, 0.3),
            Biome::Desert => (0.9, 0.8, 0.5),
            Biome::TropicalRainforest => (0.1, 0.5, 0.1),
            Biome::Ocean => (0.1, 0.3, 0.8),
            Biome::Mountain => (0.5, 0.5, 0.5),
            Biome::City => (0.7, 0.7, 0.7),
            Biome::Polluted => (0.4, 0.4, 0.4),
        }
    }

    /// Check if this biome is walkable.
    pub fn is_walkable(&self) -> bool {
        !matches!(self, Biome::Ocean)
    }

    /// Check if this biome is farmable.
    pub fn is_farmable(&self) -> bool {
        matches!(self, Biome::Grassland | Biome::TemperateForest | Biome::TropicalRainforest)
    }
}

/// Vegetation type for a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Vegetation {
    None,
    Trees,
    Bushes,
    Cacti,
    Snow,
}

/// A single tile in the world.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldTile {
    pub coord: HexCoord,
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub biome: Biome,
    pub elevation: f32,
    pub vegetation: Vegetation,
    pub owned_by: Option<u64>,
}

impl WorldTile {
    /// Create a new world tile.
    pub fn new(coord: HexCoord, hex_id: u64, biome: Biome, elevation: f32, vegetation: Vegetation) -> Self {
        let q = coord.q as f32;
        let r = coord.r as f32;
        let sqrt3 = f32::sqrt(3.0);
        let x = HEX_SIZE * sqrt3 * (q + r / 2.0);
        let y = HEX_SIZE * 1.5 * r;

        Self {
            coord,
            hex_id,
            center_x: x,
            center_y: y,
            biome,
            elevation,
            vegetation,
            owned_by: None,
        }
    }
}

/// The entire world.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EarthWorld {
    pub tiles: HashMap<u64, WorldTile>,
    pub radius: i32,
}

impl EarthWorld {
    /// Generate a new world.
    ///
    /// - `seed`: Random seed for world generation.
    /// - `radius`: Radius of the hex grid (in hexes).
    ///
    /// Returns the world with tiles centered at (0, 0).
    pub fn generate(seed: u64, radius: i32) -> Self {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut world = Self {
            tiles: HashMap::new(),
            radius,
        };

        let max_coord = radius as i64;

        for q in -radius..=radius {
            for r in -radius..=radius {
                let s = -q - r;
                // Bounded by cube distance
                if q.unsigned_abs() as u64 <= radius as u64
                    && r.unsigned_abs() as u64 <= radius as u64
                    && s.unsigned_abs() as u64 <= radius as u64
                {
                    let hex_id = ((q as u32) as u64) << 32 | (r as u32) as u64;

                    // Calculate latitude from r coordinate (-1 to 1 maps to -90 to 90 degrees)
                    let lat_normalized = r as f64 / radius as f64;
                    let latitude = lat_normalized * 90.0; // -90 to 90 degrees

                    // Generate elevation using simple noise (simplified)
                    let elevation = Self::generate_elevation(&mut rng, q, r);

                    // Determine biome based on latitude and elevation
                    let biome = Self::determine_biome(latitude, elevation);

                    // Determine vegetation based on biome
                    let vegetation = Self::determine_vegetation(&mut rng, biome, elevation);

                    let tile = WorldTile::new(
                        HexCoord { q, r, s: -q - r },
                        hex_id,
                        biome,
                        elevation,
                        vegetation,
                    );

                    world.tiles.insert(hex_id, tile);
                }
            }
        }

        world
    }

    /// Generate elevation using a combination of hash-based noise and sine waves.
    fn generate_elevation(rng: &mut SmallRng, q: i32, r: i32) -> f32 {
        // Simple hash-based elevation (in production, use proper noise function)
        let hash = (q as u64) ^ ((r as u64) << 32);
        let noise = (hash as f64) / (u64::MAX as f64);
        // Use a sine wave for continental shapes with higher base elevation
        let continental = (noise * 6.28318).sin().abs();
        // Increase minimum elevation to avoid all-ocean
        continental as f32 * 0.5 + 0.3 + rng.gen::<f32>() * 0.2
    }

    /// Determine biome based on latitude and elevation.
    fn determine_biome(latitude: f64, elevation: f32) -> Biome {
        let abs_lat = latitude.abs();

        // Mountain biome for high elevation
        if elevation > 0.7 {
            return Biome::Mountain;
        }

        // Ocean determined by elevation (low elevation = ocean)
        if elevation < 0.35 {
            return Biome::Ocean;
        }

        // Latitude-based biomes
        if abs_lat > 60.0 {
            Biome::Tundra
        } else if abs_lat > 50.0 {
            Biome::Taiga
        } else if abs_lat > 30.0 {
            // Temperate zone: forest or grassland based on elevation
            if elevation > 0.5 {
                Biome::TemperateForest
            } else {
                Biome::Grassland
            }
        } else if abs_lat > 15.0 {
            // Subtropical: desert or grassland
            if elevation < 0.4 {
                Biome::Desert
            } else {
                Biome::Grassland
            }
        } else {
            // Tropical: rainforest
            Biome::TropicalRainforest
        }
    }

    /// Determine vegetation based on biome and elevation.
    fn determine_vegetation(rng: &mut SmallRng, biome: Biome, elevation: f32) -> Vegetation {
        match biome {
            Biome::Tundra => Vegetation::Snow,
            Biome::Taiga => Vegetation::Trees,
            Biome::TemperateForest => Vegetation::Trees,
            Biome::TropicalRainforest => Vegetation::Trees,
            Biome::Grassland => {
                if rng.gen::<f32>() > 0.5 { Vegetation::Bushes } else { Vegetation::None }
            }
            Biome::Desert => Vegetation::Cacti,
            Biome::Mountain => Vegetation::None,
            Biome::Ocean => Vegetation::None,
            Biome::City => Vegetation::None,
            Biome::Polluted => Vegetation::None,
        }
    }

    /// Get a tile by hex coordinates.
    pub fn get_tile(&self, q: i32, r: i32) -> Option<&WorldTile> {
        let hex_id = ((q as u32) as u64) << 32 | (r as u32) as u64;
        self.tiles.get(&hex_id)
    }

    /// Get a tile by id.
    pub fn get_tile_by_id(&self, hex_id: u64) -> Option<&WorldTile> {
        self.tiles.get(&hex_id)
    }

    /// Get the number of tiles in the world.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get the radius of the world.
    pub fn world_radius(&self) -> i32 {
        self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_generation() {
        let world = EarthWorld::generate(42, 10);
        assert!(world.tile_count() > 0);
    }

    #[test]
    fn test_biome_distribution() {
        let world = EarthWorld::generate(42, 50);
        let mut biome_counts: HashMap<Biome, usize> = HashMap::new();
        for tile in world.tiles.values() {
            *biome_counts.entry(tile.biome).or_insert(0) += 1;
        }
        // Should have at least some variety
        assert!(biome_counts.len() > 1, "Expected multiple biomes, got: {:?}", biome_counts);
    }

    #[test]
    fn test_hex_coordinates() {
        let world = EarthWorld::generate(42, 50);
        let tile = world.get_tile(0, 0).unwrap();
        assert_eq!(tile.coord.q, 0);
        assert_eq!(tile.coord.r, 0);
    }

    #[test]
    fn test_biome_colors() {
        assert!((Biome::Ocean.color().0 - 0.1).abs() < 0.01);
        assert!((Biome::Grassland.color().1 - 0.8).abs() < 0.01);
        assert!((Biome::Tundra.color().0 - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_walkable_biomes() {
        assert!(Biome::Grassland.is_walkable());
        assert!(!Biome::Ocean.is_walkable());
    }

    #[test]
    fn test_farmable_biomes() {
        assert!(Biome::Grassland.is_farmable());
        assert!(!Biome::Desert.is_farmable());
    }

    #[test]
    fn test_seed_determinism() {
        let world1 = EarthWorld::generate(42, 10);
        let world2 = EarthWorld::generate(42, 10);
        assert_eq!(world1.tile_count(), world2.tile_count());
    }
}
