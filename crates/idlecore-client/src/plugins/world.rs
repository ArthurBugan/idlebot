//! World rendering plugin
//! Uses idlecore_core::hex_grid for proper hex coordinate conversion

use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use bevy::asset::Assets;
use bevy::render::mesh::Mesh;
use idlecore_core::hex_grid::HexGrid;
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
    let hex_mesh = create_hex_mesh(HEX_SCALE * 100.0, 10.0);
    let hex_mesh_handle = meshes.add(hex_mesh);
    
    for tile in world.tiles.values() {
        let biome_color = tile.biome.color();
        
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(biome_color.0, biome_color.1, biome_color.2, 1.0),
            perceptual_roughness: 0.8,
            unlit: true,
            ..default()
        });
        
        // Use proper hex grid coordinate conversion
        let (x, z) = HexGrid::axial_to_world(tile.coord.q, tile.coord.r, 100.0);
        
        commands.spawn((
            Name::new(format!("hex_{}_{}", tile.coord.q, tile.coord.r)),
            Transform::from_xyz(x, 0.0, z),
            Mesh3d(hex_mesh_handle.clone()),
            MeshMaterial3d(material),
        ));
    }
    eprintln!("[WORLD] Spawned {} tiles", world.tiles.len());
}

/// Create a hexagonal prism lying flat on the ground (XZ plane)
/// Y axis is up (height)
fn create_hex_mesh(radius: f32, height: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues};
    
    // Hexagon in XZ plane (flat on ground)
    let corners: Vec<[f32; 2]> = (0..6)
        .map(|i| {
            let angle = std::f32::consts::PI / 3.0 * i as f32;
            // [x, z] for flat-top hex
            [radius * angle.cos(), radius * angle.sin()]
        })
        .collect();
    
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    
    // Bottom face (ground level, Y=0)
    for &c in &corners {
        positions.push([c[0], 0.0, c[1]]);  // X, Y=0, Z
    }
    let bot_start = positions.len() as u32;
    let bot_center = bot_start + 6;
    positions.push([0.0, 0.0, 0.0]);  // Center bottom
    
    // Bottom face triangles
    for i in 0..6u32 {
        indices.extend_from_slice(&[bot_center, bot_center + ((i + 1) % 6), bot_center + i]);
    }
    
    // Top face (Y=height)
    for &c in &corners {
        positions.push([c[0], height, c[1]]);  // X, Y=height, Z
    }
    let top_start = positions.len() as u32;
    let top_center = top_start + 6;
    positions.push([0.0, height, 0.0]);  // Center top
    
    // Top face triangles
    for i in 0..6u32 {
        indices.extend_from_slice(&[top_center, top_center + i, top_center + ((i + 1) % 6)]);
    }
    
    // Side faces
    for i in 0..6u32 {
        let i_next = (i + 1) % 6;
        let b0 = bot_start + i; let b1 = bot_start + i_next;
        let t1 = top_start + i; let t0 = top_start + i_next;
        indices.extend_from_slice(&[b0, b1, t0, b0, t0, t1]);
    }
    
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(
        bevy::render::mesh::MeshVertexAttribute::new("Vertex_Position", 0, bevy::render::mesh::VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
