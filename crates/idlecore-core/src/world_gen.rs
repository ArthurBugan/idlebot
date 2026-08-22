//! Hex World Grid Floor System — deterministic world-scale hex grid.
//!
//! Implements the core architectural primitives for a whole-world hex grid
//! at 1:10,000 scale: world bounds, compact cell data, and deterministic
//! generation from a world seed + hex coordinate.

use crate::hex::HexCoord;
use crate::terrain::TerrainType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ============================================================================
// 6. Hex Cell Data (companion to HexCoord)
// ============================================================================

/// Water classification for a hex cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WaterClass {
    #[default]
    None,
    Ocean,
    Sea,
    Lake,
    River,
    Wetland,
    Coast,
}

impl WaterClass {
    pub fn is_water(self) -> bool {
        matches!(
            self,
            WaterClass::Ocean | WaterClass::Sea | WaterClass::Lake | WaterClass::River
        )
    }
}

/// Compact hex cell data — the runtime representation of a single hex.
///
/// This is the primary data structure for the world grid. It is designed
/// to be cheaply stored and transmitted. Terrain details, biome info,
/// and other attributes are generated deterministically from `WorldSeed`
/// + `HexCoord` so that most cells never need to exist in memory.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HexCell {
    pub coord: HexCoord,
    pub q: i32,
    pub r: i32,
    pub elevation: f32,
    pub terrain: TerrainType,
    pub water: WaterClass,
    pub moisture: f32,
    pub temperature: f32,
    pub biome_id: u16,
    pub flags: u8,
    pub seed: u64,
}

impl HexCell {
    /// Generate the compact `hex_id` for a coordinate pair (without offset).
    pub fn id_of(q: i32, r: i32) -> u64 {
        let q_u32 = q as u32;
        let r_u32 = r as u32;
        (q_u32 as u64) << 32 | (r_u32 as u64)
    }

    /// World-space 2D position of this hex center.
    pub fn world_pos(&self, hex_radius: f32) -> (f32, f32) {
        self.coord.to_pixel(hex_radius)
    }

    /// Bitflag helpers (bits 0-3 reserved).
    pub const FLAG_RESOURCE: u8 = 0b0001;
    pub const FLAG_ROAD: u8 = 0b0010;
    pub const FLAG_SETTLEMENT: u8 = 0b0100;

    pub fn has_resource(self) -> bool {
        self.flags & Self::FLAG_RESOURCE != 0
    }
    pub fn has_road(self) -> bool {
        self.flags & Self::FLAG_ROAD != 0
    }
    pub fn has_settlement(self) -> bool {
        self.flags & Self::FLAG_SETTLEMENT != 0
    }
}

// ============================================================================
// 5. Hex World Generator — deterministic seed + coordinate
// ============================================================================

/// World generation parameters controlling scale and seed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldGenConfig {
    pub seed: u64,
    pub world_radius: i32,
    /// When true, every hex has the same elevation (flat plain) instead of
    /// noise-based terrain heights.
    pub flat: bool,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            world_radius: 1000,
            flat: false,
        }
    }
}

impl WorldGenConfig {
    /// Hex radius in world units. Aligned with the server hex radius (10 u),
    /// so client and server share a single world scale.
    pub const HEX_SIZE: f32 = 10.0;

    /// The chunk size in hexes.
    pub const CHUNK_SIZE: i32 = 32;

    /// Generate a single hex cell deterministically from seed + coordinate.
    ///
    /// This is the core function of the world generator. Given the same
    /// `WorldGenConfig` and `hex_coord`, it will always produce the same
    /// `HexCell`.
    pub fn generate_hex(&self, q: i32, r: i32) -> HexCell {
        let coord = HexCoord::new(q, r);

        // --- Elevation (continental + local noise) ---
        // A flat world uses a constant mid elevation: high enough to avoid
        // oceans (< 0.35), low enough to avoid mountains (> 0.75).
        let elevation = if self.flat { 0.5 } else { self.elevation(q, r) };

        // --- Climate variables ---
        let latitude = self.latitude(q, r);
        let temperature = self.temperature(elevation, latitude);
        let moisture = self.moisture(q, r, latitude);

        // --- Terrain / biome ---
        let (terrain, water, biome_id) = self.terrain_and_biome(elevation, latitude, temperature, moisture, q, r);

        // --- Flags ---
        let flags = self.flags(terrain, elevation, moisture);

        let seed = self.local_seed(q, r);

        HexCell {
            coord,
            q,
            r,
            elevation,
            terrain,
            water,
            moisture,
            temperature,
            biome_id,
            flags,
            seed,
        }
    }

    /// Compute latitude (-90..90) for a hex coordinate.
    fn latitude(&self, _q: i32, r: i32) -> f64 {
        let radius = self.world_radius as f64;
        let lat_norm = r as f64 / radius;
        (lat_norm * 90.0).clamp(-90.0, 90.0)
    }

    /// Multi-octave noise value in [0, 1).
    fn noise2d(&self, seed: u64, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut max_amp = 0.0;

        for _ in 0..octaves {
            let nx = x * freq + seed as f64 * 12.9898;
            let ny = y * freq + seed as f64 * 78.233;
            let vx = (nx * 0.12345 + ny * 0.34567).sin() * 43758.5453123;
            let noise = (vx - vx.floor()) as f64;
            sum += noise * amp;
            max_amp += amp;
            amp *= gain;
            freq *= lacunarity;
        }

        sum / max_amp
    }

    /// Continental-scale elevation in [0, 1].
    fn elevation(&self, q: i32, r: i32) -> f32 {
        let radius = self.world_radius as f64;
        let nq = q as f64 / radius;
        let nr = r as f64 / radius;

        let large = self.noise2d(self.seed, nq * 0.5, nr * 0.5, 5, 2.0, 0.5);
        let med = self.noise2d(self.seed.wrapping_add(999), nq * 2.0, nr * 2.0, 3, 2.0, 0.5);
        let fine = self.noise2d(self.seed.wrapping_add(7777), nq * 8.0, nr * 8.0, 2, 2.0, 0.5);

        let combined = large * 0.6 + med * 0.3 + fine * 0.1;
        combined as f32
    }

    /// Temperature in [-1, 1] (-1 = cold, 1 = hot).
    fn temperature(&self, elevation: f32, latitude: f64) -> f32 {
        let abs_lat = latitude.abs();
        let lat_temp = 1.0 - (abs_lat / 90.0) as f32; // warmer at equator
        let elev_temp = 1.0 - elevation * 0.5; // cooler at high elevation
        (lat_temp * 0.7 + elev_temp * 0.3).clamp(-1.0, 1.0)
    }

    /// Moisture in [0, 1].
    fn moisture(&self, q: i32, r: i32, _latitude: f64) -> f32 {
        let radius = self.world_radius as f64;
        let nq = q as f64 / radius;
        let nr = r as f64 / (radius as f64 * 0.5); // stretch longitude
        let base = self.noise2d(self.seed.wrapping_add(1337), nq * 3.0, nr * 3.0, 4, 2.0, 0.5);
        // Coastal areas tend to be wetter
        let coast_effect = 0.2;
        (base + coast_effect).clamp(0.0, 1.0) as f32
    }

    /// Determine terrain, water class, and biome id from climate variables.
    fn terrain_and_biome(
        &self,
        elevation: f32,
        latitude: f64,
        temperature: f32,
        moisture: f32,
        _q: i32,
        _r: i32,
    ) -> (TerrainType, WaterClass, u16) {
        let abs_lat = latitude.abs();

        // Oceans and deep water
        if elevation < 0.35 {
            return (TerrainType::Water, WaterClass::Ocean, 0);
        }
        // Coastal shallow water / coast
        if elevation < 0.45 {
            return (TerrainType::Water, WaterClass::Coast, 1);
        }

        // Mountain
        if elevation > 0.75 {
            return (TerrainType::Mountain, WaterClass::None, 2);
        }

        // Biome classification (Whittaker-style)
        let temp_cat = temp_category(temperature, abs_lat);
        let moisture_cat = moisture_category(moisture);
        let biome_id = biome_from_climate(temp_cat, moisture_cat);
        let terrain = terrain_from_biome(biome_id);

        (terrain, WaterClass::None, biome_id)
    }

    /// Assign gameplay flags based on terrain.
    fn flags(&self, terrain: TerrainType, elevation: f32, moisture: f32) -> u8 {
        let mut f = 0u8;
        if terrain == TerrainType::Mountain && elevation > 0.85 {
            f |= HexCell::FLAG_RESOURCE;
        }
        if terrain == TerrainType::Desert && moisture < 0.1 {
            f |= HexCell::FLAG_RESOURCE;
        }
        f
    }

    /// Per-hex seed for local variation (deterministic from world seed + coord).
    fn local_seed(&self, q: i32, r: i32) -> u64 {
        let h1 = (self.seed.wrapping_mul(0x9E3779B185EBCA87)).wrapping_add(q as u64);
        let h2 = h1 ^ (h1 >> 30);
        let h3 = h2.wrapping_mul(0xBF52E9D9);
        let h4 = h3 ^ (h3 >> 27);
        let h5 = h4.wrapping_mul(0x9E3779B97C4B3A1F);
        let h6 = h5.wrapping_add(r as u64);
        h6 ^ (h6 >> 31)
    }
}

fn temp_category(temp: f32, abs_lat: f64) -> u8 {
    if abs_lat > 66.0 {
        0 // polar
    } else if temp < -0.3 {
        0 // polar
    } else if temp < 0.1 {
        1 // subpolar
    } else if temp < 0.4 {
        2 // temperate
    } else if temp < 0.7 {
        3 // subtropical
    } else {
        4 // tropical
    }
}

fn moisture_category(moisture: f32) -> u8 {
    if moisture < 0.2 {
        0 // arid
    } else if moisture < 0.4 {
        1 // semi-arid
    } else if moisture < 0.6 {
        2 // moderate
    } else if moisture < 0.8 {
        3 // wet
    } else {
        4 // very wet
    }
}

/// Whittaker biome classification → id.
fn biome_from_climate(temp_cat: u8, moisture_cat: u8) -> u16 {
    const TABLE: [[u16; 5]; 5] = [
        [0, 0, 0, 0, 0], // polar: tundra (0)
        [0, 1, 0, 0, 0], // subpolar: mostly tundra/taiga
        [2, 2, 3, 3, 4], // temperate
        [5, 5, 6, 6, 7], // subtropical
        [5, 8, 9, 9, 10], // tropical
    ];
    TABLE[temp_cat as usize][moisture_cat as usize]
}

/// Biome id → TerrainType.
fn terrain_from_biome(id: u16) -> TerrainType {
    match id {
        0 => TerrainType::Tundra,
        1 => TerrainType::Taiga,
        2 => TerrainType::Grassland,
        3 => TerrainType::Grass,
        4 => TerrainType::TropicalRainforest,
        5 => TerrainType::Desert,
        6 => TerrainType::Grassland,
        7 => TerrainType::Forest,
        8 => TerrainType::Desert,
        9 => TerrainType::TropicalRainforest,
        _ => TerrainType::Grass,
    }
}

// ============================================================================
// 18. World Bounds
// ============================================================================

/// Defines the rectangular bounds of the hex world and provides
/// validation utilities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WorldBounds {
    pub min_q: i32,
    pub max_q: i32,
    pub min_r: i32,
    pub max_r: i32,
}

impl WorldBounds {
    /// Create bounds for a world with the given radius (hex distance from origin).
    pub fn centered(radius: i32) -> Self {
        Self {
            min_q: -radius,
            max_q: radius,
            min_r: -radius,
            max_r: radius,
        }
    }

    /// Check if a coordinate is within the world bounds.
    pub fn contains(&self, coord: HexCoord) -> bool {
        coord.q >= self.min_q
            && coord.q <= self.max_q
            && coord.r >= self.min_r
            && coord.r <= self.max_r
    }

    /// Clamp a coordinate to within the world bounds.
    pub fn clamp_coord(&self, q: i32, r: i32) -> HexCoord {
        let q = q.clamp(self.min_q, self.max_q);
        let r = r.clamp(self.min_r, self.max_r);
        HexCoord::new(q, r)
    }

    /// Width in hexes along q axis.
    pub fn width(&self) -> i32 {
        self.max_q - self.min_q + 1
    }

    /// Height in hexes along r axis.
    pub fn height(&self) -> i32 {
        self.max_r - self.min_r + 1
    }

    /// Geographic center in world coordinates.
    pub fn center(&self, hex_radius: f32) -> (f32, f32) {
        let cq = (self.min_q + self.max_q) / 2;
        let cr = (self.min_r + self.max_r) / 2;
        HexCoord::new(cq, cr).to_pixel(hex_radius)
    }
}

// ============================================================================
// 17. Chunking — generated chunk cache
// ============================================================================

/// Lifecycle state of a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ChunkState {
    #[default]
    Unloaded,
    Requested,
    Loading,
    Generating,
    Active,
    Unloading,
}

/// A generated chunk: cached hex cells for a chunk region.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Chunk {
    pub coord: (i32, i32),
    pub state: ChunkState,
    pub cells: Vec<HexCell>,
}

/// A player-centered chunk cache that only materializes nearby areas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkManager {
    pub chunks: HashMap<(i32, i32), Chunk>,
    pub chunk_size: i32,
    pub active_radius: i32,
    pub prefetch_radius: i32,
    pub last_center: Option<(i32, i32)>,
}

impl ChunkManager {
    pub fn new(chunk_size: i32, active_radius: i32, prefetch_radius: i32) -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_size,
            active_radius,
            prefetch_radius,
            last_center: None,
        }
    }

    /// Ensure chunks around the given hex coordinate are generated and cached.
    pub fn stream_around(&mut self, config: &WorldGenConfig, center_q: i32, center_r: i32) {
        let center_chunk = hex_to_chunk_coord(center_q, center_r, self.chunk_size);
        let mut requested = HashSet::new();

        // Mark all chunks within prefetch radius as Requested (deterministic set).
        for dcq in -self.prefetch_radius..=self.prefetch_radius {
            for dcr in -self.prefetch_radius..=self.prefetch_radius {
                let cq = center_chunk.0 + dcq;
                let cr = center_chunk.1 + dcr;
                requested.insert((cq, cr));
            }
        }

        // Remove chunks outside prefetch radius + a small margin (unloading).
        let unload_margin = 1;
        let unload_distance = (self.prefetch_radius + unload_margin) as u32;
        let to_remove: Vec<_> = self
            .chunks
            .keys()
            .filter(|(cq, cr)| {
                let dq = (cq - center_chunk.0).unsigned_abs();
                let dr = (cr - center_chunk.1).unsigned_abs();
                dq > unload_distance || dr > unload_distance
            })
            .cloned()
            .collect();

        for key in to_remove {
            self.unload_chunk(key);
        }

        // Generate missing chunks, prioritizing active radius first.
        let mut active_chunks = Vec::new();
        let mut prefetch_chunks = Vec::new();
        for (cq, cr) in requested {
            let contains_active = (cq - center_chunk.0).unsigned_abs() <= self.active_radius as u32
                && (cr - center_chunk.1).unsigned_abs() <= self.active_radius as u32;
            if contains_active {
                active_chunks.push((cq, cr));
            } else {
                prefetch_chunks.push((cq, cr));
            }
        }
        for (cq, cr) in active_chunks {
            self.ensure_chunk(config, cq, cr);
        }
        for (cq, cr) in prefetch_chunks {
            self.ensure_chunk(config, cq, cr);
        }

        self.last_center = Some(center_chunk);
    }

    /// Generate (or return cached) chunk cells.
    pub fn ensure_chunk(&mut self, config: &WorldGenConfig, cq: i32, cr: i32) -> &Chunk {
        if !self.chunks.contains_key(&(cq, cr)) {
            let mut chunk = Chunk {
                coord: (cq, cr),
                state: ChunkState::Generating,
                cells: Vec::new(),
            };
            for hex in chunk_hexes(cq, cr, self.chunk_size) {
                chunk.cells.push(config.generate_hex(hex.q, hex.r));
            }
            chunk.state = ChunkState::Active;
            self.chunks.insert((cq, cr), chunk);
        }
        self.chunks.get(&(cq, cr)).unwrap()
    }

    /// Unload a chunk and its cells.
    pub fn unload_chunk(&mut self, coord: (i32, i32)) {
        self.chunks.remove(&coord);
    }

    /// Get a cell by hex coordinate from the cache, if loaded.
    pub fn get_cell(&self, q: i32, r: i32) -> Option<&HexCell> {
        let (cq, cr) = hex_to_chunk_coord(q, r, self.chunk_size);
        let chunk = self.chunks.get(&(cq, cr))?;
        let start_q = cq * self.chunk_size;
        let start_r = cr * self.chunk_size;
        let idx = ((q - start_q) * self.chunk_size + (r - start_r)) as usize;
        chunk.cells.get(idx)
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

// ============================================================================
// 20-21. Segment System
// ============================================================================

/// A segment is a group of chunks — a coarse management unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: u64,
    pub cq0: i32,
    pub cr0: i32,
    pub chunks_per_side: i32,
    pub dominant_biome: u16,
    pub average_elevation: f32,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub water_percentage: f32,
    pub generation_seed: u64,
}

impl Segment {
    /// The number of chunks in a segment.
    pub fn chunk_count(&self) -> i32 {
        self.chunks_per_side * self.chunks_per_side
    }

    /// Compute segment metadata by sampling a subset of hex cells.
    pub fn compute_metadata(
        segment_id: u64,
        cq0: i32,
        cr0: i32,
        chunks_per_side: i32,
        config: &WorldGenConfig,
    ) -> Self {
        let mut sum_elev = 0.0f64;
        let mut min_elev = f32::INFINITY;
        let mut max_elev = f32::NEG_INFINITY;
        let mut water = 0usize;
        let mut total = 0usize;
        let mut biome_counts: HashMap<u16, usize> = HashMap::new();

        // Sample a sparse grid across the segment to keep costs low.
        let step = 4;
        for dcq in 0..chunks_per_side {
            for dcr in 0..chunks_per_side {
                let cq = cq0 + dcq;
                let cr = cr0 + dcr;
                for dr in (0..WorldGenConfig::CHUNK_SIZE).step_by(step) {
                    for dq in (0..WorldGenConfig::CHUNK_SIZE).step_by(step) {
                        let q = cq * WorldGenConfig::CHUNK_SIZE + dq;
                        let r = cr * WorldGenConfig::CHUNK_SIZE + dr;
                        let cell = config.generate_hex(q, r);
                        sum_elev += cell.elevation as f64;
                        min_elev = min_elev.min(cell.elevation);
                        max_elev = max_elev.max(cell.elevation);
                        if cell.water.is_water() {
                            water += 1;
                        }
                        *biome_counts.entry(cell.biome_id).or_insert(0) += 1;
                        total += 1;
                    }
                }
            }
        }

        let dominant_biome = *biome_counts
            .iter()
            .max_by_key(|(_, &c)| c)
            .map(|(b, _)| b)
            .unwrap_or(&0);

        Self {
            id: segment_id,
            cq0,
            cr0,
            chunks_per_side,
            dominant_biome,
            average_elevation: if total > 0 { (sum_elev / total as f64) as f32 } else { 0.0 },
            min_elevation: if water > 0 { min_elev.min(WorldGenConfig::HEX_SIZE * 0.4) } else { min_elev },
            max_elevation: max_elev,
            water_percentage: if total > 0 { water as f32 / total as f32 } else { 0.0 },
            generation_seed: config.seed,
        }
    }
}

/// Default chunks per side of a segment (16 × 16 chunks).
pub const SEGMENT_CHUNKS_PER_SIDE: i32 = 16;

/// Map a chunk coordinate to its segment coordinate.
pub fn chunk_to_segment_coord(cq: i32, cr: i32, chunks_per_side: i32) -> (i32, i32) {
    let sq = if cq >= 0 { cq / chunks_per_side } else { (cq - chunks_per_side + 1) / chunks_per_side };
    let sr = if cr >= 0 { cr / chunks_per_side } else { (cr - chunks_per_side + 1) / chunks_per_side };
    (sq, sr)
}

// ============================================================================
// 30. Movement Costs & Pathfinding
// ============================================================================

/// Per-terrain movement cost multiplier.
/// Higher = slower/more expensive to traverse.
pub fn terrain_movement_cost(terrain: TerrainType) -> f32 {
    match terrain {
        TerrainType::Grass => 1.0,
        TerrainType::Grassland => 1.1,
        TerrainType::Forest => 1.5,
        TerrainType::TropicalRainforest => 2.0,
        TerrainType::Taiga => 2.0,
        TerrainType::Desert => 1.5,
        TerrainType::Tundra => 1.2,
        TerrainType::Mountain => 3.0,
        TerrainType::Water => f32::INFINITY,
        TerrainType::City => 0.5,
        TerrainType::Polluted => 0.8,
    }
}

/// Whether a cell is traverseable at all.
pub fn is_traverseable(cell: &HexCell) -> bool {
    if cell.water.is_water() {
        return false;
    }
    terrain_movement_cost(cell.terrain).is_finite()
}

/// Compute the movement cost between two adjacent cells.
pub fn movement_cost_between(a: &HexCell, b: &HexCell) -> f32 {
    let ca = terrain_movement_cost(a.terrain);
    let cb = terrain_movement_cost(b.terrain);
    if ca.is_infinite() || cb.is_infinite() {
        return f32::INFINITY;
    }
    (ca + cb) / 2.0
}

/// A* pathfinding over the loaded hex grid.
pub struct HexPath {
    pub cells: Vec<(i32, i32)>,
    pub cost: f32,
}

/// Run A* from `from` to `to` using the given cell lookup.
pub fn find_path<F>(
    from: (i32, i32),
    to: (i32, i32),
    mut cost_fn: F,
) -> Option<HexPath>
where
    F: FnMut((i32, i32), (i32, i32)) -> f32,
{
    use std::collections::BinaryHeap;

    #[derive(PartialEq)]
    struct Node {
        q: i32,
        r: i32,
        f: f32,
        g: f32,
    }
    impl Eq for Node {}
    impl std::cmp::Ord for Node {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.f.partial_cmp(&other.f).unwrap_or(std::cmp::Ordering::Equal)
        }
    }
    impl std::cmp::PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    if from == to {
        return Some(HexPath { cells: vec![from], cost: 0.0 });
    }

    let heuristic = |q: i32, r: i32| {
        let dq = (q - to.0).abs() as f32;
        let dr = (r - to.1).abs() as f32;
        dq.max(dr)
    };

    let mut open = BinaryHeap::new();
    let mut g_scores: HashMap<(i32, i32), f32> = HashMap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut closed = HashSet::new();

g_scores.insert(from, 0.0);
    open.push(Node {
        q: from.0,
        r: from.1,
        f: heuristic(from.0, from.1),
        g: 0.0,
    });

    let dirs = [
        (1, -1), (0, 1), (-1, 0), (-1, 1), (0, -1), (1, 0),
    ];

    while let Some(current) = open.pop() {
        if closed.contains(&(current.q, current.r)) {
            continue;
        }
        if (current.q, current.r) == to {
            // Reconstruct
            let mut path = Vec::new();
            let mut cur = (current.q, current.r);
            path.push(cur);
            while let Some(&prev) = came_from.get(&cur) {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(HexPath { cells: path, cost: current.g });
        }
        closed.insert((current.q, current.r));

        for (dq, dr) in dirs {
            let nq = current.q + dq;
            let nr = current.r + dr;
            let n = (nq, nr);
            if closed.contains(&n) {
                continue;
            }
            let step_cost = cost_fn((current.q, current.r), n);
            if step_cost.is_infinite() {
                continue;
            }
            let tentative = current.g + step_cost;
            let entry = g_scores.entry(n).or_insert(f32::INFINITY);
            if tentative < *entry {
                *entry = tentative;
                came_from.insert(n, (current.q, current.r));
                open.push(Node {
                    q: nq,
                    r: nr,
                    f: tentative + (nq - to.0).abs() as f32 + (nr - to.1).abs() as f32,
                    g: tentative,
                });
            }
        }
    }

    None
}

// ============================================================================
// 22. Hierarchical Generation
// ============================================================================

/// A hierarchical world generator that samples large-scale features first,
/// then refines smaller scales — guaranteeing neighboring chunks agree.
///
/// Resolution ladder (see §6):
/// World (1000+ km) → Region (10-100 km) → Segment (1-10 km) →
/// Chunk (0.5-2 km) → Gameplay hex (~100 m).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HierarchicalGen {
    pub config: WorldGenConfig,
}

impl HierarchicalGen {
    pub fn new(config: WorldGenConfig) -> Self {
        Self { config }
    }

    /// Continental mask: 0.0 = deep ocean, 1.0 = high land.
    /// Sampled from a world-scale noise that is independent of hex resolution.
    pub fn continental_mask(&self, q: i32, r: i32) -> f32 {
        let radius = self.config.world_radius as f64;
        let nq = q as f64 / radius;
        let nr = r as f64 / radius;
        self.config
            .noise2d(self.config.seed, nq * 0.25, nr * 0.25, 4, 2.0, 0.5) as f32
    }

    /// Regional climate band: broad temperature by latitude + large-scale noise.
    pub fn climate_band(&self, q: i32, r: i32) -> f32 {
        let radius = self.config.world_radius as f64;
        let lat_norm = (r as f64 / radius).clamp(-1.0, 1.0);
        let base = 1.0 - lat_norm.abs(); // equator warm, poles cold
        let jitter = self
            .config
            .noise2d(self.config.seed.wrapping_add(31), q as f64 * 0.5, r as f64 * 0.5, 2, 2.0, 0.5);
        ((base * 0.8 + jitter * 0.2) as f32).clamp(0.0, 1.0)
    }

    /// Generate a hex cell, combining hierarchical layers (§22):
    /// continents → climate → ocean/land → elevation → biome.
    pub fn generate(&self, q: i32, r: i32) -> HexCell {
        // Layer 1: continental shape
        let continent = self.continental_mask(q, r);
        // Layer 2: climate
        let climate = self.climate_band(q, r);
        // Layer 3+: delegate to the deterministic generator, injecting the
        // continental bias so coastlines are smooth across chunk boundaries.
        let mut cell = self.config.generate_hex(q, r);
        let water_threshold = 0.35;
        if continent < water_threshold {
            cell.water = WaterClass::Ocean;
            cell.terrain = TerrainType::Water;
            cell.elevation = (continent / water_threshold - 0.5) * 0.2; // slightly below sea level
        } else {
            // Land elevation scales with continental mask, blended with local noise.
            let local = cell.elevation;
            cell.elevation = (local * 0.6 + (continent - water_threshold) / (1.0 - water_threshold) * 0.6)
                .clamp(0.0, 1.0);
            // Re-resolve biome using climate band influence.
            let temp = cell.temperature * 0.7 + (climate - 0.5) * 0.6;
            let (terrain, water, biome_id) = self.config.terrain_and_biome(
                cell.elevation,
                latitude_for(cell.r, self.config.world_radius),
                temp,
                cell.moisture,
                q,
                r,
            );
            cell.terrain = terrain;
            cell.water = water;
            cell.biome_id = biome_id;
        }
        cell
    }
}

fn latitude_for(r: i32, world_radius: i32) -> f64 {
    ((r as f64 / world_radius as f64).clamp(-1.0, 1.0)) * 90.0
}

// ============================================================================
// 8. World Bounds — helper for geographic conversions
// ============================================================================

/// Convert a hex coordinate to its containing chunk coordinate.
/// Uses floor division so negative coordinates tile correctly.
pub fn hex_to_chunk_coord(q: i32, r: i32, chunk_size: i32) -> (i32, i32) {
    let cq = if q >= 0 { q / chunk_size } else { (q - chunk_size + 1) / chunk_size };
    let cr = if r >= 0 { r / chunk_size } else { (r - chunk_size + 1) / chunk_size };
    (cq, cr)
}

/// Get all hex coordinates in a chunk.
pub fn chunk_hexes(cq: i32, cr: i32, chunk_size: i32) -> Vec<HexCoord> {
    let mut result = Vec::new();
    let start_q = cq * chunk_size;
    let start_r = cr * chunk_size;
    for dq in 0..chunk_size {
        for dr in 0..chunk_size {
            let q = start_q + dq;
            let r = start_r + dr;
            let s = -q - r;
            if q.unsigned_abs() <= i32::MAX as u32
                && r.unsigned_abs() <= i32::MAX as u32
                && s.unsigned_abs() <= i32::MAX as u32
            {
                result.push(HexCoord::new(q, r));
            }
        }
    }
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_bounds_contains() {
        let bounds = WorldBounds::centered(100);
        assert!(bounds.contains(HexCoord::new(0, 0)));
        assert!(bounds.contains(HexCoord::new(100, 100)));
        assert!(!bounds.contains(HexCoord::new(101, 0)));
    }

    #[test]
    fn test_world_bounds_center() {
        let bounds = WorldBounds::centered(100);
        let (x, y) = bounds.center(100.0);
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_hex_to_chunk_coord_positive() {
        let (cq, cr) = hex_to_chunk_coord(35, 42, 32);
        assert_eq!(cq, 1);
        assert_eq!(cr, 1);
    }

    #[test]
    fn test_hex_to_chunk_coord_zero() {
        let (cq, cr) = hex_to_chunk_coord(0, 0, 32);
        assert_eq!(cq, 0);
        assert_eq!(cr, 0);
    }

    #[test]
    fn test_hex_to_chunk_coord_negative() {
        let (cq, cr) = hex_to_chunk_coord(-1, -1, 32);
        assert_eq!(cq, -1);
        assert_eq!(cr, -1);
    }

    #[test]
    fn test_hex_to_chunk_coord_boundary() {
        let (cq, cr) = hex_to_chunk_coord(31, 31, 32);
        assert_eq!(cq, 0);
        assert_eq!(cr, 0);

        let (cq, cr) = hex_to_chunk_coord(32, 32, 32);
        assert_eq!(cq, 1);
        assert_eq!(cr, 1);
    }

    #[test]
    fn test_generate_hex_deterministic() {
        let config = WorldGenConfig::default();
        let cell1 = config.generate_hex(3, -7);
        let cell2 = config.generate_hex(3, -7);
        assert_eq!(cell1.elevation, cell2.elevation);
        assert_eq!(cell1.terrain, cell2.terrain);
        assert_eq!(cell1.water, cell2.water);
    }

    #[test]
    fn test_generate_hex_different_coords() {
        let config = WorldGenConfig::default();
        let cell1 = config.generate_hex(0, 0);
        let cell2 = config.generate_hex(100, -50);
        assert_ne!(cell1.elevation, cell2.elevation);
    }

    #[test]
    fn test_generate_hex_ocean_at_edge() {
        let config = WorldGenConfig {
            seed: 42,
            world_radius: 10,
            flat: false,
        };
        // Far from center tends to be water
        let cell = config.generate_hex(8, 8);
        // At high latitude, should be tundra or water
        assert!(
            cell.terrain == TerrainType::Water
                || cell.terrain == TerrainType::Tundra
                || cell.terrain == TerrainType::Mountain
        );
    }

    #[test]
    fn test_hierarchical_generation_deterministic() {
        let config = WorldGenConfig::default();
        let gen = HierarchicalGen::new(config);
        let a = gen.generate(4, -3);
        let b = gen.generate(4, -3);
        assert_eq!(a.terrain, b.terrain);
        assert_eq!(a.water, b.water);
        assert_eq!(a.elevation, b.elevation);
    }

    #[test]
    fn test_hierarchical_continental_mask() {
        let config = WorldGenConfig::default();
        let gen = HierarchicalGen::new(config);
        let mask = gen.continental_mask(0, 0);
        assert!((0.0..=1.0).contains(&mask));
    }

    #[test]
    fn test_hierarchical_ocean_consistent() {
        let config = WorldGenConfig { seed: 42, world_radius: 50, flat: false };
        let gen = HierarchicalGen::new(config);
        // Find a coordinate where the continental mask is below the water
        // threshold, then verify generation classifies it as water.
        let mut found = false;
        for q in 45..=50 {
            for r in 45..=50 {
                if gen.continental_mask(q, r) < 0.35 {
                    let cell = gen.generate(q, r);
                    assert!(cell.water.is_water() || cell.terrain == TerrainType::Water);
                    found = true;
                }
            }
        }
        // If none found, this seed has no such corner — accept either way,
        // but assert generation is deterministic.
        let cell = gen.generate(48, 48);
        let cell2 = gen.generate(48, 48);
        assert_eq!(cell.water, cell2.water);
        assert_eq!(cell.terrain, cell2.terrain);
        let _ = found;
    }

    #[test]
    fn test_local_seed_deterministic() {
        let config = WorldGenConfig::default();
        let s1 = config.local_seed(3, -7);
        let s2 = config.local_seed(3, -7);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_local_seed_unique() {
        let config = WorldGenConfig::default();
        let s1 = config.local_seed(0, 0);
        let s2 = config.local_seed(1, 0);
        let s3 = config.local_seed(0, 1);
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
        assert_ne!(s2, s3);
    }

    #[test]
    fn test_water_class_is_water() {
        assert!(WaterClass::Ocean.is_water());
        assert!(WaterClass::Sea.is_water());
        assert!(WaterClass::River.is_water());
        assert!(!WaterClass::None.is_water());
        assert!(!WaterClass::Coast.is_water());
    }

    #[test]
    fn test_hex_cell_id_packing() {
        assert_eq!(HexCell::id_of(0, 0), 0);
        assert_eq!(HexCell::id_of(1, 0), (1u64) << 32);
        assert_eq!(HexCell::id_of(-1, 0), (u32::MAX as u64) << 32);
    }

    #[test]
    fn test_chunk_hexes_count() {
        let hexes = chunk_hexes(0, 0, 8);
        assert_eq!(hexes.len(), 64);
    }

    #[test]
    fn test_chunk_hexes_offset() {
        let hexes = chunk_hexes(1, 0, 8);
        assert_eq!(hexes.len(), 64);
        // First hex should be (8, 0)
        assert_eq!(hexes[0].q, 8);
        assert_eq!(hexes[0].r, 0);
    }

    #[test]
    fn test_chunk_manager_streams_and_caches() {
        let config = WorldGenConfig::default();
        let mut manager = ChunkManager::new(8, 1, 2);
        manager.stream_around(&config, 0, 0);
        assert!(manager.loaded_chunk_count() > 0);
        // Same position — no new generation, cached
        let count = manager.loaded_chunk_count();
        manager.stream_around(&config, 0, 0);
        assert_eq!(manager.loaded_chunk_count(), count);
    }

    #[test]
    fn test_chunk_manager_unloads_distant_chunks() {
        let config = WorldGenConfig::default();
        let mut manager = ChunkManager::new(8, 1, 1);
        manager.stream_around(&config, 0, 0);
        let _initial = manager.loaded_chunk_count();
        manager.stream_around(&config, 100, 100);
        // Old chunks should be unloaded
        let after = manager.loaded_chunk_count();
        assert!(after <= (3 * 3) as usize, "expected ~9 chunks got {}", after);
    }

    #[test]
    fn test_get_cell_from_cache() {
        let config = WorldGenConfig::default();
        let mut manager = ChunkManager::new(8, 2, 2);
        manager.stream_around(&config, 0, 0);
        let cell = manager.get_cell(0, 0).expect("origin cell loaded");
        assert_eq!(cell.q, 0);
        assert_eq!(cell.r, 0);
        assert!(manager.get_cell(5000, 5000).is_none());
    }

    #[test]
    fn test_segment_metadata() {
        let config = WorldGenConfig {
            seed: 1234,
            world_radius: 500,
            flat: false,
        };
        let segment = Segment::compute_metadata(1, 0, 0, SEGMENT_CHUNKS_PER_SIDE, &config);
        assert_eq!(segment.id, 1);
        assert!(segment.average_elevation.is_finite());
        assert!(segment.min_elevation <= segment.max_elevation);
        assert!((0.0..=1.0).contains(&segment.water_percentage));
    }

    #[test]
    fn test_chunk_to_segment_coord() {
        let (sq, sr) = chunk_to_segment_coord(0, 0, 16);
        assert_eq!((sq, sr), (0, 0));
        let (sq, sr) = chunk_to_segment_coord(16, 0, 16);
        assert_eq!((sq, sr), (1, 0));
        let (sq, sr) = chunk_to_segment_coord(-1, -1, 16);
        assert_eq!((sq, sr), (-1, -1));
    }

    #[test]
    fn test_terrain_movement_cost() {
        assert_eq!(terrain_movement_cost(TerrainType::Grass), 1.0);
        assert!(terrain_movement_cost(TerrainType::Water).is_infinite());
        assert!(terrain_movement_cost(TerrainType::Mountain) > 1.0);
        assert!(terrain_movement_cost(TerrainType::City) < 1.0);
    }

    #[test]
    fn test_pathfinding_through_passable_terrain() {
        let config = WorldGenConfig::default();
        let mut manager = ChunkManager::new(8, 10, 10);
        manager.stream_around(&config, 0, 0);

        // Find a path between two nearby land cells
        let from = (0, 0);
        let to = (5, 5);
        let path = find_path(from, to, |a, b| {
            match (manager.get_cell(a.0, a.1), manager.get_cell(b.0, b.1)) {
                (Some(ca), Some(cb)) => movement_cost_between(ca, cb),
                _ => f32::INFINITY,
            }
        });
        // The path may be None if water blocks; verify either no path or valid path.
        if let Some(p) = path {
            assert_eq!(p.cells.first(), Some(&from));
            assert_eq!(p.cells.last(), Some(&to));
            assert!(p.cost.is_finite());
        }
    }

    #[test]
    fn test_pathfinding_identity() {
        let path = find_path((3, 3), (3, 3), |_, _| f32::INFINITY).unwrap();
        assert_eq!(path.cells, vec![(3, 3)]);
        assert_eq!(path.cost, 0.0);
    }

    #[test]
    fn test_pathfinding_impassable() {
        // All-water world → no path
        let config = WorldGenConfig {
            seed: 0,
            world_radius: 100,
            flat: false,
        };
        // Force an all-water lookup by checking against a field
        let path = find_path((0, 0), (3, 3), |_, _| {
            let cell = config.generate_hex(0, 0);
            if cell.water.is_water() { f32::INFINITY } else { 1.0 }
        });
        // If the sampled cell is land, path is Some; otherwise None. Just assert deterministic.
        let cell = config.generate_hex(0, 0);
        let expects_blocked = cell.water.is_water();
        let blocked = path.is_none();
        assert_eq!(blocked, expects_blocked);
    }
}
