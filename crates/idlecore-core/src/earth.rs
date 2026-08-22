//! Real-Earth map data — a 1:100 replica of the whole planet.
//!
//! The world plane is an equirectangular projection of Earth at 1:100 scale
//! (1 world unit = 100 m): x spans the equatorial circumference
//! (±180° lon), y spans pole-to-pole (±90° lat). Biomes come from a compact
//! class raster generated offline from NASA Blue Marble + Natural Earth data
//! (`examples/earthgen.rs`), and cities from Natural Earth populated places.
//!
//! Both the client and the SpacetimeDB server embed the same assets, so a
//! hex's terrain is identical everywhere and generation stays deterministic.
//! This module is dependency-free (no Bevy) so the server can use it too.

use crate::terrain::TerrainType;
use crate::world_gen::WaterClass;
use std::sync::OnceLock;

/// Half-width of the map in world units (equator ÷ 2 ÷ 100).
pub const EARTH_HALF_W_UNITS: f64 = 200_375.085;
/// Half-height of the map in world units (meridian ÷ 2 ÷ 100).
pub const EARTH_HALF_H_UNITS: f64 = 100_009.83;

pub const RASTER_W: u32 = 1024;
pub const RASTER_H: u32 = 512;

static BIOME_RASTER: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/earth_biomes.bin"));
static CITIES_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/earth_cities.json"));

/// Biome classes encoded in [`BIOME_RASTER`] (indices fixed by the generator).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Biome {
    Water = 0,
    PolarIce = 1,
    Tundra = 2,
    Taiga = 3,
    Forest = 4,
    Rainforest = 5,
    Grassland = 6,
    Desert = 7,
    Mountain = 8,
    Grass = 9,
    City = 10,
    Lake = 11,
}

impl Biome {
    pub fn from_index(i: u8) -> Option<Self> {
        Some(match i {
            0 => Self::Water,
            1 => Self::PolarIce,
            2 => Self::Tundra,
            3 => Self::Taiga,
            4 => Self::Forest,
            5 => Self::Rainforest,
            6 => Self::Grassland,
            7 => Self::Desert,
            8 => Self::Mountain,
            9 => Self::Grass,
            10 => Self::City,
            11 => Self::Lake,
            _ => return None,
        })
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn terrain(self) -> TerrainType {
        match self {
            Self::Water | Self::Lake => TerrainType::Water,
            Self::PolarIce | Self::Tundra => TerrainType::Tundra,
            Self::Taiga => TerrainType::Taiga,
            Self::Forest => TerrainType::Forest,
            Self::Rainforest => TerrainType::TropicalRainforest,
            Self::Grassland => TerrainType::Grassland,
            Self::Desert => TerrainType::Desert,
            Self::Mountain => TerrainType::Mountain,
            Self::Grass => TerrainType::Grass,
            Self::City => TerrainType::City,
        }
    }

    pub fn water_class(self) -> WaterClass {
        match self {
            Self::Water => WaterClass::Ocean,
            Self::Lake => WaterClass::Lake,
            _ => WaterClass::None,
        }
    }

    /// Elevation proxy in [0, 1] for cell fields / visuals.
    pub fn elevation(self) -> f32 {
        match self {
            Self::Water => 0.15,
            Self::Lake => 0.2,
            Self::PolarIce => 0.35,
            Self::Tundra => 0.3,
            Self::Taiga => 0.45,
            Self::Forest => 0.45,
            Self::Rainforest => 0.35,
            Self::Grassland => 0.35,
            Self::Desert => 0.25,
            Self::Mountain => 0.85,
            Self::Grass => 0.4,
            Self::City => 0.4,
        }
    }

    /// Moisture proxy in [0, 1] for cell fields / visuals.
    pub fn moisture(self) -> f32 {
        match self {
            Self::Water | Self::Lake => 1.0,
            Self::PolarIce => 0.15,
            Self::Tundra => 0.25,
            Self::Taiga => 0.6,
            Self::Forest => 0.75,
            Self::Rainforest => 0.95,
            Self::Grassland => 0.4,
            Self::Desert => 0.05,
            Self::Mountain => 0.35,
            Self::Grass => 0.6,
            Self::City => 0.5,
        }
    }
}

/// Convert game-world coordinates to (lon, lat) degrees on real Earth.
pub fn world_to_lonlat(x: f32, y: f32) -> (f64, f64) {
    (
        x as f64 / EARTH_HALF_W_UNITS * 180.0,
        -(y as f64 / EARTH_HALF_H_UNITS * 90.0),
    )
}

/// Convert (lon, lat) degrees on real Earth to game-world coordinates.
pub fn lonlat_to_world(lon: f64, lat: f64) -> (f32, f32) {
    (
        (lon * EARTH_HALF_W_UNITS / 180.0) as f32,
        (-lat * EARTH_HALF_H_UNITS / 90.0) as f32,
    )
}

/// Default spawn longitude/latitude: solid temperate forest (North America),
/// chosen so new players start on plantable land rather than mid-ocean.
pub const SPAWN_LON: f64 = -90.0;
pub const SPAWN_LAT: f64 = 45.0;

/// Axial coordinates of the spawn hex.
pub fn spawn_hex() -> (i32, i32) {
    let (x, y) = lonlat_to_world(SPAWN_LON, SPAWN_LAT);
    crate::hex_grid::HexGrid::world_to_axial(x, y, crate::world_gen::WorldGenConfig::HEX_SIZE)
}

/// True when the world position lies on the planet and is not open water.
pub fn is_land_at(x: f32, y: f32) -> bool {
    matches!(
        biome_at(x, y),
        Some(Biome::PolarIce)
            | Some(Biome::Tundra)
            | Some(Biome::Taiga)
            | Some(Biome::Forest)
            | Some(Biome::Rainforest)
            | Some(Biome::Grassland)
            | Some(Biome::Desert)
            | Some(Biome::Mountain)
            | Some(Biome::Grass)
            | Some(Biome::City)
    )
}

/// Nearest hex whose center sits on land, found by expanding axial rings
/// around `(q, r)`. `max_rings` bounds the search (`None` past it — mid-ocean
/// points like Point Nemo are thousands of hexes from any coast).
pub fn nearest_land_hex(q: i32, r: i32, max_rings: u32) -> Option<(i32, i32)> {
    // Proper cyclic cube directions: each is the previous rotated 60°, so
    // walking each one `ring` times traces exactly one full axial ring.
    const RING_DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
    let size = crate::world_gen::WorldGenConfig::HEX_SIZE;
    for ring in 0..=max_rings {
        let mut cq = q + RING_DIRS[3].0 * ring as i32;
        let mut cr = r + RING_DIRS[3].1 * ring as i32;
        for dir in RING_DIRS {
            for _ in 0..ring {
                let (x, y) = crate::hex_grid::HexGrid::axial_to_world(cq, cr, size);
                if is_land_at(x, y) {
                    return Some((cq, cr));
                }
                cq += dir.0;
                cr += dir.1;
            }
        }
    }
    None
}

/// Spawn hex guaranteed to be land: the preferred spawn when it is on land,
/// otherwise the nearest land found by spiral search (falling back to the
/// preferred spawn itself if the whole search radius is ocean).
pub fn resolve_spawn_hex() -> (i32, i32) {
    let (sq, sr) = spawn_hex();
    let (wx, wy) =
        crate::hex_grid::HexGrid::axial_to_world(sq, sr, crate::world_gen::WorldGenConfig::HEX_SIZE);
    if is_land_at(wx, wy) {
        return (sq, sr);
    }
    nearest_land_hex(sq, sr, 1024).unwrap_or((sq, sr))
}

/// True when the position lies inside the mapped planet bounds.
pub fn in_world_bounds(x: f32, y: f32) -> bool {
    x.abs() <= EARTH_HALF_W_UNITS as f32 && y.abs() <= EARTH_HALF_H_UNITS as f32
}

/// Nearest-pixel raster index at a world position (`None` outside the map).
fn raster_index(x: f32, y: f32) -> Option<usize> {
    if !in_world_bounds(x, y) || BIOME_RASTER.len() != (RASTER_W * RASTER_H) as usize {
        return None;
    }
    let px = (((x as f64 + EARTH_HALF_W_UNITS) / (2.0 * EARTH_HALF_W_UNITS))
        .clamp(0.0, 0.999_999)
        * RASTER_W as f64) as u32;
    let py = (((y as f64 + EARTH_HALF_H_UNITS) / (2.0 * EARTH_HALF_H_UNITS))
        .clamp(0.0, 0.999_999)
        * RASTER_H as f64) as u32;
    Some((py * RASTER_W + px) as usize)
}

/// Sample the Earth biome at a world position (`None` outside the map).
pub fn biome_at(x: f32, y: f32) -> Option<Biome> {
    let i = raster_index(x, y)?;
    // Unknown indices degrade to ocean rather than failing lookups.
    Some(Biome::from_index(BIOME_RASTER[i]).unwrap_or(Biome::Water))
}

/// Sample the gameplay terrain type at a world position.
pub fn terrain_at(x: f32, y: f32) -> Option<TerrainType> {
    biome_at(x, y).map(Biome::terrain)
}

/// Latitude-based temperature in [-1, 1]: hot at the equator, frigid at the poles.
pub fn temperature_at(x: f32, y: f32) -> f32 {
    let (_, lat) = world_to_lonlat(x, y);
    let t = (90.0 - lat.abs()) / 90.0 * 1.6 - 0.3;
    t.clamp(-1.0, 1.0) as f32
}

/// Server-side terrain string for a terrain type (matches interaction gates).
pub fn terrain_name(t: TerrainType) -> &'static str {
    match t {
        TerrainType::Grass => "Grass",
        TerrainType::Forest => "Forest",
        TerrainType::Water => "Water",
        TerrainType::City => "City",
        TerrainType::Desert => "Desert",
        TerrainType::Polluted => "Polluted",
        TerrainType::Tundra => "Tundra",
        TerrainType::Taiga => "Taiga",
        TerrainType::Grassland => "Grassland",
        TerrainType::TropicalRainforest => "TropicalRainforest",
        TerrainType::Mountain => "Mountain",
    }
}

/// Baseline eco rating for pristine terrain of each type.
pub fn eco_rating_for(t: TerrainType) -> i32 {
    match t {
        TerrainType::Grass => 50,
        TerrainType::Grassland => 45,
        TerrainType::Forest => 55,
        TerrainType::Taiga => 45,
        TerrainType::TropicalRainforest => 65,
        TerrainType::Tundra => 20,
        TerrainType::Desert => 12,
        TerrainType::Mountain => 25,
        TerrainType::City => 10,
        TerrainType::Water => 30,
        TerrainType::Polluted => 5,
    }
}

/// Gatherable materials on a terrain: `(name, yield per action)`.
pub fn materials_for(t: TerrainType) -> &'static [(&'static str, u32)] {
    match t {
        TerrainType::Grass => &[("Wheat", 3), ("Fiber", 2)],
        TerrainType::Forest => &[("Wood", 3), ("Resin", 1)],
        TerrainType::Taiga => &[("Timber", 2), ("Fur", 2)],
        TerrainType::TropicalRainforest => &[("Exotic Fruit", 2), ("Medicinal Herbs", 2)],
        TerrainType::Grassland => &[("Wheat", 2), ("Hides", 1)],
        TerrainType::Desert => &[("Sandstone", 2), ("Rare Minerals", 1)],
        TerrainType::Mountain => &[("Ore", 2), ("Stone", 3)],
        TerrainType::Tundra => &[("Stone", 2), ("Arctic Moss", 1)],
        TerrainType::City => &[("Scrap", 3), ("Components", 2)],
        TerrainType::Polluted => &[("Toxic Waste", 2), ("Scrap", 2)],
        TerrainType::Water => &[("Fish", 3), ("Algae", 1)],
    }
}

/// [`materials_for`] by server terrain string (HUD convenience).
pub fn materials_for_name(terrain: &str) -> &'static [(&'static str, u32)] {
    let all = [
        (TerrainType::Grass, materials_for(TerrainType::Grass)),
        (TerrainType::Forest, materials_for(TerrainType::Forest)),
        (TerrainType::Taiga, materials_for(TerrainType::Taiga)),
        (
            TerrainType::TropicalRainforest,
            materials_for(TerrainType::TropicalRainforest),
        ),
        (TerrainType::Grassland, materials_for(TerrainType::Grassland)),
        (TerrainType::Desert, materials_for(TerrainType::Desert)),
        (TerrainType::Mountain, materials_for(TerrainType::Mountain)),
        (TerrainType::Tundra, materials_for(TerrainType::Tundra)),
        (TerrainType::City, materials_for(TerrainType::City)),
        (TerrainType::Polluted, materials_for(TerrainType::Polluted)),
        (TerrainType::Water, materials_for(TerrainType::Water)),
    ];
    all.iter()
        .find(|(t, _)| terrain_name(*t) == terrain)
        .map(|(_, m)| *m)
        .unwrap_or(&[])
}

/// A real city on the map (Natural Earth populated places).
#[derive(Debug, Clone)]
pub struct EarthCity {
    pub name: String,
    pub population: u32,
    pub lon: f64,
    pub lat: f64,
}

fn parse_cities() -> Vec<EarthCity> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(CITIES_JSON) else {
        return Vec::new();
    };
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    Some(EarthCity {
                        name: c["n"].as_str()?.to_string(),
                        population: c["p"].as_u64()? as u32,
                        lon: c["lon"].as_f64()?,
                        lat: c["lat"].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// All embedded cities, largest first.
pub fn cities() -> &'static [EarthCity] {
    static CITIES: OnceLock<Vec<EarthCity>> = OnceLock::new();
    CITIES.get_or_init(parse_cities)
}

/// Approximate great-circle distance between two lon/lat points, in km.
fn lonlat_distance_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let dlon = (lon2 - lon1).to_radians();
    let dlat = (lat2 - lat1).to_radians();
    let mean_lat = ((lat1 + lat2) * 0.5).to_radians();
    let dx = dlon * 111.320 * mean_lat.cos();
    let dy = dlat * 110.574;
    (dx * dx + dy * dy).sqrt()
}

/// Nearest city to a world position, with its distance in real-world km.
pub fn nearest_city(x: f32, y: f32) -> Option<(&'static EarthCity, f64)> {
    if !in_world_bounds(x, y) {
        return None;
    }
    let (lon, lat) = world_to_lonlat(x, y);
    cities()
        .iter()
        .map(|c| (c, lonlat_distance_km(lon, lat, c.lon, c.lat)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sahara_is_desert() {
        let (x, y) = lonlat_to_world(13.0, 23.0);
        assert_eq!(terrain_at(x, y), Some(TerrainType::Desert));
    }

    #[test]
    fn amazon_is_rainforest() {
        let (x, y) = lonlat_to_world(-62.0, -3.0);
        assert_eq!(terrain_at(x, y), Some(TerrainType::TropicalRainforest));
    }

    #[test]
    fn siberia_is_taiga() {
        let (x, y) = lonlat_to_world(100.0, 62.0);
        assert_eq!(terrain_at(x, y), Some(TerrainType::Taiga));
    }

    #[test]
    fn pacific_is_water() {
        let (x, y) = lonlat_to_world(-140.0, 0.0);
        assert_eq!(terrain_at(x, y), Some(TerrainType::Water));
        assert_eq!(biome_at(x, y), Some(Biome::Water));
    }

    #[test]
    fn ice_sheets_are_tundra() {
        for (lon, lat) in [(-42.0, 72.0), (20.0, -75.0)] {
            let (x, y) = lonlat_to_world(lon, lat);
            assert_eq!(
                terrain_at(x, y),
                Some(TerrainType::Tundra),
                "ice at {lon},{lat}"
            );
        }
    }

    #[test]
    fn big_cities_are_city_terrain() {
        for (lon, lat) in [(2.35, 48.85), (-99.13, 19.43), (31.24, 30.04)] {
            let (x, y) = lonlat_to_world(lon, lat);
            assert_eq!(
                terrain_at(x, y),
                Some(TerrainType::City),
                "city at {lon},{lat}"
            );
        }
    }

    #[test]
    fn outside_map_is_none() {
        assert_eq!(biome_at(EARTH_HALF_W_UNITS as f32 * 2.0, 0.0), None);
        assert_eq!(biome_at(0.0, EARTH_HALF_H_UNITS as f32 * 2.0), None);
    }

    #[test]
    fn mapping_round_trips() {
        for &(lon, lat) in &[(0.0, 0.0), (179.9, 89.9), (-120.5, -33.2), (42.0, 12.0)] {
            let (x, y) = lonlat_to_world(lon, lat);
            let (lo, la) = world_to_lonlat(x, y);
            assert!((lo - lon).abs() < 0.01 && (la - lat).abs() < 0.01);
        }
    }

    #[test]
    fn every_terrain_has_materials_and_names_agree() {
        let terrains = [
            TerrainType::Grass,
            TerrainType::Forest,
            TerrainType::Water,
            TerrainType::City,
            TerrainType::Desert,
            TerrainType::Polluted,
            TerrainType::Tundra,
            TerrainType::Taiga,
            TerrainType::Grassland,
            TerrainType::TropicalRainforest,
            TerrainType::Mountain,
        ];
        for t in terrains {
            assert!(!materials_for(t).is_empty(), "{t:?} has no materials");
            assert!(!materials_for_name(terrain_name(t)).is_empty());
            assert!(eco_rating_for(t) > 0);
        }
    }

    #[test]
    fn cities_embedded_and_nearest_works() {
        let all = cities();
        assert!(all.len() > 3000, "expected thousands of cities");
        assert!(all.iter().any(|c| c.name == "Tokyo"));
        // At Paris itself, Paris must be nearest.
        let (x, y) = lonlat_to_world(2.35, 48.85);
        let (city, dist) = nearest_city(x, y).expect("in bounds");
        assert_eq!(city.name, "Paris", "got {city:?}");
        assert!(dist < 100.0, "Paris within ~100 km, got {dist}");
    }

    #[test]
    fn temperature_hot_at_equator_cold_at_poles() {
        let (ex, ey) = lonlat_to_world(0.0, 0.0);
        let (px, py) = lonlat_to_world(0.0, 88.0);
        assert!(temperature_at(ex, ey) > temperature_at(px, py));
        assert!(temperature_at(px, py) < 0.0);
    }

    #[test]
    fn spawn_is_plantable_land() {
        let (sq, sr) = spawn_hex();
        let (wx, wy) = crate::hex_grid::HexGrid::axial_to_world(
            sq,
            sr,
            crate::world_gen::WorldGenConfig::HEX_SIZE,
        );
        assert!(matches!(
            terrain_at(wx, wy),
            Some(TerrainType::Forest) | Some(TerrainType::Grass)
        ));
    }

    #[test]
    fn resolve_spawn_hex_is_land() {
        let (sq, sr) = resolve_spawn_hex();
        let (wx, wy) = crate::hex_grid::HexGrid::axial_to_world(
            sq,
            sr,
            crate::world_gen::WorldGenConfig::HEX_SIZE,
        );
        assert!(is_land_at(wx, wy));
    }

    #[test]
    fn spiral_from_open_water_reaches_the_coast() {
        // Gulf of Guinea (0°N 0°E) is open water; the African coast is a few
        // hundred axial rings away.
        let (gx, gy) = lonlat_to_world(0.0, 0.0);
        assert!(!is_land_at(gx, gy), "(0,0) should be sea");
        let (gq, gr) =
            crate::hex_grid::HexGrid::world_to_axial(gx, gy, crate::world_gen::WorldGenConfig::HEX_SIZE);
        let (lq, lr) = nearest_land_hex(gq, gr, 1024).expect("coast reachable");
        assert_ne!((gq, gr), (lq, lr));
        let (lx, ly) =
            crate::hex_grid::HexGrid::axial_to_world(lq, lr, crate::world_gen::WorldGenConfig::HEX_SIZE);
        assert!(is_land_at(lx, ly), "found hex must be land");
    }
}
