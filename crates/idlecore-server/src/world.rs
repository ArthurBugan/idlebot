//! Deterministic world generation — runs once on init.
//! Terrain follows the PROPOSAL 3.2 probability table; seeded so the same
//! module deployment always produces the same world (Spec 002 NFR3).

use spacetimedb::{ReducerContext, Table};
use crate::types::{hex_id_of, hex_tile, HexTile};

/// World radius in axial hexes (-R..=R on all three axes, ~12,480 hexes at
/// R=64 per PROPOSAL 3.1).
pub const WORLD_RADIUS: i32 = 64;

/// Terrain distribution table (PROPOSAL 3.2).
const TERRAIN_TABLE: [(&str, f32, i32); 6] = [
    ("Grass", 0.50, 50),
    ("Forest", 0.20, 50),
    ("Water", 0.08, 20),
    ("City", 0.10, 20),
    ("Desert", 0.07, 20),
    ("Polluted", 0.05, 10),
];

/// Deterministic hash for a hex — same seed => same world.
fn hash2(q: i32, r: i32, seed: u64) -> u64 {
    let mut x = (q as u64).wrapping_mul(0x9E3779B97F4A7C15);
    x = x.rotate_left(31);
    let mut y = (r as u64).wrapping_mul(0xC2B2AE3D27D4EB4F);
    y = y.rotate_left(17);
    let h = x ^ y ^ (seed.wrapping_mul(0x165667B19E3779F9)) ^ 0x6a09e667f3bcc909;
    h.wrapping_mul(0x9e3779b97f4a7c15) ^ (h >> 27).wrapping_mul(0xbf58476d1ce4e5b9)
}

fn terrain_for(q: i32, r: i32, seed: u64) -> (&'static str, i32) {
    let h = hash2(q, r, seed);
    let p = (h >> 33) as f64 / (1u64 << 31) as f64; // 0..1 uniform-ish
    let mut acc = 0.0f64;
    for (name, prob, eco) in TERRAIN_TABLE {
        acc += prob as f64;
        if p < acc {
            return (name, eco);
        }
    }
    ("Grass", 50)
}

/// Height-field style elevation: smooth-ish noise from adjacent hashes.
fn elevation_for(q: i32, r: i32, seed: u64) -> f32 {
    let base = (hash2(q, r, seed) >> 40) as f32 / (1u64 << 24) as f32; // 0..1
    // Blend with 2 neighbors for continuity.
    let n1 = (hash2(q + 1, r, seed) >> 40) as f32 / (1u64 << 24) as f32;
    let n2 = (hash2(q, r + 1, seed) >> 40) as f32 / (1u64 << 24) as f32;
    (0.5 * base + 0.25 * n1 + 0.25 * n2) * 2.0 - 0.7 // -0.7 .. 1.3
}

/// Generate and insert the whole map. Idempotent — fills any missing hexes,
/// so a single lost row (e.g. a manual delete) is repaired on the next
/// module init without touching existing rows.
pub fn generate_world(ctx: &ReducerContext, seed: u64) -> usize {
    let mut count = 0usize;
    for q in -WORLD_RADIUS..=WORLD_RADIUS {
        for r in -WORLD_RADIUS..=WORLD_RADIUS {
            let s = -q - r;
            if s.abs() > WORLD_RADIUS {
                continue;
            }
            let hex_id = hex_id_of(q, r);
            if ctx.db.hex_tile().hex_id().find(hex_id).is_some() {
                continue;
            }
            let (terrain, eco) = terrain_for(q, r, seed);
            let is_polluted = terrain == "Polluted";
            let elevation = if terrain == "Water" { -0.5 } else { elevation_for(q, r, seed) };
            ctx.db.hex_tile().insert(HexTile {
                hex_id,
                hex_q: q,
                hex_r: r,
                terrain: terrain.to_string(),
                elevation,
                eco_rating: eco,
                is_polluted,
                plant: None,
                planted_by: None,
                cleaned_at: None,
                last_interaction: 0,
            });
            count += 1;
        }
    }
    if count > 0 {
        tracing::info!("World filled: {count} missing hexes (seed {seed})");
    }
    count
}