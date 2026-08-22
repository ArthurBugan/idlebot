//! World tile generation — the real-Earth replica at 1:100 scale.
//!
//! The planet spans ~308M hexes, so tiles are materialized lazily: a
//! `hex_tile` row is created from `idlecore_core::earth` data the first time
//! anything needs it (movement, interactions, teleport, login). Rows are
//! immutable terrain snapshots; plants/pollution mutate on top later.

use spacetimedb::{ReducerContext, Table};
use crate::types::{hex_id_of, hex_tile, HexTile};
use idlecore_core::earth;
use idlecore_core::hex_grid::HexGrid;

/// Hex circumradius in world units — matches the client and core grid math.
const HEX_R: f32 = 10.0;

/// Radius (in hexes) of tiles materialized around a player position. Covers
/// interaction range (1) plus the HUD/eco lookups for the visible ring.
pub const ENSURE_RADIUS: i32 = 3;

/// Ensure the `hex_tile` row for one axial coordinate exists, creating it
/// from Earth data when missing. Also rolls the hex's natural resource nodes
/// (grass/rocks — never trees). Returns true if a tile row was inserted.
pub fn ensure_hex(ctx: &ReducerContext, q: i32, r: i32) -> bool {
    let Some(biome) = biome_for_hex(q, r) else { return false };
    let hex_id = hex_id_of(q, r);
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

/// Earth biome under a hex center (`None` outside the mapped planet).
fn biome_for_hex(q: i32, r: i32) -> Option<idlecore_core::earth::Biome> {
    let (wx, wy) = HexGrid::axial_to_world(q, r, HEX_R);
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
}
