//! Assets — Asset loading infrastructure and procedural placeholder models.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Asset handle for loaded glTF models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetHandle {
    pub path: String,
    pub loaded: bool,
    pub entity: Option<Entity>,
}

/// Terrain material colors matching spec
#[derive(Debug, Clone, Copy)]
pub struct TerrainColors {
    pub grass: Color,
    pub forest: Color,
    pub water: Color,
    pub city: Color,
    pub desert: Color,
    pub polluted: Color,
    pub tundra: Color,
    pub mountain: Color,
}

impl TerrainColors {
    /// Get default terrain colors matching spec
    pub fn default_colors() -> Self {
        Self {
            grass: Color::srgb(0.49, 0.78, 0.31),  // #7EC850
            forest: Color::srgb(0.13, 0.55, 0.13),  // #228B22
            water: Color::srgb(0.1, 0.3, 0.8),
            city: Color::srgb(0.7, 0.7, 0.7),
            desert: Color::srgb(0.9, 0.8, 0.5),
            polluted: Color::srgb(0.3, 0.3, 0.3),
            tundra: Color::srgb(0.9, 0.95, 1.0),
            mountain: Color::srgb(0.5, 0.5, 0.5),
        }
    }
}

/// Vehicle type for asset loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleAssetType {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

/// Cosmetic type for asset loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CosmeticAssetType {
    Hat,
    Aura,
    Trail,
}

/// Plant type for asset loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantAssetType {
    Wheat,
    Tree,
    RareHerb,
}

/// Asset manager — tracks loaded assets
pub struct AssetManager {
    handles: Vec<AssetHandle>,
    terrain_colors: TerrainColors,
}

impl AssetManager {
    /// Create a new asset manager with placeholder materials
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
            terrain_colors: TerrainColors::default_colors(),
        }
    }

    /// Get terrain colors
    pub fn terrain_colors(&self) -> &TerrainColors {
        &self.terrain_colors
    }

    /// Load vehicle assets (placeholder)
    pub fn load_vehicle_assets(&mut self) {
        let vehicles = [
            "vehicles/bicycle.glb",
            "vehicles/scooter.glb",
            "vehicles/motorcycle.glb",
            "vehicles/boat.glb",
            "vehicles/airplane.glb",
        ];
        
        for path in vehicles {
            self.handles.push(AssetHandle {
                path: path.to_string(),
                loaded: false,
                entity: None,
            });
        }
    }

    /// Load cosmetic assets (placeholder)
    pub fn load_cosmetic_assets(&mut self) {
        let cosmetics = [
            "cosmetics/hat_basic.glb",
            "cosmetics/hat_cool.glb",
            "cosmetics/aura_fire.glb",
            "cosmetics/aura_ice.glb",
            "cosmetics/trail_stars.glb",
            "cosmetics/trail_rainbow.glb",
        ];
        
        for path in cosmetics {
            self.handles.push(AssetHandle {
                path: path.to_string(),
                loaded: false,
                entity: None,
            });
        }
    }

    /// Load plant assets (placeholder)
    pub fn load_plant_assets(&mut self) {
        let plants = [
            "plants/wheat.glb",
            "plants/tree.glb",
            "plants/rare_herb.glb",
        ];
        
        for path in plants {
            self.handles.push(AssetHandle {
                path: path.to_string(),
                loaded: false,
                entity: None,
            });
        }
    }

    /// Mark an asset as loaded
    pub fn mark_loaded(&mut self, path: &str, entity: Entity) {
        if let Some(handle) = self.handles.iter_mut().find(|h| h.path == path) {
            handle.loaded = true;
            handle.entity = Some(entity);
        }
    }

    /// Get loaded asset entity
    pub fn get_entity(&self, path: &str) -> Option<Entity> {
        self.handles.iter().find(|h| h.path == path).and_then(|h| h.entity)
    }

    /// Get number of loaded assets
    pub fn loaded_count(&self) -> usize {
        self.handles.iter().filter(|h| h.loaded).count()
    }

    /// Get total number of assets
    pub fn total_count(&self) -> usize {
        self.handles.len()
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_colors_default() {
        let colors = TerrainColors::default_colors();
        // Just verify colors exist and are valid
        let _ = colors.grass;
        let _ = colors.forest;
        let _ = colors.water;
    }

    #[test]
    fn test_asset_manager_new() {
        let manager = AssetManager::new();
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_load_vehicle_assets() {
        let mut manager = AssetManager::new();
        manager.load_vehicle_assets();
        assert_eq!(manager.total_count(), 5);
    }

    #[test]
    fn test_load_cosmetic_assets() {
        let mut manager = AssetManager::new();
        manager.load_cosmetic_assets();
        assert_eq!(manager.total_count(), 6);
    }

    #[test]
    fn test_load_plant_assets() {
        let mut manager = AssetManager::new();
        manager.load_plant_assets();
        assert_eq!(manager.total_count(), 3);
    }

    #[test]
    fn test_mark_loaded() {
        let mut manager = AssetManager::new();
        manager.load_vehicle_assets();
        // Create a dummy entity for testing
        let entity = Entity::from_raw_u32(1).unwrap();
        manager.mark_loaded("vehicles/bicycle.glb", entity);
        assert_eq!(manager.loaded_count(), 1);
        assert!(manager.get_entity("vehicles/bicycle.glb").is_some());
    }

    #[test]
    fn test_get_entity_not_loaded() {
        let manager = AssetManager::new();
        assert!(manager.get_entity("vehicles/bicycle.glb").is_none());
    }
}
