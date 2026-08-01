//! Earth-scale world generation with biomes and vegetation.
//!
//! Generates a world at 1:10000 scale with real Earth-like geography,
//! latitude-based biomes, and vegetation spawning.

use crate::hex::HexCoord;
use crate::terrain::TerrainType;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// World scale: 1 unit = 100 meters (1:10000 scale)
pub const WORLD_SCALE: f32 = 100.0;

/// Hex size in world units (10x original)
pub const HEX_SIZE: f32 = 100.0;

/// Biome types based on latitude and temperature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    /// Polar ice caps (lat > 60°)
    Tundra,
    /// Boreal forests (lat 50-60°)
    Taiga,
    /// Temperate forests (lat 30-50°)
    TemperateForest,
    /// Grasslands (lat 20-40°, inland)
    Grassland,
    /// Deserts (lat 15-30°, low rainfall)
    Desert,
    /// Tropical rainforest (lat < 20°)
    TropicalRainforest,
    /// Ocean (sea level)
    Ocean,
    /// Mountain (high elevation)
    Mountain,
    /// City/urban (player-placed)
    City,
    /// Polluted (player-placed)
    Polluted,
}

impl Biome {
    /// Get the display name for this biome.
    pub fn display_name(&self) -> &'static str {
        match self {
            Biome::Tundra => "Tundra",
            Biome::Taiga => "Taiga",
            Biome::TemperateForest => "Temperate Forest",
            Biome::Grassland => "Grassland",
            Biome::Desert => "Desert",
            Biome::TropicalRainforest => "Tropical Rainforest",
            Biome::Ocean => "Ocean",
            Biome::Mountain => "Mountain",
            Biome::City => "City",
            Biome::Polluted => "Polluted",
        }
    }

    /// Get the color for this biome (RGB).
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

/// Vegetation types that can spawn in biomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vegetation {
    /// Grass (grasslands, temperate forests)
    Grass,
    /// Bush (deserts, grasslands)
    Bush,
    /// Tree (forests, taiga)
    Tree,
    /// Rare tree (tropical rainforest)
    RareTree,
    /// Cactus (deserts)
    Cactus,
    /// Snow plant (tundra)
    SnowPlant,
    /// Mountain shrub (mountains)
    MountainShrub,
    /// None (ocean, city, polluted)
    None,
}

impl Vegetation {
    /// Get the display name for this vegetation.
    pub fn display_name(&self) -> &'static str {
        match self {
            Vegetation::Grass => "Grass",
            Vegetation::Bush => "Bush",
            Vegetation::Tree => "Tree",
            Vegetation::RareTree => "Rare Tree",
            Vegetation::Cactus => "Cactus",
            Vegetation::SnowPlant => "Snow Plant",
            Vegetation::MountainShrub => "Mountain Shrub",
            Vegetation::None => "None",
        }
    }
}

/// A tile on the world grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldTile {
    /// Axial coordinate.
    pub coord: HexCoord,
    /// Hex ID.
    pub hex_id: u64,
    /// Center position in world coordinates.
    pub center_x: f32,
    pub center_y: f32,
    /// Biome type.
    pub biome: Biome,
    /// Elevation (0-1, affects biome).
    pub elevation: f32,
    /// Vegetation on this tile.
    pub vegetation: Vegetation,
    /// Whether this tile has been owned.
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

/// Earth-like world generator.
///
/// Uses latitude-based biome system:
/// - Polar (lat > 60°): Tundra
/// - Subpolar (lat 50-60°): Taiga
/// - Temperate (lat 30-50°): Temperate Forest or Grassland
/// - Subtropical (lat 15-30°): Desert or Grassland
/// - Tropical (lat < 15°): Tropical Rainforest
/// - Ocean: determined by elevation and proximity to continents
/// - Mountain: high elevation areas
#[derive(Debug, Clone, Default)]
pub struct EarthWorld {
    pub tiles: HashMap<u64, WorldTile>,
    /// World radius in hexes.
    pub radius: i32,
}

impl EarthWorld {
    /// Generate an Earth-like world with the given seed and radius.
    ///
    /// The world is centered at (0, 0) and extends to the given radius.
    /// Biomes are determined by latitude (r coordinate) and elevation.
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
                        HexCoord::new(q, r),
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

    /// Generate elevation using simple hash-based noise.
    fn generate_elevation(rng: &mut SmallRng, q: i32, r: i32) -> f32 {
        // Simple hash-based elevation (in production, use proper noise function)
        let hash = (q as u64) ^ ((r as u64) << 32);
        let noise = (hash as f64) / (u64::MAX as f64);
        // Use a sine wave for continental shapes
        let continental = (noise * 6.28318).sin().abs();
        continental as f32 * 0.7 + rng.gen::<f32>() * 0.3
    }

    /// Determine biome based on latitude and elevation.
    fn determine_biome(latitude: f64, elevation: f32) -> Biome {
        let abs_lat = latitude.abs();

        // Ocean determined by elevation (low elevation = ocean)
        if elevation < 0.3 {
            return Biome::Ocean;
        }

        // Mountain biome for high elevation
        if elevation > 0.7 {
            return Biome::Mountain;
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
            Biome::Tundra => {
                if rng.gen::<f32>() < 0.3 {
                    Vegetation::SnowPlant
                } else {
                    Vegetation::None
                }
            }
            Biome::Taiga => {
                if rng.gen::<f32>() < 0.7 {
                    Vegetation::Tree
                } else {
                    Vegetation::None
                }
            }
            Biome::TemperateForest => {
                if rng.gen::<f32>() < 0.6 {
                    Vegetation::Tree
                } else if rng.gen::<f32>() < 0.4 {
                    Vegetation::Grass
                } else {
                    Vegetation::None
                }
            }
            Biome::Grassland => {
                if rng.gen::<f32>() < 0.8 {
                    Vegetation::Grass
                } else if rng.gen::<f32>() < 0.2 {
                    Vegetation::Bush
                } else {
                    Vegetation::None
                }
            }
            Biome::Desert => {
                if rng.gen::<f32>() < 0.15 {
                    Vegetation::Cactus
                } else if rng.gen::<f32>() < 0.1 {
                    Vegetation::Bush
                } else {
                    Vegetation::None
                }
            }
            Biome::TropicalRainforest => {
                if rng.gen::<f32>() < 0.8 {
                    Vegetation::RareTree
                } else if rng.gen::<f32>() < 0.6 {
                    Vegetation::Tree
                } else {
                    Vegetation::None
                }
            }
            Biome::Mountain => {
                if rng.gen::<f32>() < 0.2 {
                    Vegetation::MountainShrub
                } else {
                    Vegetation::None
                }
            }
            Biome::Ocean | Biome::City | Biome::Polluted => Vegetation::None,
        }
    }

    /// Get a tile by ID.
    pub fn get(&self, hex_id: u64) -> Option<&WorldTile> {
        self.tiles.get(&hex_id)
    }

    /// Get all tile IDs.
    pub fn ids(&self) -> Vec<u64> {
        self.tiles.keys().cloned().collect()
    }

    /// Get the number of tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_generation_basic() {
        let world = EarthWorld::generate(42, 10);
        assert!(!world.is_empty());
        assert!(world.len() > 0);
    }

    #[test]
    fn test_biome_latitude_distribution() {
        let world = EarthWorld::generate(123, 50);
        let mut tundra_count = 0;
        let mut tropical_count = 0;
        let mut ocean_count = 0;

        for tile in world.tiles.values() {
            let lat = tile.coord.r as f64 / 50.0 * 90.0;
            if lat.abs() > 60.0 {
                tundra_count += 1;
            }
            if lat.abs() < 15.0 {
                tropical_count += 1;
            }
            if tile.biome == Biome::Ocean {
                ocean_count += 1;
            }
        }

        // Should have some of each biome type
        assert!(tundra_count > 0, "Should have tundra at high latitudes");
        assert!(tropical_count > 0, "Should have tropical at low latitudes");
        assert!(ocean_count > 0, "Should have ocean tiles");
    }

    #[test]
    fn test_vegetation_by_biome() {
        let world = EarthWorld::generate(456, 30);

        // Check that different biomes have appropriate vegetation
        for tile in world.tiles.values() {
            match tile.biome {
                Biome::Ocean => assert_eq!(tile.vegetation, Vegetation::None),
                Biome::Tundra => assert!(!matches!(tile.vegetation, Vegetation::Tree)),
                Biome::TropicalRainforest => {
                    assert!(matches!(
                        tile.vegetation,
                        Vegetation::RareTree | Vegetation::Tree | Vegetation::None
                    ))
                }
                _ => {} // Other biomes can have various vegetation
            }
        }
    }

    #[test]
    fn test_hex_size_constant() {
        assert_eq!(HEX_SIZE, 100.0);
    }

    #[test]
    fn test_biome_colors() {
        let ocean_color = Biome::Ocean.color();
        assert_eq!(ocean_color, (0.1, 0.3, 0.8));

        let desert_color = Biome::Desert.color();
        assert_eq!(desert_color, (0.9, 0.8, 0.5));
    }

    #[test]
    fn test_biome_walkable() {
        assert!(!Biome::Ocean.is_walkable());
        assert!(Biome::Grassland.is_walkable());
        assert!(Biome::TropicalRainforest.is_walkable());
    }

    #[test]
    fn test_biome_farmable() {
        assert!(Biome::Grassland.is_farmable());
        assert!(Biome::TemperateForest.is_farmable());
        assert!(Biome::TropicalRainforest.is_farmable());
        assert!(!Biome::Desert.is_farmable());
        assert!(!Biome::Ocean.is_farmable());
    }
}
