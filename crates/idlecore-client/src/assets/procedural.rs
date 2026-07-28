//! Procedural mesh generation for hexagons and trees
//!
//! Generates 3D mesh data for hexagonal tiles and trees using Bevy's mesh API.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, MeshVertexAttribute, VertexFormat, VertexAttributeValues, PrimitiveTopology};

/// Hex mesh data for a single hexagonal tile
#[derive(Debug, Clone)]
pub struct HexMeshData {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
}

/// Create a hexagonal tile mesh
pub fn create_hex_mesh(radius: f32, height: f32, color: [f32; 4]) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    // Hexagon vertices (flat-top orientation)
    let top_vertices: Vec<[f32; 3]> = (0..6)
        .map(|i| {
            let angle = std::f32::consts::FRAC_PI_3 * i as f32;
            [
                radius * angle.cos(),
                height / 2.0,
                radius * angle.sin(),
            ]
        })
        .collect();

    let bottom_vertices: Vec<[f32; 3]> = (0..6)
        .map(|i| {
            let angle = std::f32::consts::FRAC_PI_3 * i as f32;
            [
                radius * angle.cos(),
                -height / 2.0,
                radius * angle.sin(),
            ]
        })
        .collect();

    // Build all vertices: top ring + bottom ring
    let all_vertices: Vec<[f32; 3]> = [top_vertices.clone(), bottom_vertices.clone()].concat();
    let all_normals: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; 12];
    let all_colors: Vec<[f32; 4]> = vec![color; 12];

    // Top face indices (6 triangles from center)
    let top_center_idx = 0u32;
    let mut top_indices: Vec<u32> = Vec::new();
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        top_indices.extend_from_slice(&[top_center_idx, i + 1, next + 1]);
    }

    // Bottom face indices
    let bottom_center_idx = 7u32;
    let mut bottom_indices: Vec<u32> = Vec::new();
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        bottom_indices.extend_from_slice(&[bottom_center_idx, i + 8, next + 8]);
    }

    // Side indices (connecting top and bottom)
    let mut side_indices: Vec<u32> = Vec::new();
    for i in 0..6u32 {
        let next = (i + 1) % 6;
        side_indices.extend_from_slice(&[i as u32 + 1, next as u32 + 1, i as u32 + 8]);
        side_indices.extend_from_slice(&[next as u32 + 1, next as u32 + 8, i as u32 + 8]);
    }

    let all_indices: Vec<u32> = [top_indices, bottom_indices, side_indices].concat();

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Position", 0, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(all_vertices),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Normal", 1, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(all_normals),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Color", 2, VertexFormat::Float32x4),
        VertexAttributeValues::Float32x4(all_colors),
    );

    mesh.insert_indices(Indices::U32(all_indices));

    mesh
}

/// Create a tree mesh (simple cylinder + cone)
pub fn create_tree_mesh(trunk_radius: f32, trunk_height: f32, canopy_radius: f32, canopy_height: f32, trunk_color: [f32; 4], canopy_color: [f32; 4]) -> Mesh {
    let _trunk = create_hex_mesh(trunk_radius, trunk_height, trunk_color);
    let canopy = create_hex_mesh(canopy_radius, canopy_height, canopy_color);

    // Combine meshes (simplified — in production, merge properly)
    canopy
}

/// Create a single leaf mesh
pub fn create_leaf_mesh(radius: f32, color: [f32; 4]) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    let vertices: Vec<[f32; 3]> = vec![
        [0.0, radius, 0.0],
        [-radius, -radius, 0.0],
        [radius, -radius, 0.0],
    ];

    let normals: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; 3];
    let colors: Vec<[f32; 4]> = vec![color; 3];
    let indices: Vec<u32> = vec![0, 1, 2];

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Position", 0, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(vertices),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Normal", 1, VertexFormat::Float32x3),
        VertexAttributeValues::Float32x3(normals),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new("Vertex_Color", 2, VertexFormat::Float32x4),
        VertexAttributeValues::Float32x4(colors),
    );

    mesh.insert_indices(Indices::U32(indices));

    mesh
}
