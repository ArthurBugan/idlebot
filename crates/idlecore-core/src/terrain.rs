//! Terrain types for IdleBot hex grid.
//!
//! This is the single source of truth for terrain classification, colours,
//! and game-logic predicates.

use rand::Rng;

/// Unified terrain type — biome + game predicate + minimap colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum TerrainType {
    #[default]
    Grass,
    Forest,
    Water,
    City,
    Desert,
    Polluted,
    Tundra,
    Taiga,
    Grassland,
    TropicalRainforest,
    Mountain,
}

impl TerrainType {
    /// Create a random terrain type matching spec distribution.
    pub fn from_random<R: Rng>(rng: &mut R) -> Self {
        let roll = rng.gen_range(0..100);
        if roll < 50 {
            TerrainType::Grass
        } else if roll < 70 {
            TerrainType::Forest
        } else if roll < 78 {
            TerrainType::Water
        } else if roll < 88 {
            TerrainType::City
        } else if roll < 95 {
            TerrainType::Desert
        } else {
            TerrainType::Polluted
        }
    }

    /// Get the biome name for this terrain.
    pub fn biome_name(&self) -> &'static str {
        match self {
            TerrainType::Grass => "Grass",
            TerrainType::Forest => "Forest",
            TerrainType::Water => "Water",
            TerrainType::City => "City",
            TerrainType::Desert => "Desert",
            TerrainType::Polluted => "Polluted",
            TerrainType::Tundra => "Tundra",
            TerrainType::Taiga => "Taiga",
            TerrainType::Grassland => "Grassland",
            TerrainType::TropicalRainforest => "Tropical Rainforest",
            TerrainType::Mountain => "Mountain",
        }
    }

    /// Eco rating for this terrain.
    pub fn eco_rating(&self) -> i32 {
        match self {
            TerrainType::Grass | TerrainType::Forest => 50,
            TerrainType::Water | TerrainType::City | TerrainType::Desert => 20,
            TerrainType::Polluted => 10,
            TerrainType::Tundra | TerrainType::Taiga => 30,
            TerrainType::Grassland | TerrainType::TropicalRainforest => 40,
            TerrainType::Mountain => 15,
        }
    }

    /// Check if this terrain can be farmed on.
    pub fn is_farmable(&self) -> bool {
        matches!(self, TerrainType::Grass | TerrainType::Grassland | TerrainType::TropicalRainforest)
    }

    /// Check if this terrain allows planting.
    pub fn is_compatible_for_planting(&self) -> bool {
        matches!(self, TerrainType::Grass | TerrainType::Forest | TerrainType::City | TerrainType::Desert | TerrainType::Grassland)
    }

    /// Check if this terrain is clean (non-polluted).
    pub fn is_clean(&self) -> bool {
        *self != TerrainType::Polluted
    }

    /// Check if this terrain is walkable.
    pub fn is_walkable(&self) -> bool {
        *self != TerrainType::Water
    }

    /// Check if this terrain is a water biome.
    pub fn is_water(&self) -> bool {
        matches!(self, TerrainType::Water)
    }

    /// Get the minimap colour for this terrain (RGB, 0.0–1.0).
    pub fn minimap_color(&self) -> [f32; 3] {
        match self {
            TerrainType::Grass => [0.496, 0.792, 0.322],
            TerrainType::Forest => [0.133, 0.545, 0.133],
            TerrainType::Water => [0.255, 0.404, 0.882],
            TerrainType::City => [0.502, 0.502, 0.502],
            TerrainType::Desert => [0.953, 0.643, 0.376],
            TerrainType::Polluted => [0.294, 0.000, 0.514],
            TerrainType::Tundra => [0.9, 0.95, 1.0],
            TerrainType::Taiga => [0.2, 0.4, 0.2],
            TerrainType::Grassland => [0.6, 0.8, 0.3],
            TerrainType::TropicalRainforest => [0.1, 0.5, 0.1],
            TerrainType::Mountain => [0.5, 0.5, 0.5],
        }
    }

    /// Get the 3D renderer colour for this terrain.
    pub fn color(&self) -> crate::Color {
        let [r, g, b] = self.minimap_color();
        crate::Color::srgb(r, g, b)
    }

    /// Get the fog-of-war conceal colour (when not yet discovered).
    pub fn fog_color() -> [u8; 3] {
        [10, 12, 16]
    }
}
