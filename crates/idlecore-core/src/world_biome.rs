//! Biome definitions and weighted/interpolated biome influence (§9, §10, FR-02).
//!
//! Biomes are thin definitions referenced by id from [`HexCell`](crate::world_gen::HexCell).
//! Rendering and gameplay look up biome behavior here. Biome boundaries are
//! never hard; `biome_influences` returns fractional weights across nearby
//! biomes so renderers can produce natural transitions.

use crate::terrain::TerrainType;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// A biome definition — data only, no runtime objects.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BiomeDefinition {
    pub id: u16,
    pub name: &'static str,
    pub base_terrain: TerrainType,
    /// Temperature range [-1, 1] this biome prefers.
    pub temp_min: f32,
    pub temp_max: f32,
    /// Moisture range [0, 1] this biome prefers.
    pub moisture_min: f32,
    pub moisture_max: f32,
    /// Biome base color (RGB 0-1).
    pub color: [f32; 3],
    /// Vegetation density 0..1 (decoration only in active chunks).
    pub vegetation_density: f32,
}

pub const BIOME_TUNDRA: u16 = 0;
pub const BIOME_TAIGA: u16 = 1;
pub const BIOME_GRASSLAND: u16 = 2;
pub const BIOME_TEMPERATE_FOREST: u16 = 3;
pub const BIOME_TEMPERATE_RAINFOREST: u16 = 4;
pub const BIOME_DESERT: u16 = 5;
pub const BIOME_SAVANNA: u16 = 6;
pub const BIOME_SUBTROPICAL_FOREST: u16 = 7;
pub const BIOME_TROPICAL_RAINFOREST: u16 = 8;
pub const BIOME_SHRUBLAND: u16 = 9;
pub const BIOME_SWAMP: u16 = 10;

/// The canonical biome registry. Offset 0.x to keep ids 1-based (0 = water).
static BIOME_REGISTRY: LazyLock<Vec<BiomeDefinition>> = LazyLock::new(|| {
    vec![
        BiomeDefinition { id: BIOME_TUNDRA, name: "Tundra", base_terrain: TerrainType::Tundra, temp_min: -1.0, temp_max: -0.3, moisture_min: 0.0, moisture_max: 1.0, color: [0.9, 0.95, 1.0], vegetation_density: 0.0 },
        BiomeDefinition { id: BIOME_TAIGA, name: "Taiga", base_terrain: TerrainType::Taiga, temp_min: -0.4, temp_max: 0.05, moisture_min: 0.3, moisture_max: 1.0, color: [0.2, 0.4, 0.2], vegetation_density: 0.5 },
        BiomeDefinition { id: BIOME_GRASSLAND, name: "Grassland", base_terrain: TerrainType::Grassland, temp_min: 0.0, temp_max: 0.5, moisture_min: 0.2, moisture_max: 0.6, color: [0.6, 0.8, 0.3], vegetation_density: 0.3 },
        BiomeDefinition { id: BIOME_TEMPERATE_FOREST, name: "Temperate Forest", base_terrain: TerrainType::Forest, temp_min: 0.0, temp_max: 0.5, moisture_min: 0.4, moisture_max: 0.8, color: [0.133, 0.545, 0.133], vegetation_density: 0.7 },
        BiomeDefinition { id: BIOME_TEMPERATE_RAINFOREST, name: "Temperate Rainforest", base_terrain: TerrainType::Forest, temp_min: 0.1, temp_max: 0.5, moisture_min: 0.7, moisture_max: 1.0, color: [0.1, 0.5, 0.1], vegetation_density: 0.9 },
        BiomeDefinition { id: BIOME_DESERT, name: "Desert", base_terrain: TerrainType::Desert, temp_min: 0.4, temp_max: 1.0, moisture_min: 0.0, moisture_max: 0.2, color: [0.953, 0.643, 0.376], vegetation_density: 0.0 },
        BiomeDefinition { id: BIOME_SAVANNA, name: "Savanna", base_terrain: TerrainType::Grass, temp_min: 0.5, temp_max: 1.0, moisture_min: 0.1, moisture_max: 0.4, color: [0.8, 0.7, 0.3], vegetation_density: 0.2 },
        BiomeDefinition { id: BIOME_SUBTROPICAL_FOREST, name: "Subtropical Forest", base_terrain: TerrainType::Forest, temp_min: 0.4, temp_max: 0.7, moisture_min: 0.4, moisture_max: 0.9, color: [0.2, 0.5, 0.2], vegetation_density: 0.8 },
        BiomeDefinition { id: BIOME_TROPICAL_RAINFOREST, name: "Tropical Rainforest", base_terrain: TerrainType::TropicalRainforest, temp_min: 0.6, temp_max: 1.0, moisture_min: 0.5, moisture_max: 1.0, color: [0.1, 0.5, 0.1], vegetation_density: 0.95 },
        BiomeDefinition { id: BIOME_SHRUBLAND, name: "Shrubland", base_terrain: TerrainType::Grassland, temp_min: 0.0, temp_max: 0.6, moisture_min: 0.15, moisture_max: 0.4, color: [0.7, 0.6, 0.4], vegetation_density: 0.4 },
        BiomeDefinition { id: BIOME_SWAMP, name: "Swamp", base_terrain: TerrainType::Forest, temp_min: 0.1, temp_max: 0.6, moisture_min: 0.7, moisture_max: 1.0, color: [0.2, 0.3, 0.15], vegetation_density: 0.6 },
    ]
});

/// The canonical biome registry. Offset 0.x to keep ids 1-based (0 = water).
pub fn biome_registry() -> &'static [BiomeDefinition] {
    &BIOME_REGISTRY
}

/// Look up a biome definition by id.
pub fn biome_definition(id: u16) -> Option<&'static BiomeDefinition> {
    BIOME_REGISTRY.iter().find(|b| b.id == id)
}

/// Owned lookup helper for code that can't take a static reference.
pub fn biome_definition_owned(id: u16) -> Option<BiomeDefinition> {
    BIOME_REGISTRY.iter().find(|b| b.id == id).copied()
}

/// A weighted biome sample: fraction of each biome influencing a cell.
/// Fraction weights sum to ≤ 1.0; missing weight is the "unassigned" remainder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BiomeMixSample {
    pub ids: Vec<u16>,
    pub weights: Vec<f32>,
}

/// Compute weighted biome influences for a (temperature, moisture) pair.
///
/// Instead of a hard single classification, this returns the nearest 2-3
/// biomes with interpolated weights so renderers get natural gradients.
pub fn biome_influences(temperature: f32, moisture: f32) -> Vec<(u16, f32)> {
    let biomes = biome_registry();
    let mut scored: Vec<(f32, u16)> = biomes
        .iter()
        .map(|b| {
            // Distance in (temp, moisture) normalized by each biome's range.
            let t_center = (b.temp_min + b.temp_max) * 0.5;
            let m_center = (b.moisture_min + b.moisture_max) * 0.5;
            let t_half = (b.temp_max - b.temp_min).max(0.001) * 0.5;
            let m_half = (b.moisture_max - b.moisture_min).max(0.001) * 0.5;
            let t_norm = ((temperature - t_center) / t_half).abs();
            let m_norm = ((moisture - m_center) / m_half).abs();
            // Manhattan distance — weight falls off smoothly.
            let dist = t_norm + m_norm;
            (1.0 / (1.0 + dist * dist), b.id)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let total: f32 = scored.iter().take(3).map(|(w, _)| w).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    // Take top 3, normalize to sum = 1.0 for a clean blend.
    scored[..3]
        .iter()
        .map(|(w, id)| (*id, w / total))
        .collect()
}

/// Resolve an authoritative (single) biome id from an influence list —
/// used by gameplay systems that need one classification.
pub fn primary_biome(influences: &[(u16, f32)]) -> u16 {
    influences
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(id, _)| *id)
        .unwrap_or(0)
}

/// Get the blend color for a weighted mix (for renderer / minimap blending).
pub fn mixed_color(influences: &[(u16, f32)]) -> [f32; 3] {
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    for (id, w) in influences {
        if let Some(def) = biome_definition(*id) {
            r += def.color[0] * w;
            g += def.color[1] * w;
            b += def.color[2] * w;
        }
    }
    [r, g, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_expected_biomes() {
        let reg = biome_registry();
        assert_eq!(reg.len(), 11);
        assert!(reg.iter().all(|b| biome_definition(b.id).is_some()));
    }

    #[test]
    fn biome_influences_weights_sum_to_one() {
        let influences = biome_influences(0.4, 0.5);
        let sum: f32 = influences.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < 1e-3, "sum={}", sum);
    }

    #[test]
    fn primary_biome_is_definitive() {
        let influences = biome_influences(-0.2, 0.8);
        let primary = primary_biome(&influences);
        assert!(primary != 0);
    }

    #[test]
    fn mixed_color_is_finite() {
        let influences = biome_influences(0.2, 0.3);
        let color = mixed_color(&influences);
        assert!(color[0].is_finite() && color[1].is_finite() && color[2].is_finite());
    }

    #[test]
    fn desert_in_hot_arid() {
        let influences = biome_influences(0.9, 0.02);
        let primary = primary_biome(&influences);
        assert_eq!(primary, BIOME_DESERT);
    }
}