//! World rendering plugin

use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use bevy::asset::Assets;
use bevy::render::mesh::Mesh;
use idlecore_core::hex_grid::HexGrid;
use idlecore_core::world::EarthWorld;

/// Resource to store the world data for minimap access
#[derive(Resource)]
pub struct WorldResource {
    pub world: EarthWorld,
}

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
    
    // Store world as resource for minimap access
    commands.insert_resource(WorldResource { world });
    
    // Hex radius = grid size for perfect fit in flat-top grid
    let hex_mesh = create_flat_hex_mesh(150.0, 15.0);
    let hex_mesh_handle = meshes.add(hex_mesh);
    
    // We need to re-access the world since we moved it into the resource
    // For now, just regenerate it (in production, store it once)
    let world_ref = EarthWorld::generate(42, 50);
    
    for tile in world_ref.tiles.values() {
        let biome_color = tile.biome.color();
        
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(biome_color.0, biome_color.1, biome_color.2, 1.0),
            perceptual_roughness: 0.8,
            unlit: true,
            ..default()
        });
        
        let (x, z) = HexGrid::axial_to_world(tile.coord.q, tile.coord.r, 150.0);
        
        commands.spawn((
            Name::new(format!("hex_{}_{}", tile.coord.q, tile.coord.r)),
            Transform::from_xyz(x, 0.0, z),
            Mesh3d(hex_mesh_handle.clone()),
            MeshMaterial3d(material),
        ));
    }
    eprintln!("[WORLD] Spawned {} tiles", world_ref.tiles.len());
}

/// Create flat-top hex mesh (lying on ground, XZ plane)
/// Flat-top means flat sides at top and bottom, corners at left and right
fn create_flat_hex_mesh(radius: f32, height: f32) -> Mesh {
    use bevy::render::mesh::{Indices, VertexAttributeValues, PrimitiveTopology};
    
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    
    // Flat-top hex: corners at 30°, 90°, 150°, 210°, 270°, 330°
    // This gives flat sides at top (90°) and bottom (270°)
    
    // Bottom face (Y=0)
    positions.push([0.0, 0.0, 0.0]);
    let bot_center = 0u32;
    
    for i in 0..6 {
        let angle = std::f32::consts::PI / 6.0 + std::f32::consts::TAU * i as f32 / 6.0;
        positions.push([radius * angle.cos(), 0.0, radius * angle.sin()]);
    }
    let bot_start = 1u32;
    
    // Bottom triangles (clockwise from below)
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        indices.push(bot_center);
        indices.push(bot_start + next);
        indices.push(bot_start + i);
    }
    
    // Top face (Y=height)
    positions.push([0.0, height, 0.0]);
    let top_center = positions.len() as u32 - 1;
    
    for i in 0..6 {
        let angle = std::f32::consts::PI / 6.0 + std::f32::consts::TAU * i as f32 / 6.0;
        positions.push([radius * angle.cos(), height, radius * angle.sin()]);
    }
    let top_start = positions.len() as u32;
    
    // Top triangles (counter-clockwise from above)
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        indices.push(top_center);
        indices.push(top_start + i);
        indices.push(top_start + next);
    }
    
    // Side faces
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        let b0 = bot_start + i;
        let b1 = bot_start + next;
        let t0 = top_start + i;
        let t1 = top_start + next;
        
        indices.push(b0);
        indices.push(t0);
        indices.push(b1);
        
        indices.push(b1);
        indices.push(t0);
        indices.push(t1);
    }
    
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
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
