//! Terrain types for IdleBot hex grid.

use rand::Rng;

/// Terrain type based on spec probabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerrainType {
    #[default]
    Grass,
    Forest,
    Water,
    City,
    Desert,
    Polluted,
}

impl TerrainType {
    /// Create a random terrain type matching spec distribution.
    /// Probabilities: Grass 50%, Forest 20%, Water 8%, City 10%, Desert 7%, Polluted 5%
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
}

/// Get eco rating for a terrain type (spec 3.2)
pub fn eco_rating(terrain: &TerrainType) -> i32 {
    match terrain {
        TerrainType::Grass | TerrainType::Forest => 50,
        TerrainType::Water | TerrainType::City | TerrainType::Desert => 20,
        TerrainType::Polluted => 10,
    }
}

/// Check if terrain is farmable (only grass)
pub fn is_farmable(terrain: &TerrainType) -> bool {
    matches!(terrain, TerrainType::Grass)
}

/// Check if terrain is compatible for planting
pub fn is_compatible_for_planting(terrain: &TerrainType) -> bool {
    matches!(
        terrain,
        TerrainType::Grass
            | TerrainType::Forest
            | TerrainType::City
            | TerrainType::Desert
    )
}

/// Check if terrain is clean (non-polluted)
pub fn is_clean(terrain: &TerrainType) -> bool {
    !matches!(terrain, TerrainType::Polluted)
}
