//! World tile generation — the real-Earth replica at 1:100 scale.
//!
//! The planet spans ~308M hexes, so tiles are materialized lazily: a
//! `hex_tile` row is created from `idlecore_core::earth` data the first time
//! anything needs it (movement, interactions, teleport, login). Rows are
//! immutable terrain snapshots; plants/pollution mutate on top later.

use spacetimedb::{ReducerContext, Table};
use crate::types::{tile_id_of, hex_tile, HexTile};
use idlecore_core::earth;
use idlecore_core::grid::WorldGrid;

/// Square tile edge in world units — the footprint every tile covers, so
/// a tile (q, r) spans exactly the Earth points inside its own edge.
const TILE_W: f32 = idlecore_core::world_gen::WorldGenConfig::TILE_SIZE;

/// Radius (in hexes) of tiles materialized around a player position. Covers
/// interaction range (1) plus the HUD/eco lookups for the visible ring.
pub const ENSURE_RADIUS: i32 = 3;

/// Ensure the `hex_tile` row for one axial coordinate exists, creating it
/// from Earth data when missing. Also rolls the hex's natural resource nodes
/// (grass/rocks — never trees). Returns true if a tile row was inserted.
pub fn ensure_hex(ctx: &ReducerContext, q: i32, r: i32) -> bool {
    let Some(biome) = biome_for_hex(q, r) else { return false };
    let hex_id = tile_id_of(q, r);
    let inserted = if ctx.db.hex_tile().hex_id().find(hex_id).is_some() {
        false
    } else {
        let terrain = biome.terrain();
        ctx.db.hex_tile().insert(HexTile {
            hex_id,
            hex_q: q,
            hex_r: r,
            terrain: earth::terrain_name(terrain).to_string(),
            elevation: biome.elevation(),
            eco_rating: earth::eco_rating_for(terrain),
            is_polluted: false,
            plant: None,
            planted_by: None,
            plots: None,
            cleaned_at: None,
            last_interaction: 0,
        });
        true
    };
    crate::objects::ensure_objects(ctx, hex_id, biome.terrain());
    inserted
}

/// Materialize a hex-shaped neighborhood of tiles around `(q, r)`; returns
/// how many rows were newly created.
pub fn ensure_tiles_around(ctx: &ReducerContext, q: i32, r: i32) -> usize {
    let mut count = 0usize;
    for dr in -ENSURE_RADIUS..=ENSURE_RADIUS {
        for dq in -(ENSURE_RADIUS + dr)..=(ENSURE_RADIUS + dr) {
            // Hex-shaped window: |dq| <= R, |dr| <= R, |dq+dr| <= R.
            if (-ENSURE_RADIUS..=ENSURE_RADIUS).contains(&(dq + dr))
                && ensure_hex(ctx, q + dq, r + dr)
            {
                count += 1;
            }
        }
    }
    count
}

/// Earth biome under a tile center (`None` outside the mapped planet).
///
/// The sampling point MUST be the TILE_SIZE-scaled tile center — the same
/// point `world_gen::generate_cell` streams to the client. Sampling at the
/// old 10.0-unit hex basis put server rows on the wrong Earth point (a
/// visible-land tile materialized as "Water", blocking capital placement).
fn biome_for_hex(q: i32, r: i32) -> Option<idlecore_core::earth::Biome> {
    let (wx, wy) = WorldGrid::tile_center(q, r, TILE_W);
    earth::biome_at(wx, wy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_earth_points_resolve_to_expected_terrain_strings() {
        use idlecore_core::terrain::TerrainType;
        let cases = [
            ((13.0f64, 23.0f64), TerrainType::Desert),
            ((-62.0, -3.0), TerrainType::TropicalRainforest),
            ((100.0, 62.0), TerrainType::Taiga),
            ((-140.0, 0.0), TerrainType::Water),
            ((-90.0, 45.0), TerrainType::Forest),
        ];
        for &((lon, lat), want) in &cases {
            let (x, y) = earth::lonlat_to_world(lon, lat);
            let t = earth::terrain_at(x, y).unwrap();
            assert_eq!(t, want);
            assert_eq!(earth::terrain_name(t), earth::terrain_name(want));
        }
    }

    #[test]
    fn e2e_plant_destination_is_forest_on_the_planet_plane() {
        use idlecore_core::terrain::TerrainType;
        // The exact spot the e2e binary teleports to must be plantable.
        let (x, y) = earth::lonlat_to_world(-90.0, 45.0);
        assert!(matches!(
            earth::terrain_at(x, y),
            Some(TerrainType::Forest) | Some(TerrainType::Grass)
        ));
    }

    #[test]
    fn biome_for_hex_matches_the_streamed_tile_scale() {
        // Server materialization and client streaming must read the same
        // Earth point: the TILE_SIZE-scaled tile center. Near coasts roughly
        // a third of tiles flip between land and water when sampled at any
        // other basis, so agreement over a coastal window pins the scale.
        let cell_q = idlecore_core::earth::spawn_tile().0;
        let cell_r = idlecore_core::earth::spawn_tile().1;
        // Sanity: at the stale 10.0-unit basis the same window disagrees —
        // otherwise this test could pass with a still-wrong scale.
        let mut stale_mismatches = 0;
        let mut squashed = false;
        let mut matched = 0;
        for dq in -256i32..=256 {
            for dr in [-3i32, 0, 3] {
                let (q, r) = (cell_q + dq, cell_r + dr);
                let want = {
                    let (wx, wy) = WorldGrid::tile_center(q, r, TILE_W);
                    earth::biome_at(wx, wy).map(|b| earth::terrain_name(b.terrain()))
                };
                match biome_for_hex(q, r) {
                    Some(b) if Some(earth::terrain_name(b.terrain())) == want => matched += 1,
                    _ => squashed = true,
                }
                let (hx, hy) = WorldGrid::tile_center(q, r, 10.0);
                if earth::biome_at(hx, hy).map(|b| earth::terrain_name(b.terrain())) != want {
                    stale_mismatches += 1;
                }
            }
        }
        assert!(!squashed, "server biome diverged from the streamed cell scale");
        assert!(stale_mismatches > 0, "sampling scale polarity unclear");
        assert!(matched > 1000, "window did not exercise enough tiles");
    }
}
