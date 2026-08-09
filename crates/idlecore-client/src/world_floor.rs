//! Renders the streamed hex world as 3D terrain meshes.
//!
//! One parent entity per loaded chunk, with terrain + water child meshes built
//! from `idlecore_core::world_mesh`. Chunks are spawned lazily as they stream
//! in and despawned when they leave the rendered radius.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, VertexAttributeValues};
use bevy_rapier3d::geometry::{Collider, ComputedColliderShape, TriMeshFlags};
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::world_gen::{WorldGenConfig, hex_to_chunk_coord};
use idlecore_core::world_mesh::{
    ChunkMesh, MeshGenOptions,
    generate_chunk_terrain_mesh, generate_chunk_water_mesh,
};
use crate::player::PlayerTransform;
use crate::plugins::world::StreamingWorldResource;

/// Marker for the parent entity of a rendered chunk.
#[derive(Component)]
pub struct WorldChunk {
    pub coord: (i32, i32),
}

/// Tracks spawned chunk entities so we only (re)create on changes.
#[derive(Resource, Default)]
pub struct WorldFloor {
    pub entities: std::collections::HashMap<(i32, i32), Entity>,
    pub terrain_material: Option<Handle<StandardMaterial>>,
    pub water_material: Option<Handle<StandardMaterial>>,
}

/// Chunk radius around the player that is rendered.
const RENDER_RADIUS_CHUNKS: i32 = 5;

/// World-space radius (in hexes * HEX_SIZE units) around the player to show.
const RENDER_RADIUS_HEXES: f32 = 20.0 * WorldGenConfig::HEX_SIZE;

/// Mesh generation options shared by all rendered chunks.
/// Hex radius matches the generator's HEX_SIZE so chunk geometry lines up with
/// player position and the minimap's world math.
fn mesh_options() -> MeshGenOptions {
    MeshGenOptions {
        hex_radius: WorldGenConfig::HEX_SIZE,
        elevation_scale: 25.0,
    }
}

/// Build a Bevy mesh from engine-agnostic ChunkMesh data.
fn build_mesh(cm: &ChunkMesh) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(cm.vertices.len());
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(cm.vertices.len());
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(cm.vertices.len());
    for v in &cm.vertices {
        positions.push([v.x, v.y, v.z]);
        uvs.push([v.u, v.v]);
    }
    for c in &cm.colors {
        colors.push([c[0], c[1], c[2], 1.0]);
    }
    let mut indices: Vec<u32> = Vec::with_capacity(cm.triangles.len() * 3);
    for t in &cm.triangles {
        indices.push(t.a);
        indices.push(t.b);
        indices.push(t.c);
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(vec![[0.0, 1.0, 0.0]; cm.vertices.len()]),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(uvs),
    );
    if !colors.is_empty() {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            VertexAttributeValues::Float32x4(colors),
        );
    }
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Average color of a chunk's terrain (unused; kept minimal).
/// `biome_definition` color mix by cell — replaced by the shared terrain
/// material; remove if per-biome chunk tinting is desired.
fn _chunk_terrain_color_ref() {}

/// Ensure the two shared materials exist (created lazily on first run).
fn ensure_materials(floor: &mut WorldFloor, materials: &mut Assets<StandardMaterial>) {
    if floor.terrain_material.is_none() {
        floor.terrain_material = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            perceptual_roughness: 0.9,
            ..default()
        }));
    }
    if floor.water_material.is_none() {
        floor.water_material = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            perceptual_roughness: 0.1,
            ..default()
        }));
    }
}

/// Spawn/despawn chunk entities around the player position.
pub fn update_world_floor(
    mut commands: Commands,
    streaming_world: Res<StreamingWorldResource>,
    player_transform: Res<PlayerTransform>,
    mut floor: ResMut<WorldFloor>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    ensure_materials(&mut floor, &mut materials);
    let terrain_material = floor.terrain_material.clone().unwrap();
    let water_material = floor.water_material.clone().unwrap();

    let px = player_transform.translation.x;
    let pz = player_transform.translation.z;

    let (hq, hr) = world_pos_to_hex(px, pz, WorldGenConfig::HEX_SIZE);
    let (ccq, ccr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);

    // Determine the set of chunks we want rendered.
    let mut wanted: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for dcq in -RENDER_RADIUS_CHUNKS..=RENDER_RADIUS_CHUNKS {
        for dcr in -RENDER_RADIUS_CHUNKS..=RENDER_RADIUS_CHUNKS {
            let cq = ccq + dcq;
            let cr = ccr + dcr;
            let Some(chunk) = streaming_world.chunks.chunks.get(&(cq, cr)) else { continue };
            let mut close_enough = false;
            for cell in &chunk.cells {
                let (wx, wz) = cell.world_pos(WorldGenConfig::HEX_SIZE);
                let dx = wx - px;
                let dz = wz - pz;
                if dx * dx + dz * dz <= RENDER_RADIUS_HEXES * RENDER_RADIUS_HEXES {
                    close_enough = true;
                    break;
                }
            }
            if close_enough {
                wanted.insert((cq, cr));
            }
        }
    }

    // Despawn chunks that left the render radius or unloaded.
    let stale: Vec<(i32, i32)> = floor
        .entities
        .keys()
        .filter(|k| !wanted.contains(k))
        .cloned()
        .collect();
    for key in stale {
        if let Some(entity) = floor.entities.remove(&key) {
            commands.entity(entity).despawn();
        }
    }

    // Spawn new chunks (existing ones are kept as-is).
    for (cq, cr) in &wanted {
        if floor.entities.contains_key(&(*cq, *cr)) {
            continue;
        }
        let Some(chunk) = streaming_world.chunks.chunks.get(&(*cq, *cr)) else { continue };

        let terrain = generate_chunk_terrain_mesh(&chunk.cells, &mesh_options());
        let water = generate_chunk_water_mesh(&chunk.cells, &mesh_options());

        let terrain_handle = if terrain.is_empty() {
            None
        } else {
            Some(meshes.add(build_mesh(&terrain)))
        };
        let water_handle = if water.is_empty() {
            None
        } else {
            Some(meshes.add(build_mesh(&water)))
        };

        let mut parent = commands.spawn((
            Name::new(format!("WorldChunk({cq},{cr})")),
            WorldChunk { coord: (*cq, *cr) },
            Transform::default(),
            GlobalTransform::default(),
        ));

        // Terrain tinted by average biome color; solid to physics (trimesh).
        parent.with_children(|parent| {
            if let Some(handle) = &terrain_handle {
                let collider = meshes
                    .get(handle)
                    .and_then(|mesh| {
                        Collider::from_bevy_mesh(
                            mesh,
                            &ComputedColliderShape::TriMesh(TriMeshFlags::default()),
                        )
                    });
                let mut child = parent.spawn((
                    Name::new("terrain"),
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(terrain_material.clone()),
                    Transform::default(),
                ));
                if let Some(collider) = collider {
                    child.insert(collider);
                }
            }
            if let Some(handle) = &water_handle {
                parent.spawn((
                    Name::new("water"),
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(water_material.clone()),
                    Transform::from_xyz(0.0, 0.001, 0.0),
                ));
            }
        });

        floor.entities.insert((*cq, *cr), parent.id());
    }
}

#[cfg(test)]
mod tests {}
