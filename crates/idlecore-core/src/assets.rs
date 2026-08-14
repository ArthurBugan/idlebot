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
#[derive(Resource)]
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

        for path in vehicle_paths() {
            self.handles.push(AssetHandle {
                path: path.to_string(),
                loaded: false,
                entity: None,
            });
        }
    }

    /// Load cosmetic assets (placeholder)
    pub fn load_cosmetic_assets(&mut self) {

        for path in cosmetic_paths() {
            self.handles.push(AssetHandle {
                path: path.to_string(),
                loaded: false,
                entity: None,
            });
        }
    }

    /// Load plant assets (placeholder)
    pub fn load_plant_assets(&mut self) {

        for path in plant_paths() {
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

// ---------------------------------------------------------------------------
// Material specs (Spec 016 T3.3)
// ---------------------------------------------------------------------------

/// PBR parameters applied to the vehicle visual per type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleMaterialSpec {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
    pub has_trail: bool,
    pub trail_lifetime: f32,
    pub trail_interval: f32,
}

/// PBR material per vehicle type (metallic/roughness vary by body material).
pub fn vehicle_material_spec(vehicle: VehicleAssetType) -> VehicleMaterialSpec {
    match vehicle {
        VehicleAssetType::None => VehicleMaterialSpec {
            r: 0.2, g: 0.2, b: 0.2,
            metallic: 0.0, roughness: 0.9, emissive: 0.0,
            has_trail: false, trail_lifetime: 0.0, trail_interval: 0.0,
        },
        // Painted steel frame.
        VehicleAssetType::Bicycle => VehicleMaterialSpec {
            r: 0.2, g: 0.9, b: 1.0,
            metallic: 0.55, roughness: 0.35, emissive: 0.05,
            has_trail: true, trail_lifetime: 0.6, trail_interval: 0.05,
        },
        // Enameled plastic deck.
        VehicleAssetType::Scooter => VehicleMaterialSpec {
            r: 0.6, g: 1.0, b: 0.3,
            metallic: 0.1, roughness: 0.5, emissive: 0.25,
            has_trail: true, trail_lifetime: 0.8, trail_interval: 0.04,
        },
        // Brushed metal engine parts.
        VehicleAssetType::Motorcycle => VehicleMaterialSpec {
            r: 1.0, g: 0.45, b: 0.2,
            metallic: 0.85, roughness: 0.2, emissive: 0.15,
            has_trail: true, trail_lifetime: 1.0, trail_interval: 0.03,
        },
        // Fiberglass hull.
        VehicleAssetType::Boat => VehicleMaterialSpec {
            r: 0.3, g: 0.6, b: 1.0,
            metallic: 0.25, roughness: 0.6, emissive: 0.05,
            has_trail: true, trail_lifetime: 0.7, trail_interval: 0.05,
        },
        // Aluminum airframe.
        VehicleAssetType::Airplane => VehicleMaterialSpec {
            r: 0.8, g: 0.5, b: 1.0,
            metallic: 0.9, roughness: 0.15, emissive: 0.4,
            has_trail: true, trail_lifetime: 1.2, trail_interval: 0.02,
        },
    }
}

/// Animation clip names per vehicle (Spec 016 T5.1): pedal/ride/float/fly/idle.
pub fn vehicle_animation_clips(vehicle: VehicleAssetType) -> &'static [&'static str] {
    match vehicle {
        VehicleAssetType::None => &[],
        VehicleAssetType::Bicycle | VehicleAssetType::Scooter => &["ride", "pedal", "idle"],
        VehicleAssetType::Motorcycle => &["ride", "idle"],
        VehicleAssetType::Boat => &["float", "idle"],
        VehicleAssetType::Airplane => &["fly", "idle"],
    }
}

// ---------------------------------------------------------------------------
// Path accessors (single source for Spec 016 T3.1/T4.1/T4.4)
// ---------------------------------------------------------------------------

/// Vehicle model paths, relative to the asset root.
pub fn vehicle_paths() -> &'static [&'static str] {
    &[
        "vehicles/bicycle.glb",
        "vehicles/scooter.glb",
        "vehicles/motorcycle.glb",
        "vehicles/boat.glb",
        "vehicles/airplane.glb",
    ]
}

/// Cosmetic model paths, relative to the asset root.
pub fn cosmetic_paths() -> &'static [&'static str] {
    &[
        "cosmetics/hat_basic.glb",
        "cosmetics/hat_cool.glb",
        "cosmetics/aura_fire.glb",
        "cosmetics/aura_ice.glb",
        "cosmetics/trail_stars.glb",
        "cosmetics/trail_rainbow.glb",
    ]
}

/// Plant model paths, relative to the asset root.
pub fn plant_paths() -> &'static [&'static str] {
    &[
        "plants/wheat.glb",
        "plants/tree.glb",
        "plants/rare_herb.glb",
    ]
}

#[cfg(test)]
mod spec_tests {
    use super::*;

    fn all_vehicle_types() -> Vec<VehicleAssetType> {
        vec![
            VehicleAssetType::None,
            VehicleAssetType::Bicycle,
            VehicleAssetType::Scooter,
            VehicleAssetType::Motorcycle,
            VehicleAssetType::Boat,
            VehicleAssetType::Airplane,
        ]
    }

    #[test]
    fn material_spec_pbr_ranges() {
        for v in all_vehicle_types() {
            let s = vehicle_material_spec(v);
            assert!((0.0..=1.0).contains(&s.metallic), "{v:?} metallic out of range");
            assert!((0.0..=1.0).contains(&s.roughness), "{v:?} roughness out of range");
            assert!((0.0..=1.0).contains(&s.emissive), "{v:?} emissive out of range");
            for c in [s.r, s.g, s.b] {
                assert!((0.0..=1.0).contains(&c), "{v:?} color channel out of range");
            }
        }
    }

    #[test]
    fn none_has_no_trail_all_real_vehicles_do() {
        assert!(!vehicle_material_spec(VehicleAssetType::None).has_trail);
        for v in all_vehicle_types().into_iter().skip(1) {
            let s = vehicle_material_spec(v);
            assert!(s.has_trail, "{v:?} should have a trail");
            assert!(s.trail_lifetime > 0.0);
            assert!(s.trail_interval > 0.0);
        }
    }

    #[test]
    fn animation_clips_per_vehicle() {
        assert!(vehicle_animation_clips(VehicleAssetType::None).is_empty());
        for v in all_vehicle_types().into_iter().skip(1) {
            let clips = vehicle_animation_clips(v);
            assert!(!clips.is_empty(), "{v:?} needs clips");
            assert!(clips.contains(&"idle"), "{v:?} missing idle clip");
            let primary = match v {
                VehicleAssetType::Bicycle | VehicleAssetType::Scooter
                | VehicleAssetType::Motorcycle => "ride",
                VehicleAssetType::Boat => "float",
                VehicleAssetType::Airplane => "fly",
                VehicleAssetType::None => unreachable!(),
            };
            assert!(clips.contains(&primary), "{v:?} missing {primary}");
        }
    }

    #[test]
    fn paths_end_in_glb_and_are_unique() {
        for paths in [vehicle_paths(), cosmetic_paths(), plant_paths()] {
            let mut seen = std::collections::HashSet::new();
            for p in paths {
                assert!(p.ends_with(".glb"), "{p} not a glb");
                assert!(seen.insert(*p), "{p} duplicated");
            }
        }
        assert_eq!(vehicle_paths().len(), 5);
        assert_eq!(cosmetic_paths().len(), 6);
        assert_eq!(plant_paths().len(), 3);
    }

    #[test]
    fn factory_count_matches_path_lists() {
        let mut m = AssetManager::new();
        m.load_vehicle_assets();
        assert_eq!(m.total_count(), vehicle_paths().len());
        m.load_cosmetic_assets();
        assert_eq!(m.total_count(), vehicle_paths().len() + cosmetic_paths().len());
        m.load_plant_assets();
        assert_eq!(m.total_count(), 14);
    }
}

// ---------------------------------------------------------------------------
// Primitive shape plans (Spec 016 T3.4 — <500 triangles per vehicle)
// ---------------------------------------------------------------------------

/// One primitive part of a vehicle/cosmetic model. Pure data; the client maps
/// it onto Bevy mesh builders with the resolutions mirrored in
/// `estimated_triangles`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapePart {
    /// (radius, height, segments)
    Cylinder { radius: f32, height: f32, segments: u32 },
    /// (major_radius, minor_radius, major_segments, minor_segments)
    Torus { major: f32, minor: f32, major_segments: u32, minor_segments: u32 },
    /// (x, y, z)
    Box { x: f32, y: f32, z: f32 },
    /// (radius, height, segments)
    Cone { radius: f32, height: f32, segments: u32 },
    /// (min_y, max_y, radius, segments)
    Capsule { min_y: f32, max_y: f32, radius: f32, segments: u32 },
    /// Regular tetrahedron; `size` is the base circumradius, apex at 1.63×size.
    Tetrahedron { size: f32 },
    /// UV sphere; (radius, segments)
    Sphere { radius: f32, segments: u32 },
}

impl ShapePart {
    /// Conservative triangle count for the builder resolutions used by the
    /// client (cylinder: 4·segments; cone: 3·segments; torus: major·minor·2;
    /// capsule: 10·segments with 4 latitudes; box: 12).
    pub fn estimated_triangles(&self) -> u32 {
        match self {
            ShapePart::Cylinder { segments, .. } => segments * 4,
            ShapePart::Torus { major_segments, minor_segments, .. } => {
                major_segments * minor_segments * 2
            }
            ShapePart::Box { .. } => 12,
            ShapePart::Cone { segments, .. } => segments * 3,
            ShapePart::Capsule { segments, .. } => segments * 10,
            ShapePart::Tetrahedron { .. } => 4,
            ShapePart::Sphere { segments, .. } => segments * segments * 2,
        }
    }
}

/// Vehicle model plans (Spec 016 T3.2/T3.4): every type stays under 500
/// triangles with primitive shapes.
pub fn vehicle_shape(vehicle: VehicleAssetType) -> &'static [ShapePart] {
    match vehicle {
        VehicleAssetType::None => &[],
        // Frame tube + saddle + two wheels.
        VehicleAssetType::Bicycle => &[
            ShapePart::Cylinder { radius: 0.05, height: 0.8, segments: 10 },
            ShapePart::Cylinder { radius: 0.05, height: 0.5, segments: 10 },
            ShapePart::Torus { major: 0.28, minor: 0.06, major_segments: 8, minor_segments: 5 },
            ShapePart::Torus { major: 0.28, minor: 0.06, major_segments: 8, minor_segments: 5 },
        ],
        // Deck + column + two small wheels.
        VehicleAssetType::Scooter => &[
            ShapePart::Box { x: 0.3, y: 0.05, z: 0.7 },
            ShapePart::Cylinder { radius: 0.04, height: 0.6, segments: 8 },
            ShapePart::Torus { major: 0.16, minor: 0.05, major_segments: 8, minor_segments: 4 },
            ShapePart::Torus { major: 0.16, minor: 0.05, major_segments: 8, minor_segments: 4 },
        ],
        // Body + two wheels + handlebar.
        VehicleAssetType::Motorcycle => &[
            ShapePart::Box { x: 0.45, y: 0.25, z: 1.1 },
            ShapePart::Torus { major: 0.3, minor: 0.07, major_segments: 10, minor_segments: 5 },
            ShapePart::Torus { major: 0.3, minor: 0.07, major_segments: 10, minor_segments: 5 },
            ShapePart::Cylinder { radius: 0.03, height: 0.7, segments: 8 },
        ],
        // Hull + cabin + mast.
        VehicleAssetType::Boat => &[
            ShapePart::Box { x: 0.7, y: 0.3, z: 1.3 },
            ShapePart::Box { x: 0.3, y: 0.25, z: 0.4 },
            ShapePart::Cylinder { radius: 0.03, height: 0.9, segments: 8 },
        ],
        // Fuselage + wings + tail.
        VehicleAssetType::Airplane => &[
            ShapePart::Box { x: 0.3, y: 0.3, z: 1.5 },
            ShapePart::Box { x: 1.3, y: 0.05, z: 0.35 },
            ShapePart::Box { x: 0.35, y: 0.05, z: 0.3 },
            ShapePart::Capsule { min_y: -0.25, max_y: 0.25, radius: 0.08, segments: 8 },
        ],
    }
}

/// Total estimated triangle count for a vehicle's model plan.
pub fn vehicle_triangle_budget(vehicle: VehicleAssetType) -> u32 {
    vehicle_shape(vehicle).iter().map(|p| p.estimated_triangles()).sum()
}

/// Cosmetic plans layered on top of the player mesh (Spec 016 T4.2/T4.3).
pub fn cosmetic_shape(cosmetic: CosmeticAssetType) -> &'static [ShapePart] {
    match cosmetic {
        CosmeticAssetType::Hat => &[
            ShapePart::Cone { radius: 0.35, height: 0.45, segments: 10 },
            ShapePart::Cylinder { radius: 0.37, height: 0.06, segments: 10 },
        ],
        CosmeticAssetType::Aura => &[
            ShapePart::Torus { major: 0.5, minor: 0.03, major_segments: 16, minor_segments: 4 },
        ],
        CosmeticAssetType::Trail => &[ShapePart::Box { x: 0.05, y: 0.05, z: 0.05 }],
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;

    #[test]
    fn every_vehicle_under_500_triangles() {
        for v in [
            VehicleAssetType::Bicycle,
            VehicleAssetType::Scooter,
            VehicleAssetType::Motorcycle,
            VehicleAssetType::Boat,
            VehicleAssetType::Airplane,
        ] {
            let budget = vehicle_triangle_budget(v);
            assert!(
                budget < 500,
                "{v:?} exceeds triangle budget: {budget} >= 500"
            );
            assert!(!vehicle_shape(v).is_empty(), "{v:?} has no parts");
        }
    }

    #[test]
    fn none_has_no_parts() {
        assert!(vehicle_shape(VehicleAssetType::None).is_empty());
        assert_eq!(vehicle_triangle_budget(VehicleAssetType::None), 0);
    }

    #[test]
    fn primitives_are_axis_aligned() {
        // Smoke: builders won't be fed degenerate sizes.
        for part in vehicle_shape(VehicleAssetType::Airplane) {
            match part {
                ShapePart::Box { x, y, z } => {
                    assert!(*x > 0.0 && *y > 0.0 && *z > 0.0);
                }
                ShapePart::Cylinder { radius, height, .. } => {
                    assert!(*radius > 0.0 && *height > 0.0);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn cosmetics_have_parts() {
        assert!(cosmetic_shape(CosmeticAssetType::Hat).len() == 2);
        assert!(cosmetic_shape(CosmeticAssetType::Aura).len() == 1);
        assert!(!cosmetic_shape(CosmeticAssetType::Trail).is_empty());
    }
}
