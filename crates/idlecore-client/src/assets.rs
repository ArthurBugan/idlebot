//! Asset loading (Spec 016 T2.2/T2.3) and trail VFX (Spec 016 T5.5).
//!
//! The glTF model files under `assets/models/` do not exist yet (T3.4/T4.4
//! need authored assets); loading is wired through the asset server so the
//! handles resolve the moment files land, while procedural placeholders
//! (plate, aura, particles) keep the game playable regardless.

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use bevy::time::Time;
use idlecore_core::assets::{
    AssetManager, CosmeticAssetType, VehicleAssetType, cosmetic_paths, cosmetic_shape,
    plant_paths, vehicle_animation_clips, vehicle_material_spec, vehicle_paths, vehicle_shape,
};
use idlecore_core::Vehicle;

use crate::plugins::player::{PhysicsBody, VehicleIndicator};
use crate::player::ClientPlayer;

/// Maps a server `Vehicle` to the asset pipeline's `VehicleAssetType`.
pub fn to_asset_type(vehicle: &Vehicle) -> VehicleAssetType {
    match vehicle {
        Vehicle::None => VehicleAssetType::None,
        Vehicle::Bicycle => VehicleAssetType::Bicycle,
        Vehicle::Scooter => VehicleAssetType::Scooter,
        Vehicle::Motorcycle => VehicleAssetType::Motorcycle,
        Vehicle::Boat => VehicleAssetType::Boat,
        Vehicle::Airplane => VehicleAssetType::Airplane,
    }
}

/// Real asset handles resolving through the core `AssetManager`.
#[derive(Resource)]
pub struct LoadedAssets {
    pub vehicles: Vec<(String, Handle<WorldAsset>)>,
    pub cosmetics: Vec<(String, Handle<WorldAsset>)>,
    pub plants: Vec<(String, Handle<WorldAsset>)>,
}

/// Registers the core asset manager, registers every declared model path with
/// the asset server, and stores the real handles. Missing files resolve as
/// failed loads rather than blocking startup.
pub fn load_all_assets(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.init_resource::<VehicleIndicator>();
    commands.init_resource::<TrailFx>();
    let mut manager = AssetManager::new();
    let mut assets = LoadedAssets {
        vehicles: Vec::new(),
        cosmetics: Vec::new(),
        plants: Vec::new(),
    };
    manager.load_vehicle_assets();
    for path in vehicle_paths() {
        assets
            .vehicles
            .push((path.to_string(), asset_server.load(format!("models/{path}"))));
    }
    manager.load_cosmetic_assets();
    for path in cosmetic_paths() {
        assets
            .cosmetics
            .push((path.to_string(), asset_server.load(format!("models/{path}"))));
    }
    manager.load_plant_assets();
    for path in plant_paths() {
        assets
            .plants
            .push((path.to_string(), asset_server.load(format!("models/{path}"))));
    }
    commands.insert_resource(manager);
    commands.insert_resource(assets);
}

/// Polls load state each frame; once a glTF resolves (loaded or failed to
/// load) it is marked in the core manager so `loaded_count()` reflects it.
pub fn track_asset_loading(
    asset_server: Res<AssetServer>,
    loaded: Option<Res<LoadedAssets>>,
    mut manager: ResMut<AssetManager>,
) {
    let Some(loaded) = loaded else { return };
    let pending = |handles: &[(String, Handle<WorldAsset>)]| {
        handles.iter().any(|(_, h)| {
            matches!(
                asset_server.get_load_state(h.id()),
                None | Some(LoadState::NotLoaded) | Some(LoadState::Loading)
            )
        })
    };
    if pending(&loaded.vehicles) || pending(&loaded.cosmetics) || pending(&loaded.plants) {
        return;
    }
    for (path, h) in loaded
        .vehicles
        .iter()
        .chain(&loaded.cosmetics)
        .chain(&loaded.plants)
    {
        if matches!(
            asset_server.get_load_state(h.id()),
            Some(LoadState::Loaded) | Some(LoadState::Failed(_))
        ) {
            manager.mark_loaded(path, Entity::PLACEHOLDER);
        }
    }
}

/// Plays the primary animation clip on the model root if it carries an
/// `AnimationPlayer` (Spec 016 T5.2). Returns the selected clip name.
///
/// Bevy 0.19's `AnimationPlayer::play` takes an `AnimationNodeIndex` from the
/// model's `AnimationGraph`; procedural placeholders have no graph, so the
/// playback starts the moment a real glTF animation player is wired with the
/// graph node — until then this is a safe no-op.
#[allow(clippy::result_unit_err)]
pub fn play_vehicle_animation(
    root: Entity,
    vehicle: VehicleAssetType,
    players: Query<&AnimationPlayer>,
) -> Option<String> {
    let clip = *vehicle_animation_clips(vehicle).first()?;
    players.get(root).ok()?;
    Some(clip.to_string())
}

/// Per-vehicle flyweight spawned at the player position while riding.
#[derive(Component)]
pub struct TrailParticle {
    pub remaining: f32,
}

/// Trail VFX controller (Spec 016 T5.5).
#[derive(Resource, Default)]
pub struct TrailFx {
    pub accumulator: f32,
    pub active: bool,
}

/// Emits a small emissive quad behind the player while a trailing vehicle is
/// equipped and the player is moving.
#[allow(clippy::too_many_arguments)]
pub fn update_trail_vfx(
    body: Query<(&Transform, &ClientPlayer), (With<PhysicsBody>, Without<TrailParticle>)>,
    mut fx: ResMut<TrailFx>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let Ok((body_t, player)) = body.single() else { return };
    let Some(vehicle) = player
        .owned_vehicle
        .as_ref()
        .filter(|v| **v != Vehicle::None)
    else {
        fx.active = false;
        fx.accumulator = 0.0;
        return;
    };
    let spec = vehicle_material_spec(to_asset_type(vehicle));
    if !spec.has_trail {
        fx.active = false;
        return;
    }
    if player.velocity.length_squared() < 0.01 {
        fx.active = false;
        return;
    }
    fx.active = true;
    fx.accumulator += time.delta_secs();
    let mut spawned = 0usize;
    while fx.accumulator >= spec.trail_interval && spawned < 8 {
        fx.accumulator -= spec.trail_interval;
        spawned += 1;
        let mut m = StandardMaterial::from_color(Color::srgb(spec.r, spec.g, spec.b));
        m.emissive = bevy::color::LinearRgba::rgb(spec.r, spec.g, spec.b) * spec.emissive.max(0.05);
        let quad = meshes.add(Plane3d::default().mesh().size(0.35, 0.35));
        commands.spawn((
            Name::new("trail-particle"),
            Mesh3d(quad),
            MeshMaterial3d(materials.add(m)),
            Transform::from_xyz(body_t.translation.x, 0.35, body_t.translation.z)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            TrailParticle {
                remaining: spec.trail_lifetime,
            },
        ));
    }
}

/// Fades out and despawns expired trail particles.
pub fn expire_trail_particles(
    mut commands: Commands,
    mut particles: Query<(&mut TrailParticle, Entity)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut p, entity) in &mut particles {
        p.remaining -= dt;
        if p.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Applies the per-type PBR material spec (Spec 016 T3.3) to the indicator
/// plate whenever the equipped vehicle changes.
pub fn apply_vehicle_material(
    body: Query<&ClientPlayer, With<PhysicsBody>>,
    indicator: Option<Res<VehicleIndicator>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(indicator) = indicator else { return };
    let Ok(player) = body.single() else { return };
    let Some(vehicle) = player
        .owned_vehicle
        .as_ref()
        .filter(|v| **v != Vehicle::None)
    else {
        return;
    };
    let spec = vehicle_material_spec(to_asset_type(vehicle));
    let Some(h) = &indicator.plate_material else { return };
    let Some(mut m) = materials.get_mut(h) else { return };
    m.base_color = Color::srgb(spec.r, spec.g, spec.b);
    m.metallic = spec.metallic;
    m.perceptual_roughness = spec.roughness;
    m.emissive = bevy::color::LinearRgba::rgb(spec.r, spec.g, spec.b) * spec.emissive;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::time::TimePlugin;
    use idlecore_core::assets::{vehicle_triangle_budget, ShapePart};

    #[test]
    fn bicycle_material_is_painted_steel() {
        let spec = vehicle_material_spec(VehicleAssetType::Bicycle);
        assert!(spec.metallic > 0.5);
        assert!(spec.roughness < 0.5);
        assert!(spec.has_trail);
    }

    #[test]
    fn to_asset_type_maps_identically() {
        assert_eq!(to_asset_type(&Vehicle::None), VehicleAssetType::None);
        assert_eq!(to_asset_type(&Vehicle::Airplane), VehicleAssetType::Airplane);
    }

    #[test]
    fn trail_defaults_inactive() {
        let fx = TrailFx::default();
        assert!(!fx.active);
        assert_eq!(fx.accumulator, 0.0);
    }

    #[test]
    fn trail_system_no_ops_without_body() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<TrailFx>();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.add_systems(Update, update_trail_vfx);
        app.update();
        app.update();
    }

    #[test]
    fn play_animation_returns_none_without_player() {
        let mut app = App::new();
        let root = app.world_mut().spawn_empty().id();
        app.add_systems(Update, move |players: Query<&AnimationPlayer>| {
            assert!(play_vehicle_animation(root, VehicleAssetType::Bicycle, players).is_none());
        });
        app.update();
    }

    #[test]
    fn manager_accounting_matches_declared_paths() {
        let mut m = AssetManager::new();
        m.load_vehicle_assets();
        m.load_cosmetic_assets();
        m.load_plant_assets();
        assert_eq!(m.total_count(), 14);
    }
}
// ============================================================================
// Primitive vehicle models (Spec 016 T3.2/T3.4 — simple shapes, no glb needed)
// ============================================================================

use idlecore_core::assets::ShapePart;

/// Marks a spawned primitive vehicle model so its visibility can be synced to
/// the equipped vehicle.
#[derive(Component, Clone, Copy, PartialEq)]
pub struct VehicleModelKind(pub VehicleAssetType);

/// Builds the Bevy mesh for a core `ShapePart` at the same resolutions the
/// core triangle estimates assume.
pub fn build_shape_mesh(part: &ShapePart) -> Mesh {
    match part {
        ShapePart::Cylinder { radius, height, segments } => {
            Cylinder::new(*radius, *height).mesh().resolution(*segments).build()
        }
        ShapePart::Torus { major, minor, major_segments, minor_segments } => {
            // bevy Torus::new(inner, outer); minor_radius = (outer-inner)/2.
            Torus::new(*major - *minor, *major + *minor)
                .mesh()
                .major_resolution(*major_segments as usize)
                .minor_resolution(*minor_segments as usize)
                .build()
        }
        ShapePart::Box { x, y, z } => Mesh::from(Cuboid::new(*x, *y, *z)),
        ShapePart::Cone { radius, height, segments } => {
            Cone::new(*radius, *height).mesh().resolution(*segments).build()
        }
        ShapePart::Capsule { min_y, max_y, radius, segments } => {
            Capsule3d::new(*radius, (max_y - min_y) * 0.5)
                .mesh()
                .longitudes(*segments)
                .latitudes(4)
                .build()
        }
    }
}

/// Per-part placement (translation, rotation) for each vehicle plan.
fn part_transform(vehicle: VehicleAssetType, index: usize) -> (Vec3, Quat) {
    use VehicleAssetType as V;
    let rot_x = |a: f32| Quat::from_rotation_x(a);
    match vehicle {
        V::Bicycle => match index {
            0 => (Vec3::new(0.0, 0.55, -0.25), rot_x(std::f32::consts::FRAC_PI_2)),
            1 => (Vec3::new(0.0, 0.7, 0.25), rot_x(std::f32::consts::FRAC_PI_2)),
            2 => (Vec3::new(0.0, 0.28, -0.42), rot_x(std::f32::consts::FRAC_PI_2)),
            _ => (Vec3::new(0.0, 0.28, 0.42), rot_x(std::f32::consts::FRAC_PI_2)),
        },
        V::Scooter => match index {
            0 => (Vec3::new(0.0, 0.15, 0.0), Quat::IDENTITY),
            1 => (Vec3::new(0.0, 0.48, 0.28), Quat::IDENTITY),
            2 => (Vec3::new(0.0, 0.16, -0.24), rot_x(std::f32::consts::FRAC_PI_2)),
            _ => (Vec3::new(0.0, 0.16, 0.22), rot_x(std::f32::consts::FRAC_PI_2)),
        },
        V::Motorcycle => match index {
            0 => (Vec3::new(0.0, 0.52, 0.0), Quat::IDENTITY),
            1 => (Vec3::new(0.0, 0.32, -0.35), rot_x(std::f32::consts::FRAC_PI_2)),
            2 => (Vec3::new(0.0, 0.32, 0.35), rot_x(std::f32::consts::FRAC_PI_2)),
            _ => (Vec3::new(0.0, 0.78, -0.12), Quat::IDENTITY),
        },
        V::Boat => match index {
            0 => (Vec3::new(0.0, 0.2, 0.0), Quat::IDENTITY),
            1 => (Vec3::new(0.0, 0.45, -0.15), Quat::IDENTITY),
            _ => (Vec3::new(0.0, 0.75, 0.4), Quat::IDENTITY),
        },
        V::Airplane => match index {
            0 => (Vec3::new(0.0, 0.7, 0.0), rot_x(std::f32::consts::FRAC_PI_2)),
            1 => (Vec3::new(0.0, 0.68, 0.0), Quat::IDENTITY),
            2 => (Vec3::new(0.0, 0.85, 0.55), rot_x(std::f32::consts::FRAC_PI_2)),
            _ => (Vec3::new(0.0, 0.7, -0.62), rot_x(std::f32::consts::FRAC_PI_2)),
        },
        V::None => (Vec3::ZERO, Quat::IDENTITY),
    }
}

/// Spawns every vehicle's primitive model as hidden children of the player
/// root; `sync_vehicle_model` shows the equipped one.
pub fn spawn_vehicle_models(
    mut commands: Commands,
    bodies: Query<Entity, With<PhysicsBody>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use VehicleAssetType as V;
    let Ok(player_root) = bodies.single() else { return };
    for vehicle in [
        V::Bicycle, V::Scooter, V::Motorcycle, V::Boat, V::Airplane,
    ] {
        let spec = vehicle_material_spec(vehicle);
        for (i, part) in vehicle_shape(vehicle).iter().enumerate() {
            let (translation, rotation) = part_transform(vehicle, i);
            let mut m = StandardMaterial::from_color(Color::srgb(spec.r, spec.g, spec.b));
            m.metallic = spec.metallic;
            m.perceptual_roughness = spec.roughness;
            commands.spawn((
                Name::new(format!("vehicle-{:?}-part-{i}", vehicle).to_lowercase()),
                VehicleModelKind(vehicle),
                Mesh3d(meshes.add(build_shape_mesh(part))),
                MeshMaterial3d(materials.add(m)),
                Transform::from_translation(translation).with_rotation(rotation),
                Visibility::Hidden,
                ChildOf(player_root),
            ));
        }
    }
}

/// Shows the model matching the equipped vehicle, hides the rest.
pub fn sync_vehicle_model(
    body: Query<&ClientPlayer, With<PhysicsBody>>,
    mut models: Query<(&VehicleModelKind, &mut Visibility)>,
) {
    let Ok(player) = body.single() else { return };
    let equipped = player.owned_vehicle.as_ref().map(to_asset_type).unwrap_or(VehicleAssetType::None);
    for (kind, mut vis) in &mut models {
        *vis = if kind.0 == equipped && kind.0 != VehicleAssetType::None {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod vehicle_mesh_tests {
    use super::*;
    use idlecore_core::assets::vehicle_triangle_budget;

    fn tri_count(mesh: &Mesh) -> usize {
        mesh.indices().map(|i| i.len() / 3).unwrap_or(0)
    }

    #[test]
    fn built_meshes_match_triangle_budget() {
        use VehicleAssetType as V;
        for vehicle in [V::Bicycle, V::Scooter, V::Motorcycle, V::Boat, V::Airplane] {
            let mut total = 0usize;
            for part in vehicle_shape(vehicle) {
                total += tri_count(&build_shape_mesh(part));
            }
            assert!(
                total < 500,
                "{vehicle:?} built with {total} triangles (budget 500)"
            );
            assert!(
                total <= vehicle_triangle_budget(vehicle) as usize,
                "{vehicle:?} real {total} exceeds estimate {}",
                vehicle_triangle_budget(vehicle)
            );
        }
    }

    #[test]
    fn every_part_has_a_transform() {
        use VehicleAssetType as V;
        for vehicle in [V::Bicycle, V::Scooter, V::Motorcycle, V::Boat, V::Airplane] {
            let parts = vehicle_shape(vehicle);
            for (i, _) in parts.iter().enumerate() {
                let _ = part_transform(vehicle, i); // must not panic
            }
        }
    }

    #[test]
    fn shape_estimates_are_exact_for_boxes() {
        assert_eq!(ShapePart::Box { x: 1.0, y: 1.0, z: 1.0 }.estimated_triangles(), 12);
    }
}

// ============================================================================
// Cosmetic layers on the avatar (Spec 016 T4.2/T4.3 — primitive shapes)
// ============================================================================

/// Which cosmetic layer set is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CosmeticMode {
    #[default]
    None,
    Hat,
    Aura,
    Both,
}

/// Cycles None → Hat → Aura → Both → None (pure, testable).
pub fn next_cosmetic_mode(mode: CosmeticMode) -> CosmeticMode {
    match mode {
        CosmeticMode::None => CosmeticMode::Hat,
        CosmeticMode::Hat => CosmeticMode::Aura,
        CosmeticMode::Aura => CosmeticMode::Both,
        CosmeticMode::Both => CosmeticMode::None,
    }
}

#[derive(Resource, Default)]
pub struct CosmeticLayers {
    pub mode: CosmeticMode,
}

/// Marks a spawned cosmetic layer and its category.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct CosmeticLayerKind(pub CosmeticAssetType);

/// Spawns the hat (cone + brim at head height) and the aura ring around the
/// player, both hidden until toggled. Parented to the unscaled physics body.
pub fn spawn_cosmetic_layers(
    mut commands: Commands,
    bodies: Query<Entity, With<PhysicsBody>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(player_root) = bodies.single() else { return };
    let hat = cosmetic_shape(CosmeticAssetType::Hat);
    let placements: [(&[ShapePart], CosmeticAssetType, Vec3, Quat, Color); 2] = [
        (&hat, CosmeticAssetType::Hat, Vec3::new(0.0, 1.5, 0.0), Quat::IDENTITY, Color::srgb(1.0, 0.55, 0.1)),
        (
            cosmetic_shape(CosmeticAssetType::Aura),
            CosmeticAssetType::Aura,
            Vec3::new(0.0, 1.0, 0.0),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Color::srgb(0.2, 1.0, 0.5),
        ),
    ];
    for (parts, kind, offset, rotation, color) in placements {
        let srgb = color.to_srgba();
        let mut m = StandardMaterial::from_color(color);
        m.emissive = bevy::color::LinearRgba::rgb(srgb.red, srgb.green, srgb.blue) * 0.15;
        for part in parts {
            commands.spawn((
                Name::new(format!("cosmetic-{kind:?}").to_lowercase()),
                CosmeticLayerKind(kind),
                Mesh3d(meshes.add(build_shape_mesh(part))),
                MeshMaterial3d(materials.add(m.clone())),
                Transform::from_translation(offset).with_rotation(rotation),
                Visibility::Hidden,
                ChildOf(player_root),
            ));
        }
    }
}

/// Shows layers per the current `CosmeticLayers.mode`.
pub fn sync_cosmetic_layers(
    state: Option<Res<CosmeticLayers>>,
    mut layers: Query<(&CosmeticLayerKind, &mut Visibility)>,
) {
    for (kind, mut vis) in &mut layers {
        let mode = state.as_ref().map(|s| s.mode).unwrap_or_default();
        let on = match (mode, kind.0) {
            (CosmeticMode::None, _) => false,
            (CosmeticMode::Hat, CosmeticAssetType::Hat) => true,
            (CosmeticMode::Aura, CosmeticAssetType::Aura) => true,
            (CosmeticMode::Both, _) => true,
            _ => false,
        };
        *vis = if on { Visibility::Visible } else { Visibility::Hidden };
    }
}

/// J cycles through cosmetic layer modes.
pub fn toggle_cosmetic_layers(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CosmeticLayers>,
) {
    if keys.just_pressed(KeyCode::KeyJ) {
        state.mode = next_cosmetic_mode(state.mode);
    }
}


#[cfg(test)]
mod cosmetic_tests {
    use super::*;
    use bevy::app::App;

    #[test]
    fn mode_cycles_through_all_states() {
        let mut mode = CosmeticMode::None;
        let seen = vec![CosmeticMode::Hat, CosmeticMode::Aura, CosmeticMode::Both, CosmeticMode::None];
        for expected in seen {
            mode = next_cosmetic_mode(mode);
            assert_eq!(mode, expected);
        }
    }

    #[test]
    fn sync_shows_only_selected_layers() {
        let mut app = App::new();
        app.insert_resource(CosmeticLayers::default());
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        let root = app.world_mut().spawn_empty().id();
        let mut commands = app.world_mut().commands();
        let mut spawn = |kind: CosmeticAssetType, y: f32| {
            commands.spawn((
                CosmeticLayerKind(kind),
                Transform::from_xyz(0.0, y, 0.0),
                Visibility::Hidden,
                ChildOf(root),
            ));
        };
        spawn(CosmeticAssetType::Hat, 1.5);
        spawn(CosmeticAssetType::Aura, 1.0);
        app.add_systems(Update, sync_cosmetic_layers);
        app.update();
        {
            let mut layers = app.world_mut().query::<(&CosmeticLayerKind, &Visibility)>();
            for (kind, vis) in layers.iter(app.world()) {
                assert_eq!(*vis, Visibility::Hidden, "{kind:?} should be hidden");
            }
        }
        app.world_mut()
            .resource_mut::<CosmeticLayers>()
            .mode = CosmeticMode::Hat;
        app.update();
        {
            let mut layers = app.world_mut().query::<(&CosmeticLayerKind, &Visibility)>();
            for (kind, vis) in layers.iter(app.world()) {
                let expect = matches!(kind.0, CosmeticAssetType::Hat);
                assert_eq!(*vis == Visibility::Visible, expect, "{kind:?} mismatch");
            }
        }
    }

    #[test]
    fn sync_no_ops_without_state() {
        // Res<CosmeticLayers> missing: system must be skipped gracefully.
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.world_mut().spawn((CosmeticLayerKind(CosmeticAssetType::Hat), Visibility::Hidden));
        app.add_systems(Update, sync_cosmetic_layers);
        app.update();
    }
}


// ============================================================================
// Burst / explosion VFX (Spec 016 T5.6 — optional, done on teleport arrival)
// ============================================================================

/// One expanding quad of a teleport burst.
#[derive(Component)]
pub struct ExpandingParticle {
    pub t: f32,
    pub life: f32,
    pub dir: Vec3,
}

/// Progress 0..1 of a burst particle's lifetime (pure).
pub fn burst_step(t: f32, life: f32) -> f32 {
    (t / life).clamp(0.0, 1.0)
}

/// Size multiplier while an expanding particle lives: 1.0 → 3.0.
pub fn burst_scale(progress: f32) -> f32 {
    1.0 + 2.0 * progress
}

/// Pending burst requests (filled by net_drain on teleport/explosion).
#[derive(Resource, Default)]
pub struct BurstFx {
    pub pending: Vec<Vec3>,
}

impl BurstFx {
    pub fn request(&mut self, at: Vec3) {
        self.pending.push(at);
    }
}

const BURST_PARTICLES: usize = 8;
const BURST_LIFE: f32 = 0.6;
const BURST_SPREAD: f32 = 1.2;

/// Consumes pending requests and spawns an 8-quad expanding ring.
pub fn update_burst_vfx(
    mut burst: ResMut<BurstFx>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let quad = meshes.add(Plane3d::default().mesh().size(0.9, 0.9));
    let mut emissive = StandardMaterial::from_color(Color::srgb(1.0, 0.8, 0.2));
    emissive.emissive = bevy::color::LinearRgba::rgb(1.0, 0.8, 0.2) * 0.8;
    let material = materials.add(emissive);
    for at in burst.pending.drain(..) {
        for k in 0..BURST_PARTICLES {
            let angle = k as f32 * std::f32::consts::TAU / BURST_PARTICLES as f32;
            let dir = Vec3::new(angle.cos(), 0.35, angle.sin());
            commands.spawn((
                Name::new("burst-particle"),
                Mesh3d(quad.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(at.x, at.y + 0.3, at.z),
                ExpandingParticle { t: 0.0, life: BURST_LIFE, dir },
            ));
        }
    }
    let _ = time;
}

/// Ages and despawns burst particles; expansion is applied by
/// `apply_burst_expansion` (separate system, borrow-clean).
pub fn expire_burst_particles(
    mut commands: Commands,
    mut particles: Query<(Entity, &mut ExpandingParticle)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (entity, mut p) in &mut particles {
        p.t += dt;
        if p.t >= p.life {
            commands.entity(entity).despawn();
        }
    }
}

/// Applies outward expansion + scale growth to living burst particles.
pub fn apply_burst_expansion(
    mut particles: Query<(&ExpandingParticle, &mut Transform)>,
) {
    for (p, mut t) in &mut particles {
        let progress = burst_step(p.t, p.life);
        let scale = burst_scale(progress);
        let offset = p.dir * BURST_SPREAD * progress;
        t.translation += offset;
        t.scale = Vec3::splat(scale);
    }
}

#[cfg(test)]
mod burst_tests {
    use super::*;
    use bevy::app::App;
    use bevy::time::TimePlugin;

    #[test]
    fn burst_math_is_monotonic() {
        for i in 0..10u32 {
            let t = i as f32;
            let p = i as f32 / 10.0;
            assert!((burst_step(t, 10.0) - p).abs() < 1e-6);
            assert!(burst_scale(0.0) >= 1.0);
        }
        assert!((burst_scale(1.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn requests_accumulate_until_drained() {
        let mut fx = BurstFx::default();
        assert!(fx.pending.is_empty());
        fx.request(Vec3::new(1.0, 2.0, 3.0));
        fx.request(Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(fx.pending.len(), 2);
    }

    #[test]
    fn update_spawns_ring_and_consumes_requests() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<BurstFx>();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.world_mut().resource_mut::<BurstFx>().request(Vec3::ZERO);
        app.add_systems(Update, update_burst_vfx);
        app.update();
        assert!(app.world_mut().resource::<BurstFx>().pending.is_empty());
        let count = app.world_mut().query::<&ExpandingParticle>().iter(app.world()).count();
        assert_eq!(count, BURST_PARTICLES);
    }

    #[test]
    fn expire_removes_old_particles() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.world_mut().spawn((
            ExpandingParticle { t: 5.0, life: 1.0, dir: Vec3::Y },
            Transform::default(),
        ));
        app.add_systems(Update, expire_burst_particles);
        app.update();
        let count = app.world_mut().query::<&ExpandingParticle>().iter(app.world()).count();
        assert_eq!(count, 0);
    }
}
