//! Hex grid system — generates and manages the hex tile world.
//! Bevy 0.19 — flat-top hexagons with terrain colors.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::hex::Hex;
use crate::hex_tile::HexTile;
use crate::terrain::TerrainType;

/// All hexes stored by hex_id (u64).
#[derive(Component, Default)]
pub struct WorldGrid {
    pub hexes: HashMap<u64, HexTile>,
}

/// Spawns a full hex grid and bevy entities for each tile.
pub fn spawn_world_grid(
    hex_radius: f32,
    map_radius: i32,
    seed: u64,
    mut commands: Commands,
    mut grid: Local<WorldGrid>,
) {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);

    let mut hex_entities = Vec::new();

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let s = -q - r;
            if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                let hex_id = (q as u64).wrapping_shl(32) | (r as u64 & 0xFFFFFFFF);
                let hex = Hex::new(q, r, s);
                let center = hex.center(hex_radius);

                let terrain = TerrainType::from_random(&mut rng);
                let tile = HexTile::new(center, terrain);

                grid.hexes.insert(hex_id, tile.clone());

                let entity = commands
                    .spawn((
                        Name::new(format!("hex_{q}_{r}")),
                        tile,
                        Transform::from_xyz(center.x, center.y, 0.0),
                        bevy::prelude::Visibility::default(),
                    ))
                    .id();

                hex_entities.push(entity);
            }
        }
    }

    let _ = (hex_entities, hex_radius, map_radius); // unused but ok
}

/// Spawn a small plant marker on a hex.
pub fn spawn_plant(
    commands: &mut Commands,
    parent: Entity,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Name::new("plant"),
            Transform::from_xyz(0.0, 1.5, 0.0),
        ))
        .with_children(|builder| {
            builder.spawn((
                Name::new("plant_mesh"),
                Mesh3d(build_cylinder_mesh(0.3, 0.6)),
                MeshMaterial3d(
                    commands
                        .insert_resource(MaterialMeshBundle {
                            mesh: Mesh3d(build_cylinder_mesh(0.3, 0.6)),
                            material: color.into(),
                            transform: Transform::from_xyz(0.0, 0.0, 0.0),
                            ..Default::default()
                        })
                        .0,
                ),
            ));
        })
        .id()
}

/// Spawn a pollution marker (dark spiky cluster) on a hex.
pub fn spawn_pollution(
    commands: &mut Commands,
    parent: Entity,
) -> Entity {
    commands
        .spawn((
            Name::new("pollution"),
            Transform::from_xyz(0.0, 0.8, 0.0),
        ))
        .with_children(|builder| {
            builder.spawn((
                Name::new("pollution_mesh"),
                Mesh3d(build_spiky_mesh(0.8)),
                MeshMaterial3d(
                    commands
                        .insert_resource(MaterialMeshBundle {
                            mesh: Mesh3d(build_spiky_mesh(0.8)),
                            material: Color::srgb(0.3, 0.0, 0.1).into(),
                            transform: Transform::from_xyz(0.0, 0.0, 0.0),
                            ..Default::default()
                        })
                        .0,
                ),
            ));
        })
        .id()
}

/// Build a simple cylinder mesh for plants.
fn build_cylinder_mesh(radius: f32, height: f32) -> Mesh {
    let sides = 8;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Bottom face
    positions.push([0.0, -height / 2.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);

    // Top face
    positions.push([0.0, height / 2.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);

    // Side vertices
    for i in 0..sides {
        let angle = 2.0 * std::f32::consts::PI * i as f32 / sides as f32;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, -height / 2.0, z]);
        positions.push([x, height / 2.0, z]);
        let nx = angle.cos();
        let nz = angle.sin();
        normals.push([nx, 0.0, nz]);
        normals.push([nx, 0.0, nz]);
    }

    let center_bottom = 0u32;
    let center_top = 1u32;
    for i in 2..sides as u32 + 2 {
        indices.push(center_bottom);
        indices.push(i);
        indices.push((i + 1) % (sides as u32 + 2));
    }
    for i in 2..sides as u32 + 2 {
        indices.push(center_top);
        indices.push((i + 1) % (sides as u32 + 2));
        indices.push(i);
    }

    // Side faces
    for i in 0..sides as u32 {
        let v0 = 2 + i * 2;
        let v1 = 2 + i * 2 + 1;
        let v2 = 2 + ((i + 1) % sides) * 2;
        let v3 = 2 + ((i + 1) % sides) * 2 + 1;
        indices.push(v0);
        indices.push(v2);
        indices.push(v1);
        indices.push(v1);
        indices.push(v2);
        indices.push(v3);
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
    );
    mesh.insert_attribute(
        bevy::render::mesh::Mesh::POSITION_ATTRIBUTE,
        positions,
    );
    mesh.insert_attribute(
        bevy::render::mesh::Mesh::NORMAL_ATTRIBUTE,
        normals,
    );
    mesh.set_indices(Some(
        bevy::render::render_resource::Indices::U32(indices),
    ));

    mesh
}

/// Build a spiky mesh for pollution markers.
fn build_spiky_mesh(radius: f32) -> Mesh {
    let spikes = 12;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Base circle
    for i in 0..spikes {
        let angle = 2.0 * std::f32::consts::PI * i as f32 / spikes as f32;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
    }

    // Center top point
    positions.push([0.0, radius * 1.5, 0.0]);
    normals.push([0.0, 1.0, 0.0]);

    let center_top = spikes as u32;
    for i in 0..spikes as u32 {
        indices.push(i);
        indices.push((i + 1) % spikes as u32);
        indices.push(center_top);
    }

    // Side spikes
    for i in 0..spikes as u32 {
        let v0 = i;
        let v1 = (i + 1) % spikes as u32;
        let apex = spikes as u32 + i;
        positions.push([
            1.5 * radius * (2.0 * std::f32::consts::PI * i as f32 / spikes as f32).cos(),
            radius * 2.0,
            1.5 * radius * (2.0 * std::f32::consts::PI * i as f32 / spikes as f32).sin(),
        ]);
        normals.push([0.0, 1.0, 0.0]);

        let v2 = spikes as u32 + i + 1;
        if v2 < spikes as u32 + spikes as u32 {
            indices.push(v0);
            indices.push(v2);
            indices.push(v1);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
    );
    mesh.insert_attribute(
        bevy::render::mesh::Mesh::POSITION_ATTRIBUTE,
        positions,
    );
    mesh.insert_attribute(
        bevy::render::mesh::Mesh::NORMAL_ATTRIBUTE,
        normals,
    );
    mesh.set_indices(Some(
        bevy::render::render_resource::Indices::U32(indices),
    ));

    mesh
}

/// Bevy resource to hold the grid for lookup.
#[derive(Resource, Default)]
pub struct GridResource(pub HashMap<u64, HexTile>);
