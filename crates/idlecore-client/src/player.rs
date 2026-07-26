//! Player component and spawning
//!
//! Orange tetrahedron avatar as placeholder for the Tamagotchi character.
//! Tracks position, velocity, hex, gold, XP, level, eco points, vehicle, cosmetics, and last login.

use bevy::prelude::*;
use idlebot_core::Vehicle;
use idlebot_core::Vehicle as CoreVehicle;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::progression;

/// Marker for spawn point (visible indicator at world center)
#[derive(Component)]
pub struct SpawnMarker;

/// Convert world position to axial hex coordinates (q, r)
/// Matching the coordinate system: hex center at (sqrt(3)*(q + r/2), 1.5*r)
pub fn world_pos_to_hex(world_x: f32, world_z: f32, hex_radius: f32) -> (i32, i32) {
    let sq3 = std::f32::consts::SQRT_3;
    let r_approx = (world_z / (1.5 * hex_radius)) as i32;
    let q_approx = ((world_x / (sq3 * hex_radius)) - (r_approx as f32) / 2.0) as i32;

    let fq = q_approx as f64;
    let fr = r_approx as f64;
    let fs = -(fq + fr);

    // Round to nearest of the three directions (q, r, or s)
    let dq = (fq - fr).abs();
    let dr = (fq - fs).abs();
    let ds = (fr - fs).abs();

    if dq > dr && dq > ds {
        // On q edge: project to q direction
        let sgn = fs.signum() as i32;
        (sgn * ((fs - fr) + fr), r_approx)
    } else if dr > ds {
        (q_approx, r_approx)
    } else {
        // On s edge
        let sgn = fs.signum() as i32;
        (q_approx, sgn * (fs - fr) + fr)
    }
}

/// Build a fresh ClientPlayer component
pub fn client_player_component(
    world_pos: Vec3,
    vehicle: Option<CoreVehicle>,
    xp: u64,
    gold: u64,
    eco_points: u64,
    equipped_cosmetics: Vec<String>,
) -> ClientPlayer {
    let level = progression::calculate_level(xp);
    let current_hex = Some(CurrentHex {
        q: world_pos_to_hex(world_pos.x, world_pos.z, 10.0).0,
        r: world_pos_to_hex(world_pos.x, world_pos.z, 10.0).1,
    });
    ClientPlayer {
        position: world_pos,
        velocity: Vec2::ZERO,
        current_hex,
        gold,
        xp,
        level,
        eco_points,
        owned_vehicle: vehicle,
        equipped_cosmetics,
        last_login_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        time_offline: None,
    }
}

/// Spawn player with orange tetrahedron at world center (0, 0, 0).
pub fn spawn_player(
    mut commands: Commands,
    vehicle: Option<CoreVehicle>,
    xp: u64,
    gold: u64,
    eco_points: u64,
    equipped_cosmetics: Vec<String>,
) -> Entity {
    let position = Vec3::ZERO;
    let level = progression::calculate_level(xp);

    let player_entity = commands.spawn((
        Name::new("player"),
        ClientPlayer::new_spawn(vehicle, position, level, xp, gold, eco_points, equipped_cosmetics),
        player_mesh(),
        Transform::from_xyz(position.x, position.y, position.z),
    ));

    // Small spawn marker
    commands.spawn((
        Name::new("spawn_marker"),
        Transform::from_xyz(position.x, 1.0, position.z),
    ));

    tracing::info!(
        "Player spawned at (0, 0, 0) with level {} and {} gold",
        level,
        gold
    );

    player_entity
}

/// Create the orange tetrahedron mesh for the player avatar
pub fn player_mesh() -> Mesh {
    let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);

    // Tetrahedron vertices (height 1.0, base side ~1.0)
    let vertices: [Vec3; 4] = [
        Vec3::new(0.0, 0.4, 0.0),       // apex
        Vec3::new(-0.433, -0.2, 0.2),   // bottom-left
        Vec3::new(0.433, -0.2, 0.2),    // bottom-right
        Vec3::new(0.0, -0.2, -0.4),     // back
    ];

    // 4 triangular faces
    let face_indices: [[u32; 3]; 4] = [
        [0, 1, 4], // front face (visible)
        [0, 1, 2], // left face
        [0, 2, 3], // right face
        [0, 3, 4], // back face
    ];

    let positions: Vec<[f32; 3]> = vertices.map(|v| v.into());
    let indices: Vec<u32> = face_indices.flat_map(|f| f).collect();

    // Colors: orange front, slightly darker back
    let orange = Color::srgb(1.0, 0.78, 0.2);
    let orange_back = Color::srgb(0.92, 0.65, 0.2);

    // Build MeshVertex arrays
    let mut vertices_data = Vec::new();
    for v in &vertices {
        vertices_data.push([v.x, v.y, v.z]);     // position
        vertices_data.push([0.0, 0.0, 1.0]);     // normal (pointing outward from base)
        vertices_data.push(orange_back);         // ambient color
        vertices_data.push(orange);              // diffuse color
    }

    mesh.set_attributes(
        bevy::render::mesh::MeshVertex::mesh_vertex([
            positions,
            vec![[0.0, 0.0, 1.0]; positions.len()],
            vec![orange_back; positions.len()],
            vec![orange; positions.len()],
        ]),
    );

    mesh.set_indices(Some(bevy::render::render_resource::IndexBuffer::new(
        bevy::render::render_resource::BufferSize::Size(indices.len() as u64 * 4),
        bevy::render::render_resource::IndexFormat::Uint32,
        indices,
    )));

    mesh
}

/// ClientPlayer with spawn defaults
pub fn new_spawn(
    vehicle: Option<CoreVehicle>,
    position: Vec3,
    level: u32,
    xp: u64,
    gold: u64,
    eco_points: u64,
    equipped_cosmetics: Vec<String>,
) -> ClientPlayer {
    let current_hex = Some(CurrentHex {
        q: world_pos_to_hex(position.x, position.z, 10.0).0,
        r: world_pos_to_hex(position.x, position.z, 10.0).1,
    });
    ClientPlayer {
        position,
        velocity: Vec2::ZERO,
        current_hex,
        gold,
        xp,
        level,
        eco_points,
        owned_vehicle: vehicle,
        equipped_cosmetics,
        last_login_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        time_offline: None,
    }
}
