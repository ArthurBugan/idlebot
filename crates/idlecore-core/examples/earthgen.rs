//! Offline Earth-map generator — builds the embedded 1:100 Earth replica
//! assets from public-domain source data.
//!
//! Inputs (download once, e.g. into /tmp/earth):
//!   1. NASA Blue Marble surface raster (equirectangular 2048×1024):
//!      https://raw.githubusercontent.com/mrdoob/three.js/r128/examples/textures/planets/earth_atmos_2048.jpg
//!   2. Matching land/ocean specular mask (white = land, black = water):
//!      https://raw.githubusercontent.com/mrdoob/three.js/r128/examples/textures/planets/earth_specular_2048.jpg
//!   3. Natural Earth populated places (GeoJSON, public domain):
//!      https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_populated_places_simple.geojson
//!
//! Outputs (written into `crates/idlecore-core/assets/`, embedded by
//! `src/earth.rs` via include_bytes!/include_str!):
//!   - earth_biomes.bin : W×H raw u8 class indices (Biome enum)
//!   - earth_cities.json: compact city list {n,p,lon,lat}
//!
//! Run: cargo run -p idlecore-core --example earthgen -- \
//!        /tmp/earth/earth_atmos_2048.jpg /tmp/earth/earth_specular_2048.jpg \
//!        /tmp/earth/ne_10m_populated_places_simple.geojson

use image::GenericImageView;
use serde_json::Value;
use std::collections::HashMap;

const W: u32 = 1024;
const H: u32 = 512;

// Biome class indices (must match `earth::Biome`).
const WATER: u8 = 0;
const POLAR_ICE: u8 = 1;
const TUNDRA: u8 = 2;
const TAIGA: u8 = 3;
const FOREST: u8 = 4;
const RAINFOREST: u8 = 5;
const GRASSLAND: u8 = 6;
const DESERT: u8 = 7;
const MOUNTAIN: u8 = 8;
const GRASS: u8 = 9;
const CITY: u8 = 10;
const LAKE: u8 = 11;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: earthgen <atmos.jpg> <specular.jpg> <cities.geojson> [out_dir]");
        std::process::exit(2);
    }
    let out_dir = args.get(4).cloned().unwrap_or_else(|| "assets".to_string());

    let atmos = image::open(&args[1]).expect("open atmos jpg").to_rgb8();
    let spec = image::open(&args[2]).expect("open specular jpg").to_rgb8();
    let (aw, ah) = atmos.dimensions();
    let (sw, sh) = spec.dimensions();
    println!("source: atmos {aw}x{ah}, spec {sw}x{sh}");

    // Calibration probe: raw colors at known biomes.
    for s in SAMPLES {
        let x = ((s.lon + 180.0) / 360.0 * aw as f64) as u32;
        let y = ((90.0 - s.lat) / 180.0 * ah as f64) as u32;
        let p = atmos.get_pixel(x.min(aw - 1), y.min(ah - 1)).0;
        let q = spec.get_pixel(x.min(sw - 1), y.min(sh - 1)).0;
        let lum =
            0.299 * q[0] as f32 + 0.587 * q[1] as f32 + 0.114 * q[2] as f32;
        println!(
            "probe {:>12} ({:>5},{:>5}) rgb=({:>3},{:>3},{:>3}) spec_lum={:.0}",
            s.expect, s.lon, s.lat, p[0], p[1], p[2], lum
        );
    }

    let mut raster = vec![0u8; (W * H) as usize];
    let mut counts: HashMap<u8, usize> = HashMap::new();

    for y in 0..H {
        for x in 0..W {
            // Nearest sample from the 2048×1024 sources; v flipped so row 0
            // is +90° lat (north-up equirectangular).
            let sx = ((x as f32 + 0.5) / W as f32 * aw as f32) as u32;
            let sy = ((y as f32 + 0.5) / H as f32 * ah as f32) as u32;
            let px = atmos.get_pixel(sx.min(aw - 1), sy.min(ah - 1));
            let sp = spec.get_pixel(sx.min(sw - 1), sy.min(sh - 1));
            let [r, g, b] = px.0;
            let land_lum =
                0.299 * sp.0[0] as f32 + 0.587 * sp.0[1] as f32 + 0.114 * sp.0[2] as f32;

            // Specular map polarity: bright = water (specular reflection).
            let cls = if land_lum < 100.0 {
                classify_land(r, g, b, y)
            } else {
                WATER
            };
            raster[(y * W + x) as usize] = cls;
            *counts.entry(cls).or_default() += 1;
        }
    }

    let cities = parse_cities(&args[3]);
    paint_cities(&mut raster, &cities);

    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(format!("{out_dir}/earth_biomes.bin"), &raster).unwrap();
    std::fs::write(
        format!("{out_dir}/earth_cities.json"),
        serde_json::to_string(&cities.iter().map(city_json).collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();

    println!("class counts (after city paint):");
    for (name, cls) in [
        ("water", WATER),
        ("polar ice", POLAR_ICE),
        ("tundra", TUNDRA),
        ("taiga", TAIGA),
        ("forest", FOREST),
        ("rainforest", RAINFOREST),
        ("grassland", GRASSLAND),
        ("desert", DESERT),
        ("mountain", MOUNTAIN),
        ("grass", GRASS),
        ("city", CITY),
        ("lake", LAKE),
    ] {
        println!("  {:>12}: {}", name, count_class(&raster, cls));
    }
    println!(
        "cities embedded: {} total",
        cities.len()
    );
}

#[derive(Clone, Copy)]
struct SamplePoint {
    lon: f64,
    lat: f64,
    expect: &'static str,
}

/// Sanity samples printed after generation (lon, lat, expected biome).
const SAMPLES: [SamplePoint; 16] = [
    SamplePoint { lon: 13.0, lat: 23.0, expect: "desert" },      // Sahara
    SamplePoint { lon: -62.0, lat: -3.0, expect: "rainforest" }, // Amazon
    SamplePoint { lon: 100.0, lat: 62.0, expect: "taiga" },      // Siberia
    SamplePoint { lon: -140.0, lat: 0.0, expect: "ocean" },      // Pacific
    SamplePoint { lon: 20.0, lat: -75.0, expect: "polar" },      // Antarctica
    SamplePoint { lon: -42.0, lat: 72.0, expect: "polar" },      // Greenland
    SamplePoint { lon: 88.0, lat: 33.0, expect: "mountain" },    // Tibet
    SamplePoint { lon: -102.0, lat: 41.0, expect: "grassland" }, // US prairie
    SamplePoint { lon: 15.0, lat: 13.0, expect: "grassland" },   // Sahel
    SamplePoint { lon: 10.0, lat: 51.0, expect: "forest" },      // Germany
    SamplePoint { lon: 133.0, lat: -25.0, expect: "desert" },    // Outback
    SamplePoint { lon: 22.0, lat: -23.0, expect: "desert" },     // Kalahari
    SamplePoint { lon: 110.0, lat: -1.0, expect: "rainforest" }, // Borneo
    SamplePoint { lon: 115.0, lat: 35.0, expect: "grass" },      // N-China plain
    SamplePoint { lon: -95.0, lat: 52.0, expect: "taiga" },      // Canada
    SamplePoint { lon: 2.35, lat: 48.85, expect: "city" },       // Paris
];

/// Classify one land pixel from Blue Marble color + latitude band.
///
/// The three.js Blue Marble render is dark and atmospherically hazy, so the
/// thresholds are tuned to its actual palette (see probe output): tropical
/// forest is near-black with a slight green cast, boreal forest dark
/// blue-green, desert bright tan.
fn classify_land(r: u8, g: u8, b: u8, y: u32) -> u8 {
    let rf = r as f32;
    let gf = g as f32;
    let bf = b as f32;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let brightness = (rf + gf + bf) / 3.0;
    let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
    // Row 0 = north pole.
    let lat = 90.0 - (y as f32 + 0.5) / H as f32 * 180.0;

    // Ice sheets: very bright, low saturation (Greenland, Antarctica).
    if brightness > 175.0 && sat < 0.35 && lat.abs() > 55.0 {
        return POLAR_ICE;
    }

    let green_dominant = gf > rf && gf >= bf - 12.0;

    if lat.abs() < 24.0 && brightness < 75.0 && !green_dominant_or_bright(gf, brightness) {
        return RAINFOREST;
    }
    if lat.abs() >= 48.0 && brightness < 90.0 {
        return TAIGA;
    }
    if green_dominant {
        if brightness < 110.0 {
            FOREST
        } else {
            GRASS
        }
    } else if r > g {
        if sat >= 0.26 && brightness > 90.0 {
            DESERT
        } else if sat <= 0.13 && (80.0..170.0).contains(&brightness) {
            MOUNTAIN
        } else {
            GRASSLAND
        }
    } else {
        GRASSLAND
    }
}

/// Tropical rainforest check helper: rainforest pixels are dark but not deep
/// water-dark; anything clearly green and mid-bright is regular forest.
fn green_dominant_or_bright(g: f32, brightness: f32) -> bool {
    g > 60.0 || brightness > 55.0
}

fn count_class(raster: &[u8], cls: u8) -> usize {
    raster.iter().filter(|&&v| v == cls).count()
}

fn parse_cities(path: &str) -> Vec<(String, u32, f64, f64)> {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read geojson")).unwrap();
    let mut out = Vec::new();
    for f in data["features"].as_array().expect("features") {
        let props = &f["properties"];
        let name = props["name"]
            .as_str()
            .or(props["nameascii"].as_str())
            .unwrap_or("?")
            .to_string();
        let pop = props["pop_max"].as_u64().unwrap_or(0) as u32;
        let coords = f["geometry"]["coordinates"].as_array().expect("coords");
        let lon = coords[0].as_f64().unwrap_or(0.0);
        let lat = coords[1].as_f64().unwrap_or(0.0);
        if !name.is_empty() && name != "?" {
            out.push((name, pop, lon, lat));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

fn city_json(c: &(String, u32, f64, f64)) -> Value {
    serde_json::json!({"n": c.0, "p": c.1, "lon": c.2, "lat": c.3})
}

/// Paint City pixels over land so hex sampling picks up urban terrain.
/// Radius in pixels (~39 km/px at this resolution), by population tier.
fn paint_cities(raster: &mut [u8], cities: &[(String, u32, f64, f64)]) {
    for (_, pop, lon, lat) in cities {
        let radius: i32 = match *pop {
            p if p >= 10_000_000 => 3,
            p if p >= 2_000_000 => 2,
            p if p >= 500_000 => 1,
            p if p >= 100_000 => 1,
            _ => continue,
        };
        let cx = ((lon + 180.0) / 360.0 * W as f64) as i32;
        let cy = ((90.0 - lat) / 180.0 * H as f64) as i32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius + 1 {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
                    continue;
                }
                let i = (y as u32 * W + x as u32) as usize;
                // Cities sit on land only (never paint over sea/lake).
                if matches!(raster[i], WATER | LAKE | POLAR_ICE) {
                    continue;
                }
                raster[i] = CITY;
            }
        }
    }
}
