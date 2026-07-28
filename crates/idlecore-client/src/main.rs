/usr/bin/bash: warning: setlocale: LC_ALL: cannot change locale (en_US.UTF-8): No such file or directory
/usr/bin/bash: warning: setlocale: LC_ALL: cannot change locale (en_US.UTF-8): No such file or directory
//! IdleBot — Bevy 0.19 hex grid single-player client.

use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use bevy::asset::Assets;
use bevy::render::mesh::Mesh;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

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
        let i2n = ((i + 1) * 2);
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

/// Create a flat-top hexagonal prism (radius, height)
fn create_hex_mesh(radius: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let h = 0.15;
    let corners: Vec<[f32; 2]> = (0..6)
        .map(|i| {
            let angle = std::f32::consts::PI / 3.0 * i as f32;
            [radius * angle.cos(), radius * angle.sin()]
        })
        .collect();
    let top: Vec<[f32; 3]> = corners.iter().map(|c| [c[0], c[1], h]).collect();
    let bottom: Vec<[f32; 3]> = corners.iter().map(|c| [c[0], c[1], 0.0]).collect();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    positions.push([0.0, 0.0, h]);
    for &c in &top { positions.push(c); }
    let center_idx = positions.len() as u32 - 7;
    for i in 0..6u32 {
        indices.extend_from_slice(&[center_idx, center_idx + i + 1, center_idx + ((i + 1) % 6) + 1]);
    }
    let bot_start = positions.len() as u32;
    for &c in &bottom { positions.push(c); }
    let bot_center = bot_start + 6;
    positions.push([0.0, 0.0, 0.0]);
    for i in 0..6u32 {
        indices.extend_from_slice(&[bot_center, bot_center + ((i + 1) % 6), bot_center + i]);
    }
    for i in 0..6u32 {
        let i_next = (i + 1) % 6;
        let b0 = bot_start + i; let b1 = bot_start + i_next;
        let t0 = center_idx + 1 + i; let t1 = center_idx + 1 + i_next;
        indices.extend_from_slice(&[b0, b1, t1, b0, t1, t0]);
    }
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new("Vertex_Position", 0, bevy::render::mesh::VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[path = "world/map_generator.rs"]
mod map_generator;
mod idle;
mod player;
// ... existing imports
use crate::voice::mod; // <-- New: Import the voice module
use bevy::prelude::*;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::render::mesh::Mesh3d;
use std::time::{SystemTime, UNIX_EPOCH};

// --- IdleBot Modules ---
mod map_generator;
mod idle;
mod player;
mod progression;
mod vehicle;

// --- Voice Chat Modules (New/Migrated) ---
pub mod voice {
    pub mod indicator;
    pub mod ui;
    pub mod update;
}

// --- Main Entry ---
fn main() {
    eprintln!("=== IdleBot Starting ===");
    App::new()
        .add_plugins(DefaultPlugins)
        // Initialize voice module dependencies
        .add_plugins(voice::mod::VoicePlugin) 
        // Startup Phase: Set up graphics and initialize voice UI
        .add_systems(Startup, (setup, spawn_world, voice::ui::setup_voice_ui("LocalPlayer")))
        // Update Phase: Handle player input, run game logic, and update voice state
        .add_systems(Update, (player_movement, debug_commands, voice::update::voice_indicator_updater))
        .run();
}

// ... (rest of main.rs content unchanged)

// ... rest of file unchanged

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[SETUP] >>> STARTING SETUP <<<");
    
    // Lighting: Ambient + Directional + Point
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.7, 0.8),
        brightness: 400.0,
        ..default()
    });
    eprintln!("[SETUP] Added ambient light");
    
    commands.spawn((
        DirectionalLight { illuminance: 20000.0, shadow_maps_enabled: true, ..default() },
        Transform { translation: Vec3::new(0.0, 30.0, 0.0), rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4), ..default() },
    ));
    eprintln!("[SETUP] Added directional sun");
    
    commands.spawn((
        PointLight { intensity: 80_000.0, color: Color::srgb(1.0, 0.9, 0.7), shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(20.0, 15.0, 20.0),
    ));
    eprintln!("[SETUP] Added point light");
    
    // 3D Camera with NO tonemapping (fixes pink rendering)
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(0.0, 40.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    eprintln!("[SETUP] Camera: Tonemapping::None");
    
    // Spawn the player (teal capsule shape)
    let player_mesh = meshes.add(create_player_mesh());
    let player_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.8, 0.8, 1.0),
        emissive: LinearRgba::new(0.0, 0.4, 0.4, 0.0),
        ..default()
    });
    commands.spawn((
        Name::new("player"),
        player::ClientPlayer::new_spawn(None, Vec3::new(0.0, 0.0, 0.0), 1, 0, 100, 0, Vec::new(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
        Mesh3d(player_mesh),
        MeshMaterial3d(player_material),
        Transform::from_xyz(0.0, 1.5, 0.0),
    ));
    eprintln!("[SETUP] Player spawned at (0, 1.5, 0)");
}

/// Spawn the hex world.
fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[WORLD] Spawning hex world...");
    let hexes = crate::map_generator::generate_hex_map(&mut rand::thread_rng());
    let hex_mesh_handle = meshes.add(create_hex_mesh(1.0));
    
    for hex in &hexes {
        let base_color = match hex.terrain {
            crate::map_generator::TerrainType::Grass => Color::srgba(0.3, 0.8, 0.3, 1.0),
            crate::map_generator::TerrainType::Forest => Color::srgba(0.1, 0.5, 0.2, 1.0),
            crate::map_generator::TerrainType::Water => Color::srgba(0.2, 0.5, 0.9, 1.0),
            crate::map_generator::TerrainType::City => Color::srgba(0.6, 0.6, 0.6, 1.0),
            crate::map_generator::TerrainType::Desert => Color::srgba(0.9, 0.8, 0.4, 1.0),
            crate::map_generator::TerrainType::Polluted => Color::srgba(0.5, 0.3, 0.6, 1.0),
        };
        let material = materials.add(StandardMaterial { base_color, perceptual_roughness: 0.8, ..default() });
        commands.spawn((
            Name::new(format!("hex_{}_{}", hex.q, hex.r)),
            Transform::from_xyz(hex.center_x, 0.0, hex.center_y),
            Mesh3d(hex_mesh_handle.clone()),
            MeshMaterial3d(material),
        ));
    }
    eprintln!("[WORLD] Spawned {} hex tiles", hexes.len());
}

/// Player movement system — WASD input with smooth acceleration/deceleration
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        eprintln!("[ERROR] No player found!");
        return;
    };
    if player.current_hex.is_none() {
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
    }
    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.y -= 1.0; eprintln!("[INPUT] W"); }
    if keyboard.pressed(KeyCode::KeyS) { input.y += 1.0; eprintln!("[INPUT] S"); }
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; eprintln!("[INPUT] A"); }
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; eprintln!("[INPUT] D"); }
    if input.length() > 0.0 { input = input.normalize(); }
    let vehicle = player.owned_vehicle.clone();
    let speed_multiplier = vehicle.map_or(1.0, |v| v.speed_multiplier());
    let max_speed = 10.0 * speed_multiplier;
    let acceleration = 50.0;
    let friction = 10.0;
    let dt = time.delta_secs();
    let was_moving = player.velocity.length() > 0.01;
    if input.length() > 0.0 {
        let target_velocity = input * max_speed;
        let mut new_vel = player.velocity + (target_velocity - player.velocity) * (acceleration * dt).min(1.0);
        new_vel = new_vel.clamp_length_max(max_speed);
        player.velocity = new_vel;
        eprintln!("[MOVE] ACCEL in=({:.1},{:.1}) vel=({:.2},{:.2})", input.x, input.y, player.velocity.x, player.velocity.y);
    } else if !was_moving { }
    else {
        let mut new_vel = player.velocity * (1.0 - friction * dt).max(0.0);
        if new_vel.length() < 0.01 { new_vel = Vec2::ZERO; eprintln!("[MOVE] STOPPED"); }
        player.velocity = new_vel;
    }
    let delta = player.velocity * dt;
    if delta.length() > 0.001 {
        let old_pos = transform.translation;
        let new_pos = Vec3::new(old_pos.x + delta.x, old_pos.y, old_pos.z + delta.y);
        transform.translation = new_pos;
        player.position = new_pos;
        let (q, r) = idlecore_client::world_pos_to_hex(transform.translation.x, transform.translation.z, 10.0);
        player.current_hex = Some(player::CurrentHex { q, r });
        eprintln!("[MOVE] pos=({:.1},{:.1},{:.1}) hex=({},{}) vel=({:.2},{:.2})", transform.translation.x, transform.translation.y, transform.translation.z, q, r, player.velocity.x, player.velocity.y);
    }
}

/// Debug commands
fn debug_commands(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else { return; };
    if keyboard.just_pressed(KeyCode::Numpad0) || keyboard.just_pressed(KeyCode::Digit0) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        eprintln!("[DEBUG] Reset to spawn");
    }
    if keyboard.just_pressed(KeyCode::KeyV) { eprintln!("[DEBUG] Vehicle: {:?}", player.owned_vehicle); }
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        eprintln!("[DEBUG] Position reset");
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let seconds_offline = now.saturating_sub(player.last_login_time);
        if seconds_offline > 60 {
            eprintln!("[DEBUG] Offline: {}s, applying idle gains", seconds_offline);
            let gains = if seconds_offline < 3600 { (10, 5) }
                else if seconds_offline < 21600 { (60, 30) }
                else if seconds_offline < 43200 { (100, 50) }
                else { (150, 75) };
            player.xp += gains.0;
            player.gold += gains.1;
            player.level = crate::progression::calculate_level(player.xp);
            eprintln!("[DEBUG] Applied: +{} XP, +{} Gold. Level: {}", gains.0, gains.1, player.level);
        }
        player.last_login_time = now;
    }
}
