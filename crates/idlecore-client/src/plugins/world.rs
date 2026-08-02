//! World rendering plugin
//! Handles spawning the hex world

use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use bevy::asset::Assets;
use bevy::render::mesh::Mesh;
use idlecore_core::world::EarthWorld;

/// Scale factor for hex tiles (0.95 = 5% gap between tiles)
const HEX_SCALE: f32 = 0.95;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_world);
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    eprintln!("[WORLD] Spawning Earth world...");
    
    let world = EarthWorld::generate(42, 50);
    let hex_mesh = create_flat_top_hex_mesh(HEX_SCALE * 100.0);
    let hex_mesh_handle = meshes.add(hex_mesh);
    
    for tile in world.tiles.values() {
        let biome_color = tile.biome.color();
        
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(biome_color.0, biome_color.1, biome_color.2, 1.0),
            perceptual_roughness: 0.8,
            unlit: true,
            ..default()
        });
        
        commands.spawn((
            Name::new(format!("hex_{}_{}", tile.coord.q, tile.coord.r)),
            Transform::from_xyz(tile.center_x, 0.0, tile.center_y),
            Mesh3d(hex_mesh_handle.clone()),
            MeshMaterial3d(material),
        ));
    }
    eprintln!("[WORLD] Spawned {} tiles", world.tiles.len());
}

/// Create a flat-top hexagonal prism (horizontal, like game hex tiles)
fn create_flat_top_hex_mesh(radius: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let h = 10.0;
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
    
    // Top face
    positions.push([0.0, 0.0, h]);
    for &c in &top { positions.push(c); }
    let center_idx = positions.len() as u32 - 7;
    for i in 0..6u32 {
        indices.extend_from_slice(&[center_idx, center_idx + i + 1, center_idx + ((i + 1) % 6) + 1]);
    }
    
    // Bottom face
    let bot_start = positions.len() as u32;
    for &c in &bottom { positions.push(c); }
    let bot_center = bot_start + 6;
    positions.push([0.0, 0.0, 0.0]);
    for i in 0..6u32 {
        indices.extend_from_slice(&[bot_center, bot_center + ((i + 1) % 6), bot_center + i]);
    }
    
    // Side faces
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
