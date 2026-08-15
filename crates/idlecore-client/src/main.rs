//! IdleBot — Bevy 0.19 hex grid single-player client.

#![allow(clippy::type_complexity)]

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::player::{Player, PlayerTransform};
use plugins::camera::CameraZoom;
use plugins::world::StreamingWorldResource;

mod player;
mod idle;
mod minimap;
mod plugins;
mod skins;
mod net;
mod fps_counter;
mod world_floor;
mod assets;


// --- Main Entry ---
fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<()>::default())
        .add_plugins(RapierDebugRenderPlugin {
            enabled: false,
            ..default()
        })
        .add_systems(Startup, boost_gravity)
        .insert_resource(CameraZoom::default())
        .insert_resource(TimestepMode::Interpolated {
            // Physics steps at a fixed 60 Hz, and `TransformInterpolation` on
            // the player body renders smooth poses between steps — the render
            // frame rate can dip or jitter without the motion doing the same.
            dt: 1.0 / 60.0,
            time_scale: 1.0,
            substeps: 1,
        })
        .insert_resource(plugins::world::StreamingWorldResource::default())
        .insert_resource(minimap::MinimapState::default())
        .insert_resource(minimap::MinimapWaypoints::default())
        .insert_resource(minimap::MinimapMarkers::default())
        .insert_resource(minimap::HexEntityMap::default())
        .insert_resource(minimap::ExploredHexes::default())
        .insert_resource(minimap::WaypointEntityMap::default())
        .insert_resource(minimap::ChunkLoadState::default())
        .insert_resource(world_floor::WorldFloor::default())
        .add_plugins(plugins::player::PlayerPlugin)
        .add_plugins(plugins::camera::CameraPlugin)
        .add_plugins(plugins::world::WorldPlugin)
        .add_plugins(skins::SkinsPlugin)
        .add_plugins(fps_counter::FpsCounterPlugin)
        .add_plugins(net::plugin::NetPlugin)
        .add_plugins(net::hud::NetHudPlugin)
        .add_plugins(net::market::MarketPlugin)
        .insert_resource(PlayerTransform::default())
        .insert_resource(idle::IdleGainsState::default())
        .insert_resource(world_floor::FloorPlantState::default())
        .insert_resource(world_floor::FloorPlantAssets::default())
        .add_systems(Startup, (
            setup,
            assets::load_all_assets,
            // Must run after `setup` spawns the physics body (deferred
            // commands flush only between Startup systems).
            assets::spawn_vehicle_models.after(setup),
            minimap::spawn_minimap_ui,
            idle::spawn_idle_panel,
        ))
        .add_systems(Update, (
            toggle_physics_debug,
            minimap::handle_input,
            minimap::sync_player_state
                .after(minimap::handle_input)
                .after(PhysicsSet::Writeback),
            minimap::load_nearby_chunks
                .after(minimap::sync_player_state),
            minimap::render_visible_tiles
                .after(minimap::load_nearby_chunks),
            minimap::render_waypoints
                .after(minimap::render_visible_tiles)
                .after(minimap::handle_input),
            minimap::render_nav_markers
                .after(minimap::render_visible_tiles),
            minimap::render_remote_players
                .after(minimap::render_nav_markers)
                .after(minimap::sync_player_state),
            minimap::render_selection_marker
                .after(minimap::render_remote_players),
            minimap::resize_minimap_container
                .after(minimap::handle_input),
            minimap::update_player_marker
                .after(minimap::sync_player_state),
            idle::update_idle_gains_panel,
            world_floor::update_plant_visuals,
            world_floor::update_world_floor
                .after(minimap::sync_player_state)
                .after(minimap::load_nearby_chunks),
        ))
        .add_systems(Update, (
            assets::track_asset_loading,
            assets::spawn_cosmetic_layers,
            assets::sync_vehicle_model,
            assets::sync_cosmetic_layers,
            assets::toggle_cosmetic_layers,
            assets::apply_vehicle_material,
            assets::update_trail_vfx,
            assets::expire_trail_particles,
        ))
        .add_systems(Update, (
            assets::update_burst_vfx,
            assets::apply_burst_expansion,
            assets::expire_burst_particles,
        ))
        .run();
}

/// Setup lights, camera, and player
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    streaming_world: Res<StreamingWorldResource>,
) {
    // Directional sun
    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            color: Color::srgba(1.0, 0.95, 0.8, 1.0),
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
    ));
    
    // Camera
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(0.0, 60.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // Spawn player using the glTF character model.
    let player_scene: Handle<bevy::world_serialization::WorldAsset> =
        asset_server.load("models/characterLargeMale.glb#Scene0");

    // Drop the player in from above the starting hex; gravity settles it on the
    // terrain colliders.
    let start_height =
        3.0 + streaming_world.config.generate_hex(0, 0).elevation * 25.0;

    // Physics body: a top-level, unscaled entity in world units. Keep it off
    // the 13× scaled visual root — rapiper scales colliders by the entity
    // transform, which would otherwise balloon the capsule ~13× and float the
    // character high above the ground.
    commands.spawn((
        Name::new("PlayerPhysics"),
        plugins::player::PhysicsBody,
        RigidBody::Dynamic,
        Velocity::zero(),
        Ccd::enabled(),
        Collider::capsule(Vec3::Y * 1.6365, Vec3::Y * 6.0, 1.5),
        Friction::coefficient(0.8),
        TransformInterpolation::default(),
        Damping {
            linear_damping: 0.0,
            angular_damping: 6.0,
        },
        LockedAxes::ROTATION_LOCKED_X | LockedAxes::ROTATION_LOCKED_Z,
        Transform::from_xyz(0.0, start_height, 0.0),
        GlobalTransform::default(),
        player::ClientPlayer {
            position: Vec3::new(0.0, start_height, 0.0),
            velocity: Vec2::ZERO,
            current_hex: None,
            gold: 0,
            usdt: 0,
            xp: 0,
            level: 1,
            eco_points: 0,
            owned_vehicle: None,
            avatar: "Tetrahedron".to_string(),
            position_restored: false,
        },
    ));

    // Visual root: the 13× scaled character model; pose is copied from the
    // physics body by `sync_visual_to_physics`.
    commands.spawn((
        Name::new("Player"),
        Player,
        Transform {
            translation: Vec3::ZERO,
            scale: Vec3::splat(13.0),
            ..default()
        },
        GlobalTransform::default(),
        bevy::world_serialization::WorldAssetRoot(player_scene),
    ));
}

/// F12 toggles the Rapier debug renderer (collider wireframes).
fn toggle_physics_debug(
    keys: Res<ButtonInput<KeyCode>>,
    mut context: ResMut<DebugRenderContext>,
) {
    if keys.just_pressed(KeyCode::F12) {
        context.enabled = !context.enabled;
        info!(
            "Physics debug render: {}",
            if context.enabled { "ON" } else { "OFF" }
        );
    }
}
/// The world is big (hexes × 25-unit elevations), so the default rapier
/// gravity feels floaty; give it a snappier fall.
fn boost_gravity(
    mut configuration: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
) {
    if let Ok(mut config) = configuration.single_mut() {
        config.gravity = Vec3::NEG_Y * 45.0;
    }
}

