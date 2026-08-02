//! Player component and spawning
//!
//! Orange tetrahedron avatar as placeholder for the Tamagotchi character.
//! Tracks position, velocity, hex, gold, XP, level, eco points, vehicle, cosmetics, and last login.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, MeshVertexAttribute, VertexFormat, VertexAttributeValues, PrimitiveTopology};
use bevy::ecs::prelude::Component;
use idlecore_core::Vehicle;

use crate::progression;

/// Marker for spawn point (visible indicator at world center)
#[derive(Component)]
pub struct SpawnMarker;

/// Hex coordinate pair (axial coordinates q, r)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentHex {
    pub q: i32,
    pub r: i32,
}

/// Main player component — tracks all player state
#[derive(Component)]
pub struct ClientPlayer {
    pub position: Vec3,
    pub velocity: Vec2,
    pub current_hex: Option<CurrentHex>,
    pub gold: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    pub owned_vehicle: Option<Vehicle>,
    pub equipped_cosmetics: Vec<String>,
    pub last_login_time: u64,
    pub time_offline: Option<u64>,
}

impl ClientPlayer {
    /// Create a new ClientPlayer at spawn with defaults
    pub fn new_spawn(
        vehicle: Option<Vehicle>,
        position: Vec3,
        level: u32,
        xp: u64,
        gold: u64,
        eco_points: u64,
        equipped_cosmetics: Vec<String>,
        last_seen: u64,
    ) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            current_hex: Some(CurrentHex { q: 0, r: 0 }),
            gold,
            xp,
            level,
            eco_points,
            owned_vehicle: vehicle,
            equipped_cosmetics,
            last_login_time: last_seen,
            time_offline: None,
        }
    }

    /// Set the player's last seen timestamp
    pub fn set_last_seen(&mut self, seconds: u64) {
        self.last_login_time = seconds;
    }
}

/// Spawn player with orange tetrahedron at world center (0, 0, 0).
pub fn spawn_player(
    mut commands: Commands,
    vehicle: Option<Vehicle>,
    xp: u64,
    gold: u64,
    eco_points: u64,
    equipped_cosmetics: Vec<String>,
) -> Entity {
    let position = Vec3::ZERO;
    let level = progression::calculate_level(xp);

    let player_entity = commands.spawn((
        Name::new("player"),
        ClientPlayer::new_spawn(vehicle, position, level, xp, gold, eco_points, equipped_cosmetics, 0),
        Mesh3d::default(),
        Transform::from_xyz(position.x, position.y, position.z),
    ));

    tracing::info!(
        "Player spawned at (0, 0, 0) with level {} and {} gold",
        level,
        gold
    );

    player_entity.id()
}

/// Create the orange tetrahedron mesh for the player avatar
pub fn player_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    // Tetrahedron vertices (height 1.0, base side ~1.0)
    let vertices: [Vec3; 4] = [
        Vec3::new(0.0, 0.4, 0.0),
        Vec3::new(-0.433, -0.2, 0.2),
        Vec3::new(0.433, -0.2, 0.2),
        Vec3::new(0.0, -0.2, -0.4),
    ];

    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
    let normals: Vec<[f32; 3]> = vec![[0.0, 0.0, 1.0]; 4];

    let face_indices: [[u32; 3]; 4] = [
        [0, 1, 2],
        [0, 2, 3],
        [0, 3, 1],
        [1, 3, 2],
    ];

    // Flatten indices
    let mut indices_vec: Vec<u32> = Vec::new();
    for f in &face_indices {
        indices_vec.extend_from_slice(f);
    }

    // Colors
    let orange: [f32; 4] = [1.0, 0.78, 0.2, 1.0];
    let orange_back: [f32; 4] = [0.92, 0.65, 0.2, 1.0];
    let colors: Vec<[f32; 4]> = vec![orange, orange_back, orange, orange_back];

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Position", 0, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(positions),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Normal", 1, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(normals),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Color", 2, VertexFormat::Float32x4),
        VertexAttributeValues::Float32x4(colors),
    );

    mesh.insert_indices(Indices::U32(indices_vec));

    mesh
}

/// Player transform resource for camera/minimap follow
#[derive(Resource, Default)]
pub struct PlayerTransform {
    pub translation: Vec3,
}
