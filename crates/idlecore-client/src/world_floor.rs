//! Renders the streamed hex world as 3D terrain meshes.
//!
//! One parent entity per loaded chunk, with terrain + water child meshes built
//! from `idlecore_core::world_mesh`. Chunks are spawned lazily as they stream
//! in and despawned when they leave the rendered radius.

use bevy::prelude::*;
use spacetimedb_sdk::Table;
use std::collections::HashMap;
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
pub struct WorldChunk;

/// Tracks spawned chunk entities so we only (re)create on changes.
#[derive(Resource, Default)]
pub struct WorldFloor {
    pub entities: std::collections::HashMap<(i32, i32), Entity>,
    pub terrain_material: Option<Handle<StandardMaterial>>,
    pub water_material: Option<Handle<StandardMaterial>>,
    /// Chunk coord of the last rebuilt render set; unchanged → skip the pass.
    pub last_player_chunk: Option<(i32, i32)>,
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

    // Perf: chunk membership only changes when the player crosses a chunk
    // boundary (streaming is gated the same way), so skip otherwise.
    if !floor.entities.is_empty() && Some((ccq, ccr)) == floor.last_player_chunk {
        return;
    }
    floor.last_player_chunk = Some((ccq, ccr));

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
            WorldChunk,
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

// ============================================================================
// Plant / Pollution Visuals (Spec 016 T2.4, Spec 004 T6.5)
// ============================================================================

/// Per-hex visual state cache for plants and pollution.
#[derive(Resource, Default)]
pub struct FloorPlantState {
    pub visuals: HashMap<u64, Entity>,
    pub stage: HashMap<u64, (bool, bool, i8)>,
    /// Last raw `plant` JSON per hex, so unchanged rows skip re-parsing.
    pub raw: HashMap<u64, String>,
    /// Last parsed plant descriptor per hex (maturity flips over time without
    /// the row changing, so the parse is cached but `mature` recomputed).
    pub parsed: HashMap<u64, ParsedPlant>,
}

/// Cached result of parsing a hex's `plant` JSON column.
#[derive(Clone)]
pub struct ParsedPlant {
    kind_name: String,
    mature_at: u64,
}


/// Root entity rendering a plant or pollution marker on one hex.
#[derive(Component)]
pub struct HexPlantVisual;

/// Spec 016 T4.6: per-plant-type young/mature colors. Unknown types fall
/// back to None (caller uses the default young/mature pair).
pub fn plant_type_color(plant_type: &str, mature: bool) -> Option<Color> {
    let (young, mature_c) = match plant_type {
        "Wheat" => ((0.35, 0.85, 0.4), (0.85, 0.9, 0.45)),
        "Corn" => ((0.3, 0.8, 0.25), (0.9, 0.85, 0.35)),
        "Sunflower" => ((0.55, 0.85, 0.25), (1.0, 0.85, 0.2)),
        "Tree" => ((0.15, 0.6, 0.2), (0.1, 0.5, 0.18)),
        "RareHerb" => ((0.45, 0.3, 0.9), (0.65, 0.5, 1.0)),
        _ => return None,
    };
    let (r, g, b) = if mature { mature_c } else { young };
    Some(Color::srgb(r, g, b))
}

/// Shared meshes/materials for plant visuals (built lazily).
#[derive(Resource, Default)]
pub struct FloorPlantAssets {
    pub plant_mats: HashMap<String, (Handle<StandardMaterial>, Handle<StandardMaterial>)>,
    pub pollution_mat: Option<Handle<StandardMaterial>>,
    pub lush_mat: Option<Handle<StandardMaterial>>,
    pub degraded_mat: Option<Handle<StandardMaterial>>,
    pub cone_mesh: Option<Handle<Mesh>>,
    pub tall_cone_mesh: Option<Handle<Mesh>>,
    pub disc_mesh: Option<Handle<Mesh>>,
    pub eco_disc_mesh: Option<Handle<Mesh>>,
}

const PLANT_TYPES: [&str; 5] = ["Wheat", "Corn", "Sunflower", "Tree", "RareHerb"];

fn ensure_plant_assets(
    assets: &mut FloorPlantAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if assets.plant_mats.is_empty() {
        for plant_type in PLANT_TYPES {
            let young = plant_type_color(plant_type, false).unwrap();
            let mature = plant_type_color(plant_type, true).unwrap();
            let pair = (
                materials.add(StandardMaterial::from_color(young)),
                materials.add(StandardMaterial::from_color(mature)),
            );
            assets.plant_mats.insert(plant_type.to_string(), pair);
        }
        assets.pollution_mat = Some(materials.add(StandardMaterial::from_color(Color::srgb(0.18, 0.2, 0.16))));
        assets.lush_mat = Some(materials.add(StandardMaterial::from_color(Color::srgba(0.2, 0.9, 0.35, 0.35))));
        assets.degraded_mat = Some(materials.add(StandardMaterial::from_color(Color::srgba(0.55, 0.35, 0.15, 0.3))));
        assets.cone_mesh = Some(meshes.add(Cone::new(0.8, 1.6)));
        assets.tall_cone_mesh = Some(meshes.add(Cone::new(0.9, 2.6)));
        assets.disc_mesh = Some(meshes.add(Cylinder::new(1.5, 0.12)));
        assets.eco_disc_mesh = Some(meshes.add(Cylinder::new(1.15, 0.08)));
    }
}

/// Eco band for a hex rating: 1 = lush (>= 80), -1 = degraded (< 25), else 0.
fn eco_band(rating: i32) -> i8 {
    if rating >= 80 {
        1
    } else if rating < 25 {
        -1
    } else {
        0
    }
}

/// Spawn/update/despawn hex visuals from the authoritative `hex_tile` cache.
pub fn update_plant_visuals(
    mut commands: Commands,
    net: Res<crate::net::plugin::Net>,
    player_transform: Res<crate::player::PlayerTransform>,
    _streaming_world: Res<StreamingWorldResource>,
    mut state: ResMut<FloorPlantState>,
    mut assets: ResMut<FloorPlantAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    ensure_plant_assets(&mut assets, &mut meshes, &mut materials);
    let (pollution, lush_h, degraded_h) = (
        assets.pollution_mat.clone().unwrap(),
        assets.lush_mat.clone().unwrap(),
        assets.degraded_mat.clone().unwrap(),
    );
    let (cone, tall, disc, eco_disc) = (
        assets.cone_mesh.clone().unwrap(),
        assets.tall_cone_mesh.clone().unwrap(),
        assets.disc_mesh.clone().unwrap(),
        assets.eco_disc_mesh.clone().unwrap(),
    );

    let Some(conn) = net.conn.as_ref() else { return };

    let px = player_transform.translation.x;
    let pz = player_transform.translation.z;
    let (hq, hr) = world_pos_to_hex(px, pz, WorldGenConfig::HEX_SIZE);
    let max_dist = RENDER_RADIUS_HEXES / WorldGenConfig::HEX_SIZE + 2.0;

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    for row in crate::net::gen::HexTileTableAccess::hex_tile(&conn.db).iter() {
        let dq = (row.hex_q - hq).abs() as f32;
        let dr = (row.hex_r - hr).abs() as f32;
        let ds = ((row.hex_q + row.hex_r) - (hq + hr)).abs() as f32;
        if dq.max(dr).max(ds) > max_dist {
            continue;
        }
        seen.insert(row.hex_id);

        let is_polluted = row.is_polluted;

        // Determine desired visual: pollution disc, plant cone, or nothing.
        let mut kind: Option<(Option<Handle<Mesh>>, Handle<StandardMaterial>)> = None;
        if is_polluted {
            kind = Some((Some(disc.clone()), pollution.clone()));
        }
        let mut mature = false;
        // Perf: skip the serde_json parse entirely when the raw column is
        // unchanged; only maturity (a pure time comparison) is recomputed.
        let raw_changed = state.raw.get(&row.hex_id).map(String::as_str) != row.plant.as_deref();
        if raw_changed {
            let parsed = row.plant.as_deref().and_then(|json| {
                serde_json::from_str::<serde_json::Value>(json).ok().map(|v| {
                    let planted_at = v.get("planted_at").and_then(|x| x.as_u64()).unwrap_or(0);
                    let growth = v.get("growth_time").and_then(|x| x.as_u64()).unwrap_or(3600);
                    let kind_name = v.get("plant_type").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    ParsedPlant { kind_name, mature_at: planted_at + growth }
                })
            });
            match parsed {
                Some(p) => {
                    state.raw.insert(row.hex_id, row.plant.clone().unwrap_or_default());
                    state.parsed.insert(row.hex_id, p.clone());
                    kind = Some((
                        Some(if matches!(p.kind_name.as_str(), "Tree" | "Corn" | "RareHerb") { tall.clone() } else { cone.clone() }),
                        // Spec 016 T4.6: per-type color, per-maturity shade; cached below.
                        if now >= p.mature_at {
                            assets.plant_mats.get(&p.kind_name).map(|(_, h)| h.clone()).unwrap_or(pollution.clone())
                        } else {
                            assets.plant_mats.get(&p.kind_name).map(|(h, _)| h.clone()).unwrap_or(pollution.clone())
                        },
                    ));
                }
                None => {
                    state.raw.remove(&row.hex_id);
                    state.parsed.remove(&row.hex_id);
                }
            }
        } else if let Some(p) = state.parsed.get(&row.hex_id) {
            mature = now >= p.mature_at;
            let (young_h, mature_h) = assets.plant_mats.get(&p.kind_name).cloned().unwrap_or((pollution.clone(), pollution.clone()));
            let use_tall = matches!(p.kind_name.as_str(), "Tree" | "Corn" | "RareHerb");
            kind = Some((
                Some(if use_tall { tall.clone() } else { cone.clone() }),
                if mature { mature_h.clone() } else { young_h.clone() },
            ));
        }

        let cached = state.stage.get(&row.hex_id).cloned();
        let band = eco_band(row.eco_rating);
        if cached == Some((is_polluted, mature, band)) {
            continue;
        }

        let (wx, wz) = row_world_center(row.hex_q, row.hex_r);
        let existing = state.visuals.get(&row.hex_id).copied();
        match kind {
            Some((mesh, mat)) => {
                let mesh = mesh.unwrap();
                if let Some(entity) = existing {
                    commands.entity(entity).despawn();
                }
                let mut root = commands.spawn((
                    Name::new(format!("hex-visual-{}", row.hex_id)),
                    HexPlantVisual,
                    Transform::from_xyz(wx, 0.0, wz),
                    Visibility::Visible,
                ));
                root.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, 1.1, 0.0),
                ));
                // Spec 020 T6.4: eco-rating tint disc (lush / degraded bands).
                if band != 0 {
                    let (eco_mesh, eco_mat) = if band > 0 {
                        (eco_disc.clone(), lush_h.clone())
                    } else {
                        (eco_disc.clone(), degraded_h.clone())
                    };
                    root.with_child((
                        Mesh3d(eco_mesh),
                        MeshMaterial3d(eco_mat),
                        Transform::from_xyz(0.0, 1.05, 0.0),
                    ));
                }
                state.visuals.insert(row.hex_id, root.id());
                state.stage.insert(row.hex_id, (is_polluted, mature, band));
            }
            None => {
                if let Some(entity) = existing {
                    commands.entity(entity).despawn();
                    state.visuals.remove(&row.hex_id);
                    state.stage.remove(&row.hex_id);
                }
            }
        }
    }

    // Despawn visuals for hexes that left the radius or have no row.
    let stale: Vec<u64> = state
        .visuals
        .keys()
        .filter(|k| !seen.contains(k))
        .copied()
        .collect();
    let stale_parsed: Vec<u64> = state
        .parsed
        .keys()
        .filter(|k| !seen.contains(k))
        .copied()
        .collect();
    for hex_id in stale {
        if let Some(entity) = state.visuals.remove(&hex_id) {
            commands.entity(entity).despawn();
        }
        state.stage.remove(&hex_id);
        state.raw.remove(&hex_id);
        state.parsed.remove(&hex_id);
    }
    for hex_id in stale_parsed {
        state.raw.remove(&hex_id);
        state.parsed.remove(&hex_id);
    }
}

fn row_world_center(q: i32, r: i32) -> (f32, f32) {
    idlecore_core::hex_grid::HexGrid::axial_to_world(q, r, WorldGenConfig::HEX_SIZE)
}

#[cfg(test)]
mod tests {}

#[cfg(test)]
mod tests_plants {
    use super::*;
    use crate::plugins::player::aura_config;

    #[test]
    fn every_plant_type_has_young_and_mature_color() {
        for plant_type in PLANT_TYPES {
            let young = plant_type_color(plant_type, false).expect("young color");
            let mature = plant_type_color(plant_type, true).expect("mature color");
            assert_ne!(young, mature, "{plant_type} mature should differ from young");
        }
    }

    #[test]
    fn plant_type_colors_are_distinct() {
        let mut colors: Vec<(u16, u16, u16, u16)> = PLANT_TYPES
            .iter()
            .map(|t| {
                let c = plant_type_color(t, true).unwrap().to_srgba().to_f32_array();
                (
                    (c[0] * 255.0) as u16,
                    (c[1] * 255.0) as u16,
                    (c[2] * 255.0) as u16,
                    (c[3] * 255.0) as u16,
                )
            })
            .collect();
        colors.sort();
        colors.dedup();
        assert_eq!(
            colors.len(),
            PLANT_TYPES.len(),
            "mature colors must differ per type"
        );
    }

    #[test]
    fn unknown_plant_type_falls_back() {
        assert!(plant_type_color("Mushroom", false).is_none());
        assert!(!PLANT_TYPES.iter().any(|t| *t == "Mushroom"));
    }

    #[test]
    fn eco_aura_gates_by_rank() {
        assert!(aura_config(0).is_none());
        assert!(aura_config(99).is_none());
        let e = aura_config(100).unwrap();
        assert_eq!(e.1, 2.5);
        let w = aura_config(500).unwrap();
        assert_eq!(w.1, 4.0);
        let l = aura_config(1000).unwrap();
        assert_eq!(l.1, 6.0);
        assert_eq!(aura_config(9999).unwrap().1, 6.0);
    }
}
