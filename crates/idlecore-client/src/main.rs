//! IdleBot — Bevy 0.19 hex grid single-player client.

use bevy::prelude::*;
use crate::player::PlayerTransform;

mod progression;
mod player;
mod debug_panel;
mod plugins;

// --- Main Entry ---
fn main() {
    eprintln!("=== IdleBot Starting ===");
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(plugins::player::PlayerPlugin)
        .add_plugins(plugins::camera::CameraPlugin)
        .add_plugins(plugins::world::WorldPlugin)
        .insert_resource(PlayerTransform::default())
        .insert_resource(debug_panel::DebugPanelOpen(false))
        .add_systems(Startup, (
            setup,
            spawn_minimap,
            debug_panel::spawn_debug_panel,
        ))
        .add_systems(Update, (
            debug_panel::debug_panel_toggle,
        ))
        .run();
}

/// Setup lights, camera, and player
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[SETUP] >>> STARTING SETUP <<<");
    
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
    eprintln!("[SETUP] Added directional sun");
    
    // Camera
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(0.0, 500.0, 500.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    eprintln!("[SETUP] Added camera");
    
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
    eprintln!("[SETUP] Player spawned at (0, 1.5, 0)");
}

/// Spawn the minimap overlay in the bottom-right corner
fn spawn_minimap(
    mut commands: Commands,
    window: Query<&Window>,
) {
    let win_w = window.iter().next().map(|w| w.width() as f32).unwrap_or(1920.0);
    let win_h = window.iter().next().map(|w| w.height() as f32).unwrap_or(1080.0);
    let minimap_size = 120.0;
    
    commands.spawn((
        Name::new("minimap"),
        Sprite {
            color: Color::srgba(0.0, 0.0, 0.0, 0.3),
            custom_size: Some(Vec2::new(minimap_size, minimap_size)),
            ..default()
        },
        Transform::from_xyz(
            win_w / 2.0 - minimap_size / 2.0 + 250.0,
            win_h / 2.0 - minimap_size / 2.0 - 150.0,
            1000.0,
        ),
        Camera2d::default(),
    ));
}

/// Create a capsule-shaped player avatar (cylinder with rounded ends)
fn create_player_mesh() -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let radius = 0.6;
    let height = 2.4;
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
