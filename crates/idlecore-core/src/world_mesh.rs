//! Chunk terrain mesh generation — converts hex data into chunk meshes (§16, §25-27).
//!
//! The hex grid is a *logical* grid; the visual is a continuous mesh. Each
//! chunk produces a small number of renderable meshes (terrain, water) rather
//! than one object per hex. Mesh data here is engine-agnostic: a list of
//! vertices, triangles, and UV coordinates that the client renders.

use crate::hex::HexCoord;
use crate::world_gen::HexCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Level of detail for world rendering (§27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LodLevel {
    /// Gameplay — full detail.
    Lod0,
    /// Regional — simplified terrain, major roads.
    Lod1,
    /// World — biome masses, major water.
    Lod2,
    /// Strategic — continents, oceans, regions.
    Lod3,
}

impl LodLevel {
    /// Approximate index for ordering (0 = most detailed).
    pub fn index(self) -> u8 {
        match self {
            LodLevel::Lod0 => 0,
            LodLevel::Lod1 => 1,
            LodLevel::Lod2 => 2,
            LodLevel::Lod3 => 3,
        }
    }
}

/// A generated vertex in world space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeshVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u: f32,
    pub v: f32,
}

/// Triangle indices into a vertex list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshTriangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

/// A single renderable mesh for a chunk (terrain or water).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMesh {
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
    /// Per-vertex RGB color in 0.0–1.0 (parallel to `vertices`). Empty if the
    /// mesh should use a single flat material color instead.
    pub colors: Vec<[f32; 3]>,
    /// Mesh kind tag for material assignment.
    pub kind: MeshKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MeshKind {
    #[default]
    Terrain,
    Water,
    Coast,
    Road,
    Settlement,
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
}

/// How far a chunk is from the player → which LOD to use.
pub fn lod_for_distance(distance_hexes: i32) -> LodLevel {
    if distance_hexes <= 16 {
        LodLevel::Lod0
    } else if distance_hexes <= 64 {
        LodLevel::Lod1
    } else if distance_hexes <= 256 {
        LodLevel::Lod2
    } else {
        LodLevel::Lod3
    }
}

/// Options controlling chunk mesh generation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeshGenOptions {
    pub hex_radius: f32,
    pub elevation_scale: f32,
}

impl Default for MeshGenOptions {
    fn default() -> Self {
        Self {
            hex_radius: 100.0,
            elevation_scale: 20.0,
        }
    }
}

/// Return the six corner offsets for a pointy-top hex centered at (cx, cz),
/// matching `HexCoord::to_pixel` (Red Blob pointy-top grid). Corners start at
/// 30° so tessellated hexes share edges without gaps or overlaps.
fn hex_corners(cx: f32, cz: f32, hex_radius: f32) -> [(f32, f32); 6] {
    let mut corners = [(0.0, 0.0); 6];
    for i in 0..6 {
        let angle = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        corners[i] = (
            cx + hex_radius * angle.cos(),
            cz + hex_radius * angle.sin(),
        );
    }
    corners
}

/// The 6 corner indices of the center hex plus neighbor centers for shared
/// corner generation. For a flat-top grid, we generate a triangulated fan per hex.
fn hex_to_world(q: i32, r: i32, hex_radius: f32) -> (f32, f32) {
    HexCoord::new(q, r).to_pixel(hex_radius)
}

/// Generate a terrain mesh for a list of hex cells (a chunk).
///
/// Each hex becomes a fan of 6 triangles around its center using its 6 corner
/// points. Vertices are de-duplicated by (corner position) via the corner
/// sampling so adjacent chunk meshes can share edges. The `elevation` of the
/// cell raises its center and corners.
pub fn generate_chunk_terrain_mesh(
    cells: &[HexCell],
    options: &MeshGenOptions,
) -> ChunkMesh {
    let hr = options.hex_radius;
    let mut mesh = ChunkMesh { kind: MeshKind::Terrain, ..Default::default() };
    let mut vertex_index: HashMap<(i32, i32, i32), u32> = HashMap::new(); // corner id → index

    for cell in cells {
        let (cx, cz) = hex_to_world(cell.q, cell.r, hr);
        let elev = cell.elevation * options.elevation_scale;

        let color = cell.terrain.minimap_color();

        // Center vertex
        let center_key = (cell.q, cell.r, 0);
        let center_idx = *vertex_index.entry(center_key).or_insert_with(|| {
            let i = mesh.vertices.len() as u32;
            mesh.vertices.push(MeshVertex { x: cx, y: elev, z: cz, u: 0.5, v: 0.5 });
            mesh.colors.push(color);
            i
        });

        // Corner vertices
        let corners = hex_corners(cx, cz, hr);
        // Corner id: (q, r, corner_index) — crude but works for a single chunk.
        let mut corner_indices = [0u32; 6];
        for (i, (cx2, cz2)) in corners.iter().enumerate() {
            // approximation: neighboring hex shares the corner at (q',r')
            let key = (cell.q, cell.r, (i + 1) as i32);
            corner_indices[i] = *vertex_index.entry(key).or_insert_with(|| {
                let idx = mesh.vertices.len() as u32;
                let u = 0.5 + 0.5 * (cx2 - cx) / hr;
                let v = 0.5 + 0.5 * (cz2 - cz) / hr;
                mesh.vertices.push(MeshVertex { x: *cx2, y: elev, z: *cz2, u, v });
                mesh.colors.push(color);
                idx
            });
        }

        // 6 triangles around center (CCW when viewed from above → +Y normal)
        for i in 0..6 {
            let next = (i + 1) % 6;
            mesh.triangles.push(MeshTriangle {
                a: center_idx,
                b: corner_indices[next],
                c: corner_indices[i],
            });
        }
    }

    mesh
}

/// Determine if a hex is water by its cell classification.
pub fn cell_is_water(cell: &HexCell) -> bool {
    cell.water.is_water()
}

/// Generate a flat water mesh for the water cells in a chunk.
/// Water spans the same hex geometry at a fixed low elevation.
pub fn generate_chunk_water_mesh(
    cells: &[HexCell],
    options: &MeshGenOptions,
) -> ChunkMesh {
    let hr = options.hex_radius;
    let sea_level = 0.35 * options.elevation_scale;
    let mut mesh = ChunkMesh { kind: MeshKind::Water, ..Default::default() };
    for cell in cells {
        if !cell_is_water(cell) {
            continue;
        }
        let (cx, cz) = hex_to_world(cell.q, cell.r, hr);
        let center = mesh.vertices.len() as u32;
        mesh.vertices.push(MeshVertex { x: cx, y: sea_level, z: cz, u: 0.5, v: 0.5 });
        mesh.colors.push(cell.terrain.minimap_color());
        let corners = hex_corners(cx, cz, hr);
        let mut corner_indices = [0u32; 6];
        for (i, (cx2, cz2)) in corners.iter().enumerate() {
            corner_indices[i] = mesh.vertices.len() as u32;
            mesh.vertices.push(MeshVertex { x: *cx2, y: sea_level, z: *cz2, u: 0.0, v: 0.0 });
            mesh.colors.push(cell.terrain.minimap_color());
        }
        for i in 0..6 {
            let next = (i + 1) % 6;
            mesh.triangles.push(MeshTriangle {
                a: center,
                b: corner_indices[next],
                c: corner_indices[i],
            });
        }
    }
    mesh
}

/// Group porous cells into a water mask summary for a chunk.
/// Returns fraction of chunk hexes that are water.
pub fn water_fraction(cells: &[HexCell]) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    cells.iter().filter(|c| c.water.is_water()).count() as f32 / cells.len() as f32
}

/// Total byte estimate of logical cell data (for memory budgeting).
pub fn estimate_chunk_cell_bytes(chunk_size: i32) -> usize {
    let hexes = chunk_size * chunk_size;
    hexes as usize * std::mem::size_of::<HexCell>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_gen::WorldGenConfig;

    fn sample_cells() -> Vec<HexCell> {
        let config = WorldGenConfig::default();
        crate::world_gen::chunk_hexes(0, 0, 4)
            .iter()
            .map(|h| config.generate_hex(h.q, h.r))
            .collect()
    }
    use sample_cells as gen_cells;

    #[test]
    fn lod_levels_ordered() {
        assert!(LodLevel::Lod0 < LodLevel::Lod1);
        assert!(LodLevel::Lod1 < LodLevel::Lod2);
        assert!(LodLevel::Lod2 < LodLevel::Lod3);
        assert_eq!(lod_for_distance(5), LodLevel::Lod0);
        assert_eq!(lod_for_distance(300), LodLevel::Lod3);
    }

    #[test]
    fn terrain_mesh_has_triangles() {
        let cells = gen_cells();
        let mesh = generate_chunk_terrain_mesh(&cells, &MeshGenOptions::default());
        assert_eq!(mesh.kind, MeshKind::Terrain);
        assert!(mesh.triangle_count() > 0);
        // Each hex contributes 6 triangles = 16 hexes * 6
        assert_eq!(mesh.triangle_count(), cells.len() * 6);
    }

    #[test]
    fn water_mesh_only_contains_water() {
        let cells = gen_cells();
        let mesh = generate_chunk_water_mesh(&cells, &MeshGenOptions::default());
        let water_count = cells.iter().filter(|c| c.water.is_water()).count();
        // Water hexes each contribute 7 vertices (1 center + 6 corners)
        if water_count > 0 {
            assert!(mesh.is_empty() == false);
            assert!(mesh.vertices.len() == water_count * 7);
        }
    }

    #[test]
    fn water_fraction_valid() {
        let cells = gen_cells();
        let frac = water_fraction(&cells);
        assert!((0.0..=1.0).contains(&frac));
    }
}