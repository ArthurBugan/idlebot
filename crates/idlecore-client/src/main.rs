//! IdleBot — Bevy 0.19 hex grid single-player client.
//!
//! Main entry point: start the Bevy app with hex world, player, WASD movement,
//! idle gains, vehicle system, and idle time tracking.

use bevy::prelude::*;

use bevy::pbr::StandardMaterial;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::asset::Assets;
use bevy::render::mesh::Mesh;
use rand::Rng;

// Re-export from lib
pub use idlecore_client::world_pos_to_hex;

/// Create a simple orange box mesh (0.8 x 0.6 x 0.8)
fn create_box_mesh() -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let positions = vec![
        // Front face
        [-0.4, -0.3, 0.4], [0.4, -0.3, 0.4], [0.4, 0.3, 0.4], [-0.4, 0.3, 0.4],
        // Back face
        [0.4, -0.3, -0.4], [-0.4, -0.3, -0.4], [-0.4, 0.3, -0.4], [0.4, 0.3, -0.4],
        // Top face
        [-0.4, 0.3, 0.4], [0.4, 0.3, 0.4], [0.4, 0.3, -0.4], [-0.4, 0.3, -0.4],
        // Bottom face
        [-0.4, -0.3, -0.4], [0.4, -0.3, -0.4], [0.4, -0.3, 0.4], [-0.4, -0.3, 0.4],
        // Right face
        [0.4, -0.3, 0.4], [0.4, -0.3, -0.4], [0.4, 0.3, -0.4], [0.4, 0.3, 0.4],
        // Left face
        [-0.4, -0.3, -0.4], [-0.4, -0.3, 0.4], [-0.4, 0.3, 0.4], [-0.4, 0.3, -0.4],
    ];
    let indices: Vec<u32> = (0..24).map(|i| (i / 4 * 4 + [0, 1, 2, 0, 2, 3][i % 4]) as u32).collect();

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        default(),
    );
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new(
            "Vertex_Position",
            0,
            bevy::render::mesh::VertexFormat::Float32x3,
        ),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a flat-top hexagonal prism (radius, height)
fn create_hex_mesh(radius: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let h = 0.15; // hex tile height
    let sq3 = 1.732050808;

    // 6 corner vertices for top and bottom faces
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

    // Top face (2 triangles from center)
    positions.push([0.0, 0.0, h]);
    for &c in &top {
        positions.push(c);
    }
    let center_idx = positions.len() as u32 - 7;
    for i in 0..6u32 {
        indices.extend_from_slice(&[center_idx, center_idx + i + 1, center_idx + ((i + 1) % 6) + 1]);
    }

    // Bottom face
    let bot_start = positions.len() as u32;
    for &c in &bottom {
        positions.push(c);
    }
    let bot_center = bot_start + 6;
    positions.push([0.0, 0.0, 0.0]);
    for i in 0..6u32 {
        indices.extend_from_slice(&[bot_center, bot_center + ((i + 1) % 6), bot_center + i]);
    }

    // Side faces
    for i in 0..6u32 {
        let i_next = (i + 1) % 6;
        let b0 = bot_start + i;
        let b1 = bot_start + i_next;
        let t0 = center_idx + 1 + i; // top corners started after center
        let t1 = center_idx + 1 + i_next;
        // Two triangles per side
        indices.extend_from_slice(&[b0, b1, t1, b0, t1, t0]);
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        default(),
    );
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new(
            "Vertex_Position",
            0,
            bevy::render::mesh::VertexFormat::Float32x3,
        ),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[path = "world/map_generator.rs"]
mod map_generator;

mod player;
mod idle;
mod input;
mod vehicle;
mod progression;

/// 3D camera height for looking at the hex grid
const CAMERA_HEIGHT: f32 = 30.0;
const CAMERA_Y: f32 = 30.0;

/// Main function — start the Bevy app.
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, setup)
        .add_systems(Update, (
            player_movement,
            debug_commands,
        ))
        .add_systems(PostStartup, spawn_world)
        .run();
}

/// Startup: spawn camera, light, and player.
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Transform::from_xyz(0.0, CAMERA_HEIGHT, CAMERA_Y).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("main_camera"),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
        Name::new("sun_light"),
    ));

    // Spawn the player at world center — a bright orange box
    let player_mesh = meshes.add(create_box_mesh());
    let player_material = materials.add(
        StandardMaterial::from_color(Color::linear_rgba(1.0, 0.65, 0.1, 1.0)),
    );
    commands.spawn((
        Name::new("player"),
        Mesh3d(player_mesh),
        MeshMaterial3d(player_material),
        Transform::from_xyz(0.0, 0.4, 0.0),
    ));
    println!("[INFO] Player spawned at (0, 0.4, 0)");
}

/// Spawn the hex world after the window is ready.
fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();

    // Generate hex map
    let mut rng = rand::thread_rng();
    let hexes = crate::map_generator::generate_hex_map(&mut rng);
    let hex_radius = 1.0;

    // Create hex meshes and materials
    let hex_mesh_handle = meshes.add(create_hex_mesh(hex_radius));

    for hex in &hexes {
        let terrain_color = hex.terrain.color();
        let material = materials.add(StandardMaterial::from_color(
            Color::srgb(terrain_color[0], terrain_color[1], terrain_color[2]),
        ));

        commands.spawn((
            Name::new(format!("hex_{}_{}", hex.q, hex.r)),
            Transform::from_xyz(hex.center_x, 0.0, hex.center_y),
            Mesh3d(hex_mesh_handle.clone()),
            MeshMaterial3d(material),
        ));
    }

    println!("World ready with {} hexes", hexes.len());
}

/// Player movement system — WASD input with vehicle speed multipliers.
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
    };

    // Initialize current hex if not set
    if player.current_hex.is_none() {
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
    }

    // Gather WASD input
    let mut vx = 0.0f32;
    let mut vz = 0.0f32;

    if keyboard.pressed(KeyCode::KeyW) {
        vz -= 1.0;
    } else if keyboard.pressed(KeyCode::KeyS) {
        vz += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        vx -= 1.0;
    } else if keyboard.pressed(KeyCode::KeyD) {
        vx += 1.0;
    }

    // Get vehicle speed multiplier
    let vehicle = player.owned_vehicle.clone();
    let speed_multiplier = vehicle.map_or(1.0, |v| v.speed_multiplier());
    let base_speed = 10.0;
    let speed = base_speed * speed_multiplier;

    // Normalize movement direction
    let len = (vx * vx + vz * vz).sqrt();
    if len > 0.0 {
        vx /= len;
        vz /= len;
    }

    // Calculate delta position
    let dt = time.delta_secs();
    if len > 0.0 {
        let delta = speed * dt;
        let move_x = vx * delta;
        let move_z = vz * delta;

        // Clamp movement to prevent tunneling
        let actual_delta_x = move_x.clamp(-20.0, 20.0);
        let actual_delta_z = move_z.clamp(-20.0, 20.0);

        let old_pos = transform.translation;
        let new_pos = Vec3::new(
            old_pos.x + actual_delta_x,
            old_pos.y,
            old_pos.z + actual_delta_z,
        );
        transform.translation = new_pos;
    }

    // Update hex tracking and velocity
    let (q, r) = crate::world_pos_to_hex(
        transform.translation.x,
        transform.translation.z,
        10.0,
    );
    player.current_hex = Some(player::CurrentHex { q, r });
    player.velocity = Vec2::new(vx * speed * dt, vz * speed * dt);
    player.position = transform.translation;

    // Zero velocity when not moving
    if len == 0.0 {
        player.velocity = Vec2::ZERO;
    }
}

/// Debug commands for the single-player local version
fn debug_commands(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
    };

    // 0 or key 0 — reset to spawn point
    if keyboard.just_pressed(KeyCode::Numpad0) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        println!("[DEBUG] Reset to spawn point");
    }

    // V — toggle vehicle info
    if keyboard.just_pressed(KeyCode::KeyV) {
        println!("Current vehicle: {:?}", player.owned_vehicle);
    }

    // R — reset position
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        println!("[DEBUG] Position reset");
    }

    // L — apply idle gains (simulate login after offline time)
    if keyboard.just_pressed(KeyCode::KeyL) {
        let now = now()
        let last_login = player.last_login_time;
        let seconds_offline = now.saturating_sub(last_login);

        if seconds_offline > 60 {
            println!(
                "[DEBUG] Offline time: {}s, applying idle gains",
                seconds_offline
            );

            // Apply idle gains manually
            let gains = if seconds_offline < 3600 {
                (10, 5)
            } else if seconds_offline < 21600 {
                (60, 30)
            } else if seconds_offline < 43200 {
                (100, 50)
            } else {
                (150, 75)
            };

            player.xp += gains.0;
            player.gold += gains.1;
            player.level = crate::progression::calculate_level(player.xp);

            println!(
                "[DEBUG] Applied: +{} XP, +{} Gold. New level: {}",
                gains.0, gains.1, player.level
            );
        } else {
            println!("[DEBUG] Not enough offline time for idle gains (need > 60s)");
        }

        // Update last login time
        player.last_login_time = now;
    }
}
