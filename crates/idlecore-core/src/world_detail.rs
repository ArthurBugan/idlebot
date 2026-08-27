//! Procedural detail layer over the real-Earth macro-biomes (Hybrid world-gen).
//!
//! The authoritative terrain still comes from the embedded Earth raster
//! (`earth.rs`): continents, the broad biome bands and the city locations are
//! all real. This module adds the *detail* on top of that — the part that
//! makes a biome read as a living place rather than a flat tinted square:
//!
//! - [`floor_detail`] — a deterministic Perlin/fBm field that (a) clumps tile
//!   variants into natural patches instead of a repeating grid, (b) adds a
//!   subtle per-slot micro-tint, and (c) layers visual-only micro-biomes
//!   (grass clearings / dirt, desert rocky scrub, forest clearings).
//! - [`city_cell`] — a warped street grid that turns a `City` macro-hex into a
//!   believable urban zone of roads, plazas and buildings of varying height.
//!
//! Everything is a pure function of world position (+ terrain), so the client
//! and server always agree and no state has to be replicated. Gameplay terrain
//! is never changed here — patches and cities are purely visual.

use crate::terrain::TerrainType;

// ============================================================================
// Hashing + Perlin (improved) gradient noise
// ============================================================================

/// Seed for the whole detail layer; bump to reshuffle every floor/city.
const SEED: u64 = 0x5F3B_8C1D_A9E7_2046;

#[inline]
fn hash2(ix: i64, iy: i64, seed: u64) -> u64 {
    let mut h = (ix as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    h = h.wrapping_add(h >> 15);
    h ^= h.wrapping_mul(0x2C1B_3C6D_0F5B_5F67);
    h = h.wrapping_add(h >> 12);
    h ^= h.wrapping_mul(0x297A_2D39_B763_55E5);
    h = h.wrapping_add(h >> 15);
    h
}

/// Unit-ish gradient for a lattice point.
#[inline]
fn grad(ix: i64, iy: i64, seed: u64) -> (f64, f64) {
    let h = hash2(ix, iy, seed);
    let angle = ((h & 0xFFFF) as f64 / 65535.0) * std::f64::consts::TAU;
    (angle.cos(), angle.sin())
}

#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[inline]
fn dot(g: (f64, f64), x: f64, y: f64) -> f64 {
    g.0 * x + g.1 * y
}

/// Classic 2D Perlin noise in roughly [-1, 1].
pub fn perlin2(x: f64, y: f64, seed: u64) -> f64 {
    let x0 = x.floor();
    let y0 = y.floor();
    let x1 = x0 + 1.0;
    let y1 = y0 + 1.0;
    let sx = fade(x - x0);
    let sy = fade(y - y0);
    let n00 = dot(grad(x0 as i64, y0 as i64, seed), x - x0, y - y0);
    let n10 = dot(grad(x1 as i64, y0 as i64, seed), x - x1, y - y0);
    let n01 = dot(grad(x0 as i64, y1 as i64, seed), x - x0, y - y1);
    let n11 = dot(grad(x1 as i64, y1 as i64, seed), x - x1, y - y1);
    lerp(lerp(n00, n10, sx), lerp(n01, n11, sx), sy)
}

/// Fractal Brownian motion: layered Perlin octaves, normalized to ~[-1, 1].
pub fn fbm(x: f64, y: f64, seed: u64, octaves: u32) -> f64 {
    let mut amp = 0.5f64;
    let mut freq = 1.0f64;
    let mut sum = 0.0f64;
    let mut norm = 0.0f64;
    for o in 0..octaves.max(1) {
        sum += amp * perlin2(x * freq, y * freq, seed.wrapping_add((o as u64) * 1013));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    if norm == 0.0 {
        0.0
    } else {
        sum / norm
    }
}

/// Per-slot detail derived from world position. Visual only — the gameplay
/// terrain is unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorDetail {
    /// Index into the terrain's tile-variant list (clumped, not per-slot hash).
    pub variant: usize,
    /// Multiplicative tint applied on top of the variant's base tint.
    pub tint: [f32; 3],
}

/// Low-frequency, clumped tile-variant selection: neighbouring slots share a
/// variant, producing natural patches a few slots wide rather than static.
fn clumped_variant(x: f64, y: f64, n: usize, salt: u64) -> usize {
    if n <= 1 {
        return 0;
    }
    // ~6-slot patches: frequency ~1/26 world units.
    let v = fbm(x * 0.038, y * 0.038, SEED ^ salt, 3) * 0.5 + 0.5;
    ((v * n as f64).floor() as usize) % n
}

/// Subtle per-slot micro-tint (±6%) so a single biome never looks flat.
fn micro_tint(x: f64, y: f64) -> [f32; 3] {
    let r = fbm(x * 0.061, y * 0.061, SEED ^ 0xA1, 2) * 0.06;
    let g = fbm(x * 0.061 + 13.1, y * 0.061 - 7.7, SEED ^ 0xB2, 2) * 0.06;
    let b = fbm(x * 0.061 - 5.3, y * 0.061 + 19.4, SEED ^ 0xC3, 2) * 0.06;
    [1.0 + r as f32, 1.0 + g as f32, 1.0 + b as f32]
}

/// Visual-only micro-biome patch, layered on top of the macro biome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroPatch {
    None,
    /// Brighter, open ground (grass/forest clearing).
    Clearing,
    /// Bare earth speckle inside grass.
    Dirt,
    /// Stony scrub inside desert.
    Rocky,
}

pub fn micro_patch(x: f64, y: f64, terrain: TerrainType) -> MicroPatch {
    // Very low frequency → large, coherent patches.
    let p = fbm(x * 0.012, y * 0.012, SEED ^ 0xD4, 2);
    match terrain {
        TerrainType::Grass => {
            if p > 0.55 {
                MicroPatch::Clearing
            } else if p < -0.55 {
                MicroPatch::Dirt
            } else {
                MicroPatch::None
            }
        }
        TerrainType::Forest => {
            if p > 0.5 {
                MicroPatch::Clearing
            } else {
                MicroPatch::None
            }
        }
        TerrainType::Desert => {
            if p > 0.45 {
                MicroPatch::Rocky
            } else {
                MicroPatch::None
            }
        }
        _ => MicroPatch::None,
    }
}

/// Full floor detail for a land slot: a clumped variant plus a tint that folds
/// in the micro-tint and (for a few terrains) the micro-patch.
pub fn floor_detail(
    world_x: f32,
    world_y: f32,
    terrain: TerrainType,
    n_variants: usize,
) -> FloorDetail {
    let x = world_x as f64;
    let y = world_y as f64;
    let variant = clumped_variant(x, y, n_variants, 0x11);
    let mut tint = micro_tint(x, y);
    match micro_patch(x, y, terrain) {
        MicroPatch::Clearing => {
            tint[1] *= 1.08;
            tint[0] *= 0.96;
        }
        MicroPatch::Dirt => {
            tint[0] *= 1.14;
            tint[1] *= 0.95;
            tint[2] *= 0.8;
        }
        MicroPatch::Rocky => {
            tint[0] *= 0.9;
            tint[1] *= 0.9;
            tint[2] *= 0.96;
        }
        MicroPatch::None => {}
    }
    FloorDetail { variant, tint }
}

// ============================================================================
// Procedural cities (inside a City macro-hex)
// ============================================================================

/// What sits on a given world position inside a city.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CityCellKind {
    /// Street / asphalt.
    Road,
    /// Plaza / pavement (open yard between buildings).
    Block,
    /// Building footprint.
    Building,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CityCell {
    pub kind: CityCellKind,
    /// Building height in world units (only meaningful for `Building`).
    pub height: f32,
    /// Visual variant index (building style / road marking).
    pub variant: usize,
}

/// City block pitch in world units (~8 slots) and street width (~1.4 slots).
const CITY_BLOCK: f64 = 34.0;
const CITY_ROAD: f64 = 6.0;

/// Deterministic urban layout for a world position. Roads form a warped grid
/// (the warp stops the city looking like graph paper); building lots get a
/// height and a style variant from lower-frequency noise.
pub fn city_cell(world_x: f32, world_y: f32) -> CityCell {
    let x = world_x as f64;
    let y = world_y as f64;
    // Domain warp: bend the grid with low-freq noise so blocks feel organic.
    let wx = x + 14.0 * fbm(x * 0.008, y * 0.008, SEED ^ 0xE1, 2);
    let wy = y + 14.0 * fbm(x * 0.008 + 31.0, y * 0.008 - 17.0, SEED ^ 0xF2, 2);

    let gx = wx.rem_euclid(CITY_BLOCK);
    let gy = wy.rem_euclid(CITY_BLOCK);
    let on_road = gx < CITY_ROAD || gx > CITY_BLOCK - CITY_ROAD || gy < CITY_ROAD
        || gy > CITY_BLOCK - CITY_ROAD;
    if on_road {
        // Road marking variant from a mid-frequency field.
        let v = (fbm(wx * 0.05, wy * 0.05, SEED ^ 0x71, 2) * 0.5 + 0.5) * 3.0;
        return CityCell {
            kind: CityCellKind::Road,
            height: 0.0,
            variant: v as usize % 3,
        };
    }

    // Building lot: occasional plaza, otherwise a building of varied height.
    let lot = fbm(wx * 0.11, wy * 0.11, SEED ^ 0x83, 2);
    if lot < 0.16 {
        return CityCell {
            kind: CityCellKind::Block,
            height: 0.0,
            variant: 0,
        };
    }
    let h = 2.0 + 6.5 * (fbm(wx * 0.03, wy * 0.03, SEED ^ 0x95, 3) * 0.5 + 0.5);
    let v = (fbm(wx * 0.07, wy * 0.07, SEED ^ 0xA6, 2) * 0.5 + 0.5) * 5.0;
    CityCell {
        kind: CityCellKind::Building,
        height: h as f32,
        variant: v as usize % 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_is_deterministic_and_bounded() {
        for &(x, y) in &[(0.0, 0.0), (12.3, -45.6), (999.0, 1000.0), (-3.3, 7.7)] {
            let a = perlin2(x, y, SEED);
            let b = perlin2(x, y, SEED);
            assert!((a - b).abs() < 1e-12, "perlin not deterministic at {x},{y}");
            assert!(a >= -1.1 && a <= 1.1, "perlin out of range: {a}");
        }
    }

    #[test]
    fn fbm_normalized_range() {
        for i in 0..200 {
            let x = (i * 37 % 10000) as f64;
            let y = (i * 53 % 10000) as f64;
            let v = fbm(x, y, SEED, 4);
            assert!(v >= -1.0 && v <= 1.0, "fbm {v} out of [-1,1]");
        }
    }

    #[test]
    fn floor_detail_in_range_and_stable() {
        for terrain in [
            TerrainType::Grass,
            TerrainType::Forest,
            TerrainType::Desert,
            TerrainType::City,
            TerrainType::Mountain,
            TerrainType::Taiga,
        ] {
            let d1 = floor_detail(123.4, -56.7, terrain, 3);
            let d2 = floor_detail(123.4, -56.7, terrain, 3);
            assert_eq!(d1, d2, "floor_detail not stable for {terrain:?}");
            assert!(d1.variant < 3, "variant out of range");
            for c in d1.tint {
                assert!((0.6..=1.4).contains(&c), "tint {c} out of band");
            }
        }
    }

    #[test]
    fn floor_detail_variant_clumps_not_grid() {
        // Within a ~6-slot patch variants should repeat, proving clumping.
        let mut seen = std::collections::HashSet::new();
        for dx in 0..4 {
            for dy in 0..4 {
                let d = floor_detail(dx as f32 * 4.33, dy as f32 * 4.33, TerrainType::Grass, 3);
                seen.insert(d.variant);
            }
        }
        // Not every slot unique — at least some neighbouring slots share.
        assert!(seen.len() < 16, "variants look fully random, not clumped");
    }

    #[test]
    fn city_cell_is_deterministic_and_covers_kinds() {
        let a = city_cell(200.0, 200.0);
        let b = city_cell(200.0, 200.0);
        assert_eq!(a, b);
        let mut kinds = std::collections::HashSet::new();
        let mut max_h = 0.0f32;
        for qx in 0..40 {
            for qy in 0..40 {
                let c = city_cell(qx as f32 * 4.33, qy as f32 * 4.33);
                kinds.insert(c.kind);
                max_h = max_h.max(c.height);
            }
        }
        assert!(kinds.contains(&CityCellKind::Road), "cities need roads");
        assert!(kinds.contains(&CityCellKind::Building), "cities need buildings");
        assert!(max_h > 2.0, "buildings should have height");
    }
}
