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
    let hex_mesh = create_hex_mesh(HEX_SCALE * 100.0);
    let hex_mesh_handle = meshes.add(hex_mesh);
    
    for tile in world.tiles.values() {
        let biome_color = tile.biome.color();
        
        // Add subtle border effect with darker color
        let border_color = (
            biome_color.0 * 0.7,
            biome_color.1 * 0.7,
            biome_color.2 * 0.7,
        );
        
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(biome_color.0, biome_color.1, biome_color.2, 1.0),
            perceptual_roughness: 0.8,
            unlit: true,
            ..default()
        });
        
        // Create border material (slightly darker)
        let border_material = materials.add(StandardMaterial {
            base_color: Color::srgba(border_color.0, border_color.1, border_color.2, 1.0),
            perceptual_roughness: 0.9,
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

/// Create a flat-top hexagonal prism with rounded edges
fn create_hex_mesh(radius: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    let h = 50.0;
    
    // Create hexagon with slightly rounded corners (8 segments per edge)
    let segments_per_edge = 8;
    let total_segments = 6 * segments_per_edge;
    
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    
    // Generate hexagon vertices with rounded corners
    for i in 0..total_segments {
        let angle = std::f32::consts::TAU * i as f32 / total_segments as f32;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        
        // Top vertices
        positions.push([x, z, h]);
        // Bottom vertices
        positions.push([x, z, 0.0]);
    }
    
    // Center top vertex
    positions.push([0.0, 0.0, h]);
    let center_top = positions.len() as u32 - 1;
    
    // Center bottom vertex
    positions.push([0.0, 0.0, 0.0]);
    let center_bottom = positions.len() as u32 - 1;
    
    // Top face (center to perimeter)
    for i in 0..total_segments {
        let next = (i + 1) % total_segments;
        indices.extend_from_slice(&[
            center_top,
            i as u32 * 2 + 1,
            next as u32 * 2 + 1,
        ]);
    }
    
    // Bottom face (center to perimeter)
    for i in 0..total_segments {
        let next = (i + 1) % total_segments;
        indices.extend_from_slice(&[
            center_bottom,
            next as u32 * 2,
            i as u32 * 2,
        ]);
    }
    
    // Side faces
    for i in 0..total_segments {
        let next = (i + 1) % total_segments;
        let i2 = i * 2;
        let next2 = next * 2;
        
        // Quad: bottom_i -> bottom_next -> top_next -> top_i
        indices.extend_from_slice(&[
            i2,
            next2,
            next2 + 1,
            i2,
            next2 + 1,
            i2 + 1,
        ]);
    }
    
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new("Vertex_Position", 0, bevy::render::mesh::VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
