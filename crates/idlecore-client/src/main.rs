//! IdleBot — Bevy 0.19 2D hex-grid client.
//!
//! 2D world: x = east, y = north. Tiles are pixel-art isometric hexes built
//! from the Tiny* packs (`assets/models/Tiny */Tiles/`), the player is an
//! 8-direction animated pixel hero (`skins.rs`), and the server protocol is
//! unchanged.

#![allow(clippy::type_complexity)]

use bevy::prelude::*;
use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
};
use crate::player::{Player, PlayerTransform, PLAYER_SIZE};
use plugins::camera::CameraZoom;

mod player;
mod idle;
mod inventory;
mod minimap;
mod plugins;
mod skins;
mod net;
mod fps_counter;
mod world_floor;
mod tiny;
mod assets;


// --- Main Entry ---
fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        // Perf tracking: logs avg/max frame time + entity count every 5 s so
        // fps oscillation is visible in the log alongside world growth.
        .add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            LogDiagnosticsPlugin {
                wait_duration: std::time::Duration::from_secs(5),
                ..default()
            },
        ))
        .insert_resource(CameraZoom::default())
        .insert_resource(plugins::world::StreamingWorldResource::default())
        .insert_resource(minimap::MinimapState::default())
        .insert_resource(minimap::MinimapWaypoints::default())
        .insert_resource(minimap::MinimapMarkers::default())
        .insert_resource(minimap::HexEntityMap::default())
        .insert_resource(minimap::ExploredHexes::default())
        .insert_resource(minimap::WaypointEntityMap::default())
        .insert_resource(minimap::ChunkLoadState::default())
        .insert_resource(world_floor::FloorTiles::default())
        .insert_resource(world_floor::WaterTextures::default())
        .insert_resource(world_floor::SolidFloorTextures::default())
        .insert_resource(tiny::TinyKeyQueue::default())
        .add_plugins(plugins::player::PlayerPlugin)
        .add_plugins(plugins::camera::CameraPlugin)
        .add_plugins(plugins::world::WorldPlugin)
        .add_plugins(skins::SkinsPlugin)
        .add_systems(Update, tiny::process_key_queue)
        .add_plugins(fps_counter::FpsCounterPlugin)
        .add_plugins(net::plugin::NetPlugin)
        .add_plugins(net::hud::NetHudPlugin)
        .add_plugins(net::market::MarketPlugin)
        .add_plugins(net::craft::CraftPlugin)
        .add_plugins(inventory::InventoryPlugin)
        .insert_resource(PlayerTransform::default())
        .insert_resource(idle::IdleGainsState::default())
        .insert_resource(world_floor::FloorPlantState::default())
        .insert_resource(world_floor::WorldObjectState::default())
        .init_resource::<world_floor::ActionTarget>()
        .add_systems(Startup, (
            setup,
            assets::load_all_assets,
            minimap::spawn_minimap_ui,
            idle::spawn_idle_panel,
            world_floor::spawn_action_box,
            world_floor::init_prop_textures,
            world_floor::init_deco_textures,
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
            world_floor::update_world_object_visuals,
            world_floor::update_action_box,
        ))
        .add_systems(Update, (
            world_floor::init_water_textures,
            world_floor::init_solid_floor_textures,
            world_floor::update_world_floor
                .after(minimap::sync_player_state)
                .after(minimap::load_nearby_chunks)
                .after(world_floor::init_water_textures)
                .after(world_floor::init_solid_floor_textures),
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

/// Setup the 2D camera and the player sprite.
fn setup(mut commands: Commands) {
    // Start looking at the Earth-replica spawn forest (the server row snaps
    // us precisely once it replicates).
    let (sq, sr) = idlecore_core::earth::spawn_hex();
    let (wx, wy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
        sq,
        sr,
        idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
    );

    // 2D camera: follows the player (plugins/camera.rs), zoomed to the
    // tile-art pixels-per-unit scale.
    commands.spawn((
        Name::new("camera2d"),
        Camera2d,
        Transform::from_xyz(wx, wy, 1000.0),
        Projection::Orthographic(OrthographicProjection {
            // Bevy 0.19: scale = world units per pixel (1/px-per-unit).
            scale: 1.0 / CameraZoom::default().scale,
            // Planet-scale draw order: world y (±~100k at 1:100) maps onto z,
            // so the clip planes must cover the whole globe — the default_2d
            // band ([0,2000] world z) made everything vanish past ±1000.
            near: -350_000.0,
            far: 350_000.0,
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
        // Feet on the logical position; the tiny tiles are bottom-aligned.
        bevy::sprite::Anchor::BOTTOM_CENTER,
        Transform::from_xyz(wx, wy, crate::world_floor::prop_depth(wy) + 50.0),
        GlobalTransform::default(),
        player::ClientPlayer {
            position: Vec3::new(wx, wy, 0.0),
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