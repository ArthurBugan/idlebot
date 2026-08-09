//! IdleBot — Bevy 0.19 hex grid single-player client.

#![allow(dead_code)]

use bevy::prelude::*;
use crate::player::{Player, PlayerTransform};
use plugins::camera::CameraZoom;

mod progression;
mod player;
mod debug_panel;
mod idle;
mod minimap;
mod plugins;
mod world_floor;


// --- Main Entry ---
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(CameraZoom::default())
        .insert_resource(plugins::world::StreamingWorldResource::default())
        .insert_resource(minimap::MinimapState::default())
        .insert_resource(minimap::MinimapConfig::default())
        .insert_resource(minimap::MinimapWaypoints::default())
        .insert_resource(minimap::MinimapMarkers::default())
        .insert_resource(minimap::HexEntityMap::default())
        .insert_resource(minimap::HexFogMap::default())
        .insert_resource(minimap::ExploredHexes::default())
        .insert_resource(minimap::WaypointEntityMap::default())
        .insert_resource(minimap::ChunkLoadState::default())
        .insert_resource(world_floor::WorldFloor::default())
        .add_plugins(plugins::player::PlayerPlugin)
        .add_plugins(plugins::camera::CameraPlugin)
        .add_plugins(plugins::world::WorldPlugin)
        .insert_resource(PlayerTransform::default())
        .insert_resource(debug_panel::DebugPanelOpen(false))
        .insert_resource(idle::IdleGainsState::default())
        .add_systems(Startup, (
            setup,
            minimap::spawn_minimap_ui,
            idle::spawn_idle_panel,
        ))
        .add_systems(Update, (
            minimap::handle_input,
            minimap::sync_player_state.after(minimap::handle_input),
            minimap::load_nearby_chunks
                .after(minimap::sync_player_state),
            minimap::render_visible_tiles
                .after(minimap::load_nearby_chunks),
            minimap::render_waypoints
                .after(minimap::render_visible_tiles)
                .after(minimap::handle_input),
            minimap::render_nav_markers
                .after(minimap::render_visible_tiles),
            minimap::resize_minimap_container
                .after(minimap::handle_input),
            minimap::update_player_marker
                .after(minimap::sync_player_state),
            idle::update_idle_gains_panel,
            world_floor::update_world_floor
                .after(minimap::sync_player_state)
                .after(minimap::load_nearby_chunks),
        ))
        .run();
}

/// Setup lights, camera, and player
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
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

    commands.spawn((
        Name::new("Player"),
        Player,
        Transform {
            translation: Vec3::new(0.0, 0.35, 0.0),
            scale: Vec3::splat(13.0),
            ..default()
        },
        GlobalTransform::default(),
        bevy::world_serialization::WorldAssetRoot(player_scene),
        player::ClientPlayer {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: None,
            gold: 0,
            xp: 0,
            level: 1,
            eco_points: 0,
            owned_vehicle: None,
            equipped_cosmetics: Vec::new(),
            last_login_time: 0,
            time_offline: None,
        },
    ));
}

