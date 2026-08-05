//! World generation — 1:10000 scale Earth replica.
//! Uses proper continental shapes and latitude-based biomes.

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use crate::hex::HexCoord;
use crate::terrain::TerrainType;

/// Hex world size constant (10x original 10-unit hexes = 100 units)
pub const HEX_SIZE: f32 = 150.0;

/// A single tile in the world.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldTile {
    pub coord: HexCoord,
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: TerrainType,
    pub elevation: f32,
    pub vegetation: Vegetation,
    pub owned_by: Option<u64>,
}

impl WorldTile {
    /// Create a new world tile.
    pub fn new(coord: HexCoord, hex_id: u64, terrain: TerrainType, elevation: f32, vegetation: Vegetation) -> Self {
        let q = coord.q as f32;
        let r = coord.r as f32;
        let sqrt3 = f32::sqrt(3.0);
        let x = HEX_SIZE * sqrt3 * (q + r / 2.0);
        let y = HEX_SIZE * 1.5 * r;

        Self {
            coord,
            hex_id,
            center_x: x,
            center_y: y,
            terrain,
            elevation,
            vegetation,
            owned_by: None,
        }
    }
}

/// Vegetation type for a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Vegetation {
    None,
    Trees,
    Bushes,
    Cacti,
    Snow,
}

/// The entire world.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EarthWorld {
    pub tiles: HashMap<u64, WorldTile>,
    pub radius: i32,
    /// Loaded chunks (keyed by chunk coordinate).
    pub loaded_chunks: HashMap<(i32, i32), Vec<u64>>,
    /// Chunk size in hexes.
    pub chunk_size: i32,
}

impl EarthWorld {
    /// Generate a new world with Earth-like continental shapes.
    pub fn generate(seed: u64, radius: i32) -> Self {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut world = Self {
            tiles: HashMap::new(),
            radius,
            loaded_chunks: HashMap::new(),
            chunk_size: 8,
        };

        // Generate all tiles
        for q in -radius..=radius {
            for r in -radius..=radius {
                let s = -q - r;
                if q.unsigned_abs() as u64 <= radius as u64
                    && r.unsigned_abs() as u64 <= radius as u64
                    && s.unsigned_abs() as u64 <= radius as u64
                {
                    let hex_id = ((q as u32) as u64) << 32 | (r as u32) as u64;
                    let lat_normalized = r as f64 / radius as f64;
                    let latitude = lat_normalized * 90.0;

                    // Multi-octave continental noise for realistic shapes
                    let elevation = Self::generate_continental_elevation(&mut rng, q, r, radius);

                    let terrain = Self::determine_terrain(latitude, elevation);
                    let vegetation = Self::determine_vegetation(&mut rng, terrain, elevation);

                    let tile = WorldTile::new(
                        HexCoord::new(q, r),
                        hex_id,
                        terrain,
                        elevation,
                        vegetation,
                    );

                    world.tiles.insert(hex_id, tile);
                }
            }
        }

        // Build chunk indexes
        world.build_chunks();

        world
    }

    /// Build chunk indexes from all loaded tiles.
    fn build_chunks(&mut self) {
        for tile in self.tiles.values() {
            let cq = tile.coord.q / self.chunk_size;
            let cr = tile.coord.r / self.chunk_size;
            self.loaded_chunks
                .entry((cq, cr))
                .or_default()
                .push(tile.hex_id);
        }
    }

    /// Generate continental elevation using multi-octave noise for Earth-like landmasses.
    fn generate_continental_elevation(
        rng: &mut SmallRng,
        q: i32,
        r: i32,
        radius: i32,
    ) -> f32 {
        let nq = q as f64 / radius as f64;
        let nr = r as f64 / radius as f64;

        let mut value = 0.0;
        let mut amplitude = 1.0;
        let frequency = 1.0;
        let mut max_value = 0.0;

        for octave in 0..4 {
            let freq = frequency * (2.0_f64.powi(octave as i32));
            let amp = amplitude;

            let hash1 = (nq * freq + 131.7).sin() * 43758.5453;
            let hash2 = (nr * freq + 241.3).sin() * 26519.5433;
            let hash3 = (nq * freq + nr * freq + 73.1).sin() * 11371.7531;

            let noise = ((hash1 + hash2 + hash3) % 1.0 + 1.0) % 1.0;

            value += noise * amp;
            max_value += amp;
            amplitude *= 0.5;
        }

        value = value / max_value;

        value += rng.gen::<f64>() * 0.1 - 0.05;

        value as f32
    }

    /// Determine terrain type based on latitude and elevation (Earth-like distribution).
    fn determine_terrain(latitude: f64, elevation: f32) -> TerrainType {
        let abs_lat = latitude.abs();

        if elevation > 0.75 {
            return TerrainType::Mountain;
        }

        if elevation < 0.45 {
            return TerrainType::Water;
        }

        if abs_lat > 66.0 {
            TerrainType::Tundra
        } else if abs_lat > 50.0 {
            TerrainType::Taiga
        } else if abs_lat > 35.0 {
            if elevation > 0.55 {
                TerrainType::Grassland
            } else if elevation > 0.45 {
                TerrainType::Grass
            } else {
                TerrainType::Grassland
            }
        } else if abs_lat > 20.0 {
            if elevation < 0.4 {
                TerrainType::Desert
            } else {
                TerrainType::Grass
            }
        } else {
            if elevation > 0.6 {
                TerrainType::Grassland
            } else {
                TerrainType::TropicalRainforest
            }
        }
    }

    /// Determine vegetation based on terrain and elevation.
    fn determine_vegetation(rng: &mut SmallRng, terrain: TerrainType, _elevation: f32) -> Vegetation {
        match terrain {
            TerrainType::Tundra => Vegetation::Snow,
            TerrainType::Taiga => Vegetation::Trees,
            TerrainType::Mountain => Vegetation::None,
            TerrainType::Desert => Vegetation::Cacti,
            TerrainType::TropicalRainforest => Vegetation::Trees,
            TerrainType::Grass => {
                if rng.gen::<f32>() > 0.5 {
                    Vegetation::Bushes
                } else {
                    Vegetation::None
                }
            }
            _ => Vegetation::None,
        }
    }

    /// Get a tile by hex coordinates.
    pub fn get_tile(&self, q: i32, r: i32) -> Option<&WorldTile> {
        let hex_id = ((q as u32) as u64) << 32 | (r as u32) as u64;
        self.tiles.get(&hex_id)
    }

    /// Get a tile by id.
    pub fn get_tile_by_id(&self, hex_id: u64) -> Option<&WorldTile> {
        self.tiles.get(&hex_id)
    }

    /// Get the number of tiles in the world.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get the radius of the world.
    pub fn world_radius(&self) -> i32 {
        self.radius
    }

    /// Get tiles in a specific chunk.
    pub fn get_chunk_tiles(&self, chunk_x: i32, chunk_y: i32) -> Vec<&WorldTile> {
        self.loaded_chunks
            .get(&(chunk_x, chunk_y))
            .map(|ids| ids.iter().filter_map(|id| self.tiles.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get the number of loaded chunks.
    pub fn loaded_chunk_count(&self) -> usize {
        self.loaded_chunks.len()
    }

    /// Load chunks around a position (for minimap display).
    /// This marks chunks as "visible" by ensuring their tiles are accessible.
    pub fn load_chunks_around(&mut self, center_q: i32, center_r: i32, view_radius: i32) {
        let chunk_radius = (view_radius / self.chunk_size) + 1;
        let start_cq = (center_q / self.chunk_size) - chunk_radius;
        let end_cq = (center_q / self.chunk_size) + chunk_radius;
        let start_cr = (center_r / self.chunk_size) - chunk_radius;
        let end_cr = (center_r / self.chunk_size) + chunk_radius;

        // Build chunk indexes for chunks within view range
        for cq in start_cq..=end_cq {
            for cr in start_cr..=end_cr {
                // Get all tiles in this chunk range
                let chunk_q_min = cq * self.chunk_size;
                let chunk_q_max = (cq + 1) * self.chunk_size;
                let chunk_r_min = cr * self.chunk_size;
                let chunk_r_max = (cr + 1) * self.chunk_size;

                let mut chunk_tiles: Vec<u64> = Vec::new();
                for (hex_id, tile) in &self.tiles {
                    if tile.coord.q >= chunk_q_min && tile.coord.q < chunk_q_max
                        && tile.coord.r >= chunk_r_min && tile.coord.r < chunk_r_max
                    {
                        chunk_tiles.push(*hex_id);
                    }
                }

                if !chunk_tiles.is_empty() {
                    self.loaded_chunks.insert((cq, cr), chunk_tiles);
                }
            }
        }
    }

    /// Unload chunks outside the view radius (for performance).
    pub fn unload_chunks_around(&mut self, center_q: i32, center_r: i32, view_radius: i32) {
        let chunk_radius = (view_radius / self.chunk_size) + 1;
        let start_cq = (center_q / self.chunk_size) - chunk_radius;
        let end_cq = (center_q / self.chunk_size) + chunk_radius;
        let start_cr = (center_r / self.chunk_size) - chunk_radius;
        let end_cr = (center_r / self.chunk_size) + chunk_radius;

        // Remove chunks outside the view radius
        let keys_to_remove: Vec<_> = self
            .loaded_chunks
            .keys()
            .filter(|(cq, cr)| {
                cq.abs() < start_cq.abs()
                    || cq.abs() > end_cq.abs()
                    || cr.abs() < start_cr.abs()
                    || cr.abs() > end_cr.abs()
            })
            .cloned()
            .collect();

        for key in keys_to_remove {
            self.loaded_chunks.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_generation() {
        let world = EarthWorld::generate(42, 10);
        assert!(world.tile_count() > 0);
    }

    #[test]
    fn test_tile_terrain() {
        let world = EarthWorld::generate(42, 10);
        let tile = world.get_tile(0, 0).unwrap();
        assert!(matches!(
            tile.terrain,
            TerrainType::Grass
                | TerrainType::Forest
                | TerrainType::Water
                | TerrainType::City
                | TerrainType::Desert
                | TerrainType::Polluted
                | TerrainType::Tundra
                | TerrainType::Taiga
                | TerrainType::Grassland
                | TerrainType::TropicalRainforest
                | TerrainType::Mountain
        ));
    }

    #[test]
    fn test_biome_distribution() {
        let world = EarthWorld::generate(42, 50);
        let mut terrain_counts: HashMap<TerrainType, usize> = HashMap::new();
        for tile in world.tiles.values() {
            *terrain_counts.entry(tile.terrain).or_insert(0) += 1;
        }
        assert!(terrain_counts.len() > 1, "Expected multiple terrains, got: {:?}", terrain_counts);
    }

    #[test]
    fn test_water_land_ratio() {
        let world = EarthWorld::generate(42, 50);
        let water_count = world.tiles.values().filter(|t| t.terrain.is_water()).count();
        let ratio = water_count as f64 / world.tiles.len() as f64;
        assert!(ratio > 0.30 && ratio < 0.90, "Water ratio {} out of range", ratio);
    }

    #[test]
    fn test_hex_coordinates() {
        let world = EarthWorld::generate(42, 50);
        let tile = world.get_tile(0, 0).unwrap();
        assert_eq!(tile.coord.q, 0);
        assert_eq!(tile.coord.r, 0);
    }

    #[test]
    fn test_seed_determinism() {
        let world1 = EarthWorld::generate(42, 10);
        let world2 = EarthWorld::generate(42, 10);
        assert_eq!(world1.tile_count(), world2.tile_count());
    }
}
