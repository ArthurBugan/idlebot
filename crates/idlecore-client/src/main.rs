//! IdleBot — Bevy 0.19 hex grid single-player client.

#![allow(dead_code)]

use bevy::prelude::*;
use crate::player::PlayerTransform;
use plugins::camera::CameraZoom;

mod progression;
mod player;
mod debug_panel;
mod idle;
mod minimap;
mod discovered_tiles;
mod plugins;



// --- Main Entry ---
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(CameraZoom::default())
        .insert_resource(discovered_tiles::DiscoveredTiles::default())
        .insert_resource(plugins::world::WorldResource::default())
        .insert_resource(minimap::MinimapState::default())
        .insert_resource(minimap::HexEntityMap::default())
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
            minimap::update_player_pos_system,
            minimap::discover_nearby_tiles_system,
            minimap::chunk_spawn_hex_system,
            minimap::build_minimap_atlas,
            minimap::update_minimap_ui,
            minimap::spawn_world_tiles,
            idle::update_idle_gains_panel,
        ))
        .run();
}

/// Setup lights, camera, and player
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    
    // Spawn player
    let player_mesh = meshes.add(create_player_mesh());
    let player_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.6, 1.0, 1.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("Player"),
        Player,
        Transform::from_xyz(0.0, 1.5, 0.0),
        GlobalTransform::default(),
        Mesh3d(player_mesh),
        MeshMaterial3d(player_material),
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



/// Create a capsule-shaped player avatar (cylinder with rounded ends)
fn create_player_mesh() -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let radius = 0.6;
    let height = 5.0;
    let segments = 16;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Body (cylinder)
    for y in 0..=segments {
        let angle = std::f32::consts::TAU * y as f32 / segments as f32;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, -height / 2.0, z]);
        positions.push([x, height / 2.0, z]);
    }

    // Top cap
    let top_center = positions.len();
    positions.push([0.0, height / 2.0 + radius, 0.0]);
    for i in 0..segments {
        let idx = i * 2 + 1;
        indices.extend_from_slice(&[top_center as u32, idx as u32, (idx + 1) as u32]);
    }

    // Bottom cap
    let bot_center = positions.len();
    positions.push([0.0, -height / 2.0 - radius, 0.0]);
    for i in 0..segments {
        let idx = i * 2;
        indices.extend_from_slice(&[bot_center as u32, (idx + 1) as u32, idx as u32]);
    }

    // Side faces
    for i in 0..segments {
        let i2 = i * 2;
        let i2n = (i + 1) * 2;
        indices.extend_from_slice(&[i2 as u32, i2n as u32, (i2 + 1) as u32]);
        indices.extend_from_slice(&[i2n as u32, (i2 + 1) as u32, (i2n + 1) as u32]);
    }

    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new("Vertex_Position", 0, bevy::render::mesh::VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Player marker component
#[derive(Component)]
struct Player;

