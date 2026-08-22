//! IdleBot — Bevy 0.19 2D hex-grid client.
//!
//! 2D world: x = east, y = north. Tiles are isometric sprites
//! (`assets/models/Isometric Tiles Base/PNG/`), the player is a skin sprite
//! (`assets/skins/*.png`), and the server protocol is unchanged.

#![allow(clippy::type_complexity)]

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use crate::player::{Player, PlayerTransform, PLAYER_SIZE};
use plugins::camera::CameraZoom;

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
        .insert_resource(CameraZoom::default())
        .insert_resource(plugins::world::StreamingWorldResource::default())
        .insert_resource(minimap::MinimapState::default())
        .insert_resource(minimap::MinimapWaypoints::default())
        .insert_resource(minimap::MinimapMarkers::default())
        .insert_resource(minimap::HexEntityMap::default())
        .insert_resource(minimap::ExploredHexes::default())
        .insert_resource(minimap::WaypointEntityMap::default())
        .insert_resource(minimap::ChunkLoadState::default())
        .insert_resource(world_floor::WorldFloor::default())
        .insert_resource(world_floor::WaterTextures::default())
        .insert_resource(world_floor::SolidFloorTextures::default())
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
        .add_systems(Startup, (
            setup,
            assets::load_all_assets,
            minimap::spawn_minimap_ui,
            idle::spawn_idle_panel,
        ))
        .add_systems(Update, (
            minimap::handle_input,
            minimap::sync_player_state
                .after(minimap::handle_input),
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
            minimap::restore_explored_on_login,
            minimap::autosave_explored,
            minimap::save_explored_on_exit,
            idle::update_idle_gains_panel,
            world_floor::update_plant_visuals,
            world_floor::init_water_textures,
            world_floor::init_solid_floor_textures,
            world_floor::update_world_floor
                .after(minimap::sync_player_state)
                .after(minimap::load_nearby_chunks)
                .after(world_floor::init_water_textures)
                .after(world_floor::init_solid_floor_textures),
            take_debug_screenshot,
        ))
        .add_systems(Update, (
            assets::spawn_cosmetic_layers,
            assets::sync_cosmetic_layers,
            assets::toggle_cosmetic_layers,
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

/// Temporary diagnostic: screenshot the main window a few seconds after boot.
fn take_debug_screenshot(
    time: Res<Time>,
    player: Res<PlayerTransform>,
    zoom: Res<CameraZoom>,
    floor: Res<world_floor::WorldFloor>,
    mut commands: Commands,
    mut shot: Local<f32>,
) {
    if *shot == 0.0 {
        *shot = time.elapsed_secs() + 25.0;
        return;
    }
    if time.elapsed_secs() >= *shot && *shot > 0.0 {
        info!(
            "debug: player at ({:.1}, {:.1}) zoom {:.1}px/u floor_chunks {}",
            player.translation.x,
            player.translation.y,
            zoom.scale,
            floor.entities.len()
        );
        commands.spawn((
            Screenshot::primary_window(),
            Transform::default(),
            GlobalTransform::default(),
        ))
        .observe(save_to_disk("/tmp/idlebot_floor_shot.png"));
        *shot = -1.0;
    }
}

/// Setup the 2D camera and the player sprite.
fn setup(mut commands: Commands) {
    // 2D camera: follows the player (plugins/camera.rs), zoomed to the
    // tile-art pixels-per-unit scale.
    commands.spawn((
        Name::new("camera2d"),
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Projection::Orthographic(OrthographicProjection {
            // Bevy 0.19: scale = world units per pixel (1/px-per-unit).
            scale: 1.0 / CameraZoom::default().scale,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // Player sprite: one entity carries the sprite, the ClientPlayer state
    // and the PhysicsBody marker (movement + net sync query it).
    commands.spawn((
        Name::new("Player"),
        Player,
        plugins::player::PhysicsBody,
        Sprite {
            custom_size: Some(Vec2::splat(PLAYER_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 50.0),
        GlobalTransform::default(),
        player::ClientPlayer {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: None,
            gold: 0,
            usdt: 0,
            xp: 0,
            level: 1,
            eco_points: 0,
            owned_vehicle: None,
            avatar: "alienA".to_string(),
            position_restored: false,
        },
    ));
}