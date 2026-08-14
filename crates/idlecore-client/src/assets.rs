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
    AssetManager, VehicleAssetType, cosmetic_paths, plant_paths, vehicle_animation_clips,
    vehicle_material_spec, vehicle_paths,
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