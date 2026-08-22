//! Renders the streamed hex world as 2D isometric tile sprites.
//!
//! One parent entity per loaded chunk, with per-hex `Sprite` children built
//! from the `Isometric Miniature` packs (`assets/models/.../`): hand-drawn
//! 256px diamonds (Dungeon dirt/stone, Prototype grass/floor) that need little
//! upscaling at the default zoom, plus generated flat-blue water diamonds
//! (crisp at any zoom, shaded by water class) and pine-tree overlays on
//! forested terrain. Chunks are spawned lazily as they stream in and despawned
//! when they leave the rendered radius.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use spacetimedb_sdk::Table;
use std::collections::HashMap;
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::terrain::TerrainType;
use idlecore_core::world_gen::{WorldGenConfig, WaterClass, hex_to_chunk_coord};
use crate::player::PlayerTransform;
use crate::plugins::world::StreamingWorldResource;

/// Overlap factor applied to floor hexagons: real hexagons interlock
/// exactly, so only a hair of overscale is needed to guarantee neighbouring
/// antialiased edges always overlap instead of revealing background seams.
const FLOOR_OVERLAP: f32 = 1.03;

/// Floor hex circumradius in world units (hex radius 10 × overlap).
const TILE_R: f32 = WorldGenConfig::HEX_SIZE * FLOOR_OVERLAP;

/// Floor sprite size: bounding box of a pointy-top hexagon (width √3·R,
/// height 2·R). Taller than the 1.5·s row pitch — hexes interlock vertically,
/// stacked south-over-north by the draw-order depth below.
const FLOOR_W: f32 = 1.7320508075688772 * TILE_R;
const FLOOR_H: f32 = 2.0 * TILE_R;

/// Decoration-art sizing base (trees/rocks/grass were tuned against this
/// scale; kept separate from the floor overlap).
const DECOR_SCALE: f32 = 1.35;
const TILE_W: f32 = 1.7320508075688772 * WorldGenConfig::HEX_SIZE * DECOR_SCALE;
const TILE_H: f32 = 1.5 * WorldGenConfig::HEX_SIZE * DECOR_SCALE;

/// Ground-band draw order: floor tiles only, south over north. Kept in its
/// own low band ([−25k, 25k]) so no tile can ever paint over a prop.
pub fn floor_depth(y: f32) -> f32 {
    -y * 0.25
}

/// Prop/entity-band draw order: trees, grass, rocks, players, VFX. Always
/// above every floor tile ([min 30k] > floor max 25k), still ordered
/// south-over-north among themselves. The camera's clip planes (main.rs)
/// are sized to this band.
pub fn prop_depth(y: f32) -> f32 {
    130_000.0 - y
}

// Server-authoritative resource-node art (rendered from `world_object` rows).
pub(crate) const TREE_ART_PATH: &str = "models/Isometric Miniature Tiles/Isometric/treePine.png";
pub(crate) const ROCK_ART_PATH: &str =
    "models/Isometric Miniature Overworld/extracted/Isometric/grassStoneLarge_S.png";
pub(crate) const GRASS_TUFT_PATHS: [&str; 2] = [
    "models/Isometric Nature/PNG/naturePack_011_2.png",
    "models/Isometric Nature/PNG/naturePack_012_3.png",
];

// ============================================================================
// Action-target box (Stardew-style ground cursor)
// ============================================================================

/// Hex currently targeted for ground actions (plant/harvest/gather/clean),
/// driven by the mouse cursor; defaults to the player's own hex.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActionTarget {
    pub q: i32,
    pub r: i32,
}

#[derive(Component)]
pub(crate) struct ActionBoxMarker;

/// Hollow hexagon outline (the targeting box), AA edges like the fill tiles.
fn hex_outline_image(width: u32, height: u32, srgb: [f32; 3], thickness_px: f32) -> Image {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let [r, g, b] = srgb.map(|c| (c * 255.0).round() as u8);
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = height as f32 / 2.0;
    let apothem = radius * 1.7320508075688772 / 2.0;
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5 - cx;
            let py = y as f32 + 0.5 - cy;
            let m = px
                .abs()
                .max((0.5 * px + 0.8660254037844386 * py).abs())
                .max((0.5 * px - 0.8660254037844386 * py).abs());
            let inside = apothem - m;
            // Opaque band centered on the edge, fading over ~1 px each side.
            let half = thickness_px * 0.5 + 0.5;
            let alpha = (half - inside.abs()).clamp(0.0, 1.0);
            let i = ((y * width + x) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = (alpha * 255.0).round() as u8;
        }
    }
    Image::new(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn spawn_action_box(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let w = (FLOOR_W * 12.0).round() as u32;
    let h = (FLOOR_H * 12.0).round() as u32;
    let handle = images.add(hex_outline_image(w, h, [1.0, 0.95, 0.5], 3.0));
    commands.spawn((
        Name::new("action-box"),
        ActionBoxMarker,
        Sprite {
            image: handle,
            custom_size: Some(Vec2::new(FLOOR_W * 1.04, FLOOR_H * 1.04)),
            ..default()
        },
        Transform::from_xyz(f32::MAX, f32::MAX, 0.0),
        Visibility::Hidden,
    ));
}

/// Drive the targeting box from the cursor and refresh the target resource.
#[allow(clippy::type_complexity)]
pub fn update_action_box(
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    widgets: Query<&Interaction>,
    minimap_state: Res<crate::minimap::MinimapState>,
    player_transform: Res<PlayerTransform>,
    mut target: ResMut<ActionTarget>,
    mut box_q: Query<
        (&mut Transform, &mut Sprite, &mut Visibility),
        (With<ActionBoxMarker>, Without<Camera2d>),
    >,
) {
    let Ok((mut transform, mut sprite, mut visibility)) = box_q.single_mut() else { return };

    // Default target: the hex the player stands on.
    let mut next = world_pos_to_hex(
        player_transform.translation.x,
        player_transform.translation.y,
        WorldGenConfig::HEX_SIZE,
    );

    // Cursor override unless hovering UI/minimap.
    if let Ok(window) = windows.single() {
        if let Some(cursor) = window.cursor_position() {
            let over_widget =
                widgets.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
            let mm_left = window.width() - 10.0 - minimap_state.mm_size();
            let over_minimap = cursor.x >= mm_left && cursor.y >= 10.0
                && cursor.x < mm_left + minimap_state.mm_size()
                && cursor.y < 10.0 + minimap_state.mm_size();
            if !over_widget && !over_minimap {
                if let Ok((camera, cam_transform)) = cameras.single() {
                    if let Ok(world) = camera.viewport_to_world_2d(cam_transform, cursor) {
                        next = world_pos_to_hex(world.x, world.y, WorldGenConfig::HEX_SIZE);
                    }
                }
            }
        }
    }
    target.q = next.0;
    target.r = next.1;

    let (wx, wy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
        target.q,
        target.r,
        WorldGenConfig::HEX_SIZE,
    );
    transform.translation = Vec3::new(wx, wy, crate::world_floor::prop_depth(wy) + 0.4);
    *visibility = Visibility::Visible;

    // Green while within the server's 1-hex interaction range, dim otherwise.
    let player_hex = world_pos_to_hex(
        player_transform.translation.x,
        player_transform.translation.y,
        WorldGenConfig::HEX_SIZE,
    );
    let dq = (target.q - player_hex.0).abs();
    let dr = (target.r - player_hex.1).abs();
    let ds = ((target.q + target.r) - (player_hex.0 + player_hex.1)).abs();
    let in_range = dq.max(dr).max(ds) <= 1;
    let tint = if in_range { Color::srgba(0.6, 1.0, 0.5, 0.95) } else { Color::srgba(0.9, 0.9, 0.9, 0.35) };
    sprite.color = tint;
}

// ============================================================================
// Prop textures: generated tuft/icons + loaded tree/rock art
// ============================================================================

/// Handles + aspects for every prop sprite, built once at startup.
#[derive(Resource)]
pub struct PropTextures {
    pub grass: Handle<Image>,
    pub grass_aspect: f32,
    pub tree: Handle<Image>,
    pub tree_aspect: f32,
    pub sapling: Handle<Image>,
    pub sapling_aspect: f32,
    pub rock: Handle<Image>,
    pub rock_aspect: f32,
    /// Square UI icons for inventory slots.
    pub icon_seed: Handle<Image>,
    pub icon_wood: Handle<Image>,
    pub icon_stone: Handle<Image>,
    pub icon_grass: Handle<Image>,
}

pub(crate) const SAPLING_ART_PATH: &str =
    "models/Isometric Miniature Prototype/Isometric/treePineSmall_S.png";

/// Build all prop sprites once. Runs at Startup.
pub fn init_prop_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let tuft = images.add(grass_tuft_image(128));
    commands.insert_resource(PropTextures {
        grass: tuft.clone(),
        grass_aspect: 1.0,
        tree: asset_server.load(TREE_ART_PATH),
        tree_aspect: 256.0 / 216.0,
        sapling: asset_server.load(SAPLING_ART_PATH),
        sapling_aspect: 256.0 / 512.0,
        rock: asset_server.load(ROCK_ART_PATH),
        rock_aspect: 256.0 / 512.0,
        icon_seed: asset_server.load(SAPLING_ART_PATH),
        icon_wood: images.add(log_icon_image(64)),
        icon_stone: images.add(stone_icon_image(64)),
        icon_grass: tuft,
    });
}

/// Stylized grass tuft: tapered blades fanning from a base point in three
/// shades of green. Deterministic — same canvas every run.
fn grass_tuft_image(size_px: u32) -> Image {
    let size = size_px as usize;
    let mut pixels = vec![0u8; size * size * 4];
    let cx = size as f32 * 0.5;
    let base_y = size as f32 * 0.92;
    let shades: [(u8, u8, u8); 3] = [(45, 96, 36), (62, 124, 47), (86, 160, 58)];
    // Seven broad blades: angle spread, length and lean vary per blade.
    for i in 0..7usize {
        let t = i as f32 / 6.0; // 0..1 across the fan
        let ang = -0.8 + t * 1.6 + ((i * 37) % 7) as f32 * 0.02;
        let len = size as f32 * (0.55 + 0.38 * (1.0 - (t - 0.5).abs() * 1.2).max(0.25));
        let w0 = (size as f32) * 0.11;
        let (r, g, b) = shades[i % 3];
        for yy in 0..size {
            for xx in 0..size {
                let dx = xx as f32 - cx;
                let dy = base_y - yy as f32; // up-positive
                if dy <= 0.0 || dy > len {
                    continue;
                }
                // Rotate into blade space.
                let ca = (-ang).cos();
                let sa = (-ang).sin();
                let u = dx * sa + dy * ca;
                let v = dx * ca - dy * sa;
                if u < 0.0 {
                    continue;
                }
                let half = w0 * 0.5 * (1.0 - u / len);
                if v.abs() <= half.max(0.6) {
                    let k = (yy * size + xx) * 4;
                    pixels[k] = r;
                    pixels[k + 1] = g;
                    pixels[k + 2] = b;
                    pixels[k + 3] = 255;
                }
            }
        }
    }
    Image::new(
        Extent3d { width: size_px, height: size_px, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Two stacked logs — Wood icon.
fn log_icon_image(size_px: u32) -> Image {
    let s = size_px as usize;
    let mut px = vec![0u8; s * s * 4];
    let draw_log = |px: &mut [u8], cy: f32| {
        let x0 = s as f32 * 0.12;
        let x1 = s as f32 * 0.88;
        let half_h = s as f32 * 0.11;
        for y in 0..s {
            for x in 0..s {
                let fx = x as f32 + 0.5;
                let fy = y as f32 + 0.5;
                if fx >= x0 && fx <= x1 && (fy - cy).abs() <= half_h {
                    let k = (y * s + x) * 4;
                    let end = fx < x0 + half_h * 1.6;
                    let (r, g, b) = if end { (200, 155, 106) } else { (138, 90, 43) };
                    px[k] = r; px[k + 1] = g; px[k + 2] = b; px[k + 3] = 255;
                }
            }
        }
    };
    draw_log(&mut px, s as f32 * 0.38);
    draw_log(&mut px, s as f32 * 0.64);
    Image::new(
        Extent3d { width: size_px, height: size_px, depth_or_array_layers: 1 },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Faceted gray chunk — Stone icon.
fn stone_icon_image(size_px: u32) -> Image {
    let s = size_px as usize;
    let mut px = vec![0u8; s * s * 4];
    let cx = s as f32 * 0.5;
    let cy = s as f32 * 0.55;
    let r = s as f32 * 0.36;
    let n = 7usize;
    let verts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU - 0.35;
            let rr = r * (0.78 + 0.27 * ((i * 53) % 7) as f32 / 6.0);
            (cx + a.cos() * rr, cy + a.sin() * rr)
        })
        .collect();
    for y in 0..s {
        for x in 0..s {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut inside = false;
            let mut j = n - 1;
            for i in 0..n {
                let (xi, yi) = verts[i];
                let (xj, yj) = verts[j];
                if (yi > fy) != (yj > fy) && fx < (xj - xi) * (fy - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            if inside {
                let k = (y * s + x) * 4;
                let dark = fy > cy;
                let (rr, gg, bb) = if dark { (100, 103, 110) } else { (138, 142, 150) };
                px[k] = rr; px[k + 1] = gg; px[k + 2] = bb; px[k + 3] = 255;
            }
        }
    }
    Image::new(
        Extent3d { width: size_px, height: size_px, depth_or_array_layers: 1 },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

// ============================================================================
// Solid biome-colored floor
// ============================================================================

/// Flat, opaque, biome-colored pointy-top hexagon for every land terrain,
/// generated once from `TerrainType::minimap_color`. Fully opaque (1px
/// antialiased edge only) so adjacent tiles interlock with no translucent
/// gaps — the floor reads as a solid green/blue/yellow sheet
/// (forest = green, desert = yellow, sea = blue, …).
#[derive(Resource, Default)]
pub struct SolidFloorTextures {
    pub by_terrain: HashMap<TerrainType, Handle<Image>>,
}

/// Pointy-top hexagon tile (hard interior, 1px antialiased edge) colored
/// with `srgb`. The texture must match the hexagon aspect (√3·R × 2·R) so the
/// shape fills the sprite exactly; adjacent tiles then interlock with no
/// background showing through.
fn hex_tile_image(width: u32, height: u32, srgb: [f32; 3]) -> Image {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let [r, g, b] = srgb.map(|c| (c * 255.0).round() as u8);
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = height as f32 / 2.0;
    // Apothem (center → edge-midpoint distance) of the pointy-top hexagon;
    // edges face 0°, ±60°, so inside ⇔ |nᵢ·p| ≤ apothem for all three.
    let apothem = radius * 1.7320508075688772 / 2.0;
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5 - cx;
            let py = y as f32 + 0.5 - cy;
            let m = px
                .abs()
                .max((0.5 * px + 0.8660254037844386 * py).abs())
                .max((0.5 * px - 0.8660254037844386 * py).abs());
            // Signed distance in px; +0.5 centers coverage on the edge.
            let alpha = (apothem - m + 0.5).clamp(0.0, 1.0);
            let i = ((y * width + x) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = (alpha * 255.0).round() as u8;
        }
    }
    Image::new(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Generate the solid floor textures once (before the floor pass uses them).
pub fn init_solid_floor_textures(
    mut images: ResMut<Assets<Image>>,
    mut solid: ResMut<SolidFloorTextures>,
) {
    if !solid.by_terrain.is_empty() {
        return;
    }
    // Half-resolution hexagon: flat color, so GPU upscaling is invisible.
    let w = (FLOOR_W * 12.0).round().max(4.0) as u32;
    let h = (FLOOR_H * 12.0).round().max(4.0) as u32;
    for terrain in [
        TerrainType::Grass,
        TerrainType::Forest,
        TerrainType::Desert,
        TerrainType::Polluted,
        TerrainType::Tundra,
        TerrainType::Taiga,
        TerrainType::Grassland,
        TerrainType::TropicalRainforest,
        TerrainType::Mountain,
        TerrainType::City,
    ] {
        let [r, g, b] = terrain.minimap_color();
        let handle = images.add(hex_tile_image(w, h, [r, g, b]));
        solid.by_terrain.insert(terrain, handle);
    }
}

// ============================================================================
// Server-authoritative resource nodes (world_object replication)
// ============================================================================

/// Visual cache for replicated `world_object` rows: object_id → entity plus
/// the descriptor last rendered, so unchanged rows skip rework. Also caches
/// loaded texture aspects so sprites keep their native proportions.
#[derive(Resource, Default)]
pub struct WorldObjectState {
    pub visuals: HashMap<u64, Entity>,
    pub rendered: HashMap<u64, (String, bool, i32, i32)>,
    pub aspects: HashMap<String, f32>,
}

/// Pixel aspect (width/height) of the object art files — these are static
/// files, and runtime lookups through `Assets<Image>` are unreliable for
/// render-only textures, so the values live here.
fn known_aspect(path: &str) -> f32 {
    match path {
        p if p.ends_with("treePine.png") => 256.0 / 216.0,
        p if p.ends_with("grassStoneLarge_S.png") => 256.0 / 512.0,
        p if GRASS_TUFT_PATHS.contains(&p) => 220.0 / 379.0,
        _ => 1.0,
    }
}

/// Target on-screen height (world units) per object kind.
fn kind_height(kind: &str, mature: bool) -> f32 {
    match kind {
        "Grass" => 5.2,
        "Rock" => 4.5,
        "Tree" if mature => 7.5,
        "Tree" => 4.2,
        _ => 3.0,
    }
}

/// Spawn/update/despawn sprites for grass, rocks and player-grown trees.
/// The `world_object` table is the only source of truth — nothing decorative
/// is invented client-side anymore.
pub fn update_world_object_visuals(
    mut commands: Commands,
    net: Res<crate::net::plugin::Net>,
    player_transform: Res<PlayerTransform>,
    props: Res<PropTextures>,
    time: Res<Time>,
    mut next_scan: Local<f64>,
    mut state: ResMut<WorldObjectState>,
) {
    // Perf: this scans every replicated world_object row; at planet scale
    // that set grows unboundedly, so cap the pass to ~7 Hz.
    if time.elapsed_secs_f64() < *next_scan {
        return;
    }
    *next_scan = time.elapsed_secs_f64() + 0.15;
    let Some(conn) = net.conn.as_ref() else { return };
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let max_dist = RENDER_RADIUS_HEXES + 2.0 * WorldGenConfig::HEX_SIZE;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for row in crate::net::gen::WorldObjectTableAccess::world_object(&conn.db).iter() {
        seen.insert(row.object_id);
        let coord = idlecore_core::hex::HexCoord::from_id(row.hex_id);
        let (hx, hy) = row_world_center(coord.q, coord.r);
        let wx = hx + row.offset_x;
        let wy = hy + row.offset_y;
        if (wx - px).powi(2) + (wy - py).powi(2) > max_dist * max_dist {
            continue;
        }

        let mature = row.mature_at == 0 || now >= row.mature_at;
        // Descriptor: sprite + target height + flip; trees grow sapling->full.
        let flip = row.object_id % 2 == 0;
        let (handle, height, aspect, tint) = match row.kind.as_str() {
            "Grass" => (&props.grass, kind_height("Grass", true), props.grass_aspect, Color::WHITE),
            "Rock" => (&props.rock, kind_height("Rock", true), props.rock_aspect, Color::WHITE),
            "Tree" => {
                if mature {
                    (&props.tree, kind_height("Tree", true), props.tree_aspect, Color::WHITE)
                } else {
                    (
                        &props.sapling,
                        kind_height("Tree", false),
                        props.sapling_aspect,
                        Color::srgb(0.65, 0.85, 0.5),
                    )
                }
            }
            other => {
                warn!("unknown world object kind '{other}'");
                continue;
            }
        };
        let size = Vec2::new(height * aspect, height);

        let key = (
            row.kind.clone(),
            mature,
            (wx * 16.0) as i32,
            (wy * 16.0) as i32,
        );
        if state.rendered.get(&row.object_id) == Some(&key) {
            continue;
        }

        if let Some(entity) = state.visuals.get(&row.object_id).copied() {
            commands.entity(entity).despawn();
        }
        let sprite = commands.spawn((
            Name::new(format!("obj-{}-{}", row.kind, row.object_id)),
            Sprite {
                image: handle.clone(),
                custom_size: Some(size),
                color: tint,
                flip_x: flip,
                ..default()
            },
            // Bottom-center anchor: the sprite's base sits on the hex.
            Transform::from_xyz(wx, wy + size.y * 0.5, prop_depth(wy) + 0.55),
            Visibility::Visible,
        ));
        state.visuals.insert(row.object_id, sprite.id());
        state.rendered.insert(row.object_id, key);
    }

    let stale: Vec<u64> = state
        .visuals
        .keys()
        .filter(|id| !seen.contains(id))
        .copied()
        .collect();
    for object_id in stale {
        if let Some(entity) = state.visuals.remove(&object_id) {
            commands.entity(entity).despawn();
        }
        state.rendered.remove(&object_id);
    }
}

// ============================================================================
// Generated water textures
// ============================================================================

/// Flat-blue hexagon textures for water hexes, shaded by water class. Built
/// once from raw pixels so they stay pixel-perfect at any zoom (no art pack
/// contains water tiles).
#[derive(Resource, Default)]
pub struct WaterTextures {
    pub by_class: HashMap<WaterClass, Handle<Image>>,
}

/// Create a solid-color hexagon tile image sized to the tile slot, with a 1px
/// anti-aliased edge so adjacent water hexes meet without gaps.

/// Water color per class: deep blue for ocean, lighter for inland water.
fn water_color(class: WaterClass) -> [f32; 3] {
    match class {
        WaterClass::Ocean => [0.09, 0.22, 0.42],
        WaterClass::Sea => [0.14, 0.32, 0.55],
        WaterClass::Lake => [0.18, 0.42, 0.63],
        WaterClass::River => [0.16, 0.38, 0.61],
        WaterClass::Wetland => [0.2, 0.44, 0.58],
        WaterClass::Coast => [0.22, 0.5, 0.75],
        WaterClass::None => [0.15, 0.36, 0.58],
    }
}

/// Generate the water textures once (before the floor pass uses them).
pub fn init_water_textures(
    mut images: ResMut<Assets<Image>>,
    mut water: ResMut<WaterTextures>,
) {
    if !water.by_class.is_empty() {
        return;
    }
    // Half-resolution hexagon: flat color, so GPU upscaling is invisible.
    let w = (FLOOR_W * 12.0).round().max(4.0) as u32;
    let h = (FLOOR_H * 12.0).round().max(4.0) as u32;
    for class in [
        WaterClass::Ocean,
        WaterClass::Sea,
        WaterClass::Coast,
        WaterClass::Lake,
        WaterClass::River,
        WaterClass::Wetland,
    ] {
        let handle = images.add(hex_tile_image(w, h, water_color(class)));
        water.by_class.insert(class, handle);
    }
}

/// Marker for the parent entity of a rendered chunk.
#[derive(Component)]
pub struct WorldChunk;

/// Tracks spawned chunk entities so we only (re)create on changes.
#[derive(Resource, Default)]
pub struct WorldFloor {
    pub entities: std::collections::HashMap<(i32, i32), Entity>,
    /// Player hex of the last rebuilt render set; unchanged → skip the pass.
    pub last_player_hex: Option<(i32, i32)>,
}

/// Chunk radius around the player that is rendered.
const RENDER_RADIUS_CHUNKS: i32 = 5;

/// World-space radius around the player to show.
const RENDER_RADIUS_HEXES: f32 = 20.0 * WorldGenConfig::HEX_SIZE;

/// Spawn/despawn chunk entities around the player position.
pub fn update_world_floor(
    mut commands: Commands,
    streaming_world: Res<StreamingWorldResource>,
    player_transform: Res<PlayerTransform>,
    mut floor: ResMut<WorldFloor>,
    water: Res<WaterTextures>,
    solid: Res<SolidFloorTextures>,
) {
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;

    let (hq, hr) = world_pos_to_hex(px, py, WorldGenConfig::HEX_SIZE);

    // Perf: rebuild the render set when the player's hex changes (the wanted
    // set only shifts as the player crosses tile boundaries). Chunk-granular
    // gating missed updates because a chunk (32 hexes) is larger than the
    // render radius (20 hexes).
    if !floor.entities.is_empty() && Some((hq, hr)) == floor.last_player_hex {
        return;
    }
    floor.last_player_hex = Some((hq, hr));

    let (ccq, ccr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);

    // Determine the set of chunks we want rendered.
    let mut wanted: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for dcq in -RENDER_RADIUS_CHUNKS..=RENDER_RADIUS_CHUNKS {
        for dcr in -RENDER_RADIUS_CHUNKS..=RENDER_RADIUS_CHUNKS {
            let cq = ccq + dcq;
            let cr = ccr + dcr;
            let Some(chunk) = streaming_world.chunks.chunks.get(&(cq, cr)) else { continue };
            let mut close_enough = false;
            for cell in &chunk.cells {
                let (wx, wy) = cell.world_pos(WorldGenConfig::HEX_SIZE);
                let dx = wx - px;
                let dy = wy - py;
                if dx * dx + dy * dy <= RENDER_RADIUS_HEXES * RENDER_RADIUS_HEXES {
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

        let mut parent = commands.spawn((
            Name::new(format!("WorldChunk({cq},{cr})")),
            WorldChunk,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
        ));

        for cell in &chunk.cells {
            let (wx, wy) = cell.world_pos(WorldGenConfig::HEX_SIZE);
            let handle = if cell.terrain == TerrainType::Water {
                water
                    .by_class
                    .get(&cell.water)
                    .or_else(|| water.by_class.values().next())
                    .cloned()
            } else {
                solid.by_terrain.get(&cell.terrain).cloned()
            };
            let Some(handle) = handle else { continue };
            parent.with_child((
                Name::new(format!("tile({},{})", cell.q, cell.r)),
                Sprite {
                    image: handle,
                    custom_size: Some(Vec2::new(FLOOR_W, FLOOR_H)),
                    ..default()
                },
                Transform::from_xyz(wx, wy, floor_depth(wy)),
            ));
        }

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

/// Sprite descriptor for a hex visual: color, size and whether the plant is
/// tall (Tree/Corn/RareHerb) or low (Wheat/Sunflower).
fn plant_sprite(kind_name: &str, mature: bool) -> Sprite {
    let color = plant_type_color(kind_name, mature).unwrap_or(Color::srgb(0.4, 0.85, 0.3));
    let tall = matches!(kind_name, "Tree" | "Corn" | "RareHerb");
    Sprite {
        color,
        custom_size: Some(Vec2::splat(if tall { 2.8 } else { 2.0 })),
        ..default()
    }
}

/// Spawn/update/despawn hex visuals from the authoritative `hex_tile` cache.
pub fn update_plant_visuals(
    mut commands: Commands,
    net: Res<crate::net::plugin::Net>,
    player_transform: Res<crate::player::PlayerTransform>,
    _streaming_world: Res<StreamingWorldResource>,
    time: Res<Time>,
    mut next_scan: Local<f64>,
    mut state: ResMut<FloorPlantState>,
) {
    // Perf: full hex_tile scan — throttle to ~7 Hz like world objects.
    if time.elapsed_secs_f64() < *next_scan {
        return;
    }
    *next_scan = time.elapsed_secs_f64() + 0.15;
    let Some(conn) = net.conn.as_ref() else { return };

    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let (hq, hr) = world_pos_to_hex(px, py, WorldGenConfig::HEX_SIZE);
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

        // Determine desired visual: pollution disc, plant diamond, or nothing.
        let mut kind: Option<Sprite> = None;
        if is_polluted {
            kind = Some(Sprite {
                color: Color::srgb(0.18, 0.2, 0.16),
                custom_size: Some(Vec2::splat(2.4)),
                ..default()
            });
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
                    kind = Some(plant_sprite(&p.kind_name, now >= p.mature_at));
                }
                None => {
                    state.raw.remove(&row.hex_id);
                    state.parsed.remove(&row.hex_id);
                }
            }
        } else if let Some(p) = state.parsed.get(&row.hex_id) {
            mature = now >= p.mature_at;
            kind = Some(plant_sprite(&p.kind_name, mature));
        }

        let cached = state.stage.get(&row.hex_id).cloned();
        let band = eco_band(row.eco_rating);
        if cached == Some((is_polluted, mature, band)) {
            continue;
        }

        let (wx, wy) = row_world_center(row.hex_q, row.hex_r);
        let existing = state.visuals.get(&row.hex_id).copied();
        match kind {
            Some(mut sprite) => {
                if let Some(entity) = existing {
                    commands.entity(entity).despawn();
                }
                // Spec 020 T6.4: eco-rating tint disc (lush / degraded bands).
                if band != 0 {
                    sprite.color = match band {
                        1 => Color::srgba(0.2, 0.9, 0.35, 0.4),
                        _ => Color::srgba(0.55, 0.35, 0.15, 0.35),
                    };
                    sprite.custom_size = Some(Vec2::splat(2.2));
                }
                let root = commands.spawn((
                    Name::new(format!("hex-visual-{}", row.hex_id)),
                    HexPlantVisual,
                    Sprite {
                        color: sprite.color,
                        custom_size: sprite.custom_size,
                        ..default()
                    },
                    Transform::from_xyz(wx, wy, prop_depth(wy) + 1.0)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                    Visibility::Visible,
                ));
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
mod tests_plants {
    use super::*;
    use crate::plugins::player::aura_config;

    const PLANT_TYPES: [&str; 5] = ["Wheat", "Corn", "Sunflower", "Tree", "RareHerb"];

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
        assert_eq!(e.1, 0.25);
        let w = aura_config(500).unwrap();
        assert_eq!(w.1, 0.3);
        let l = aura_config(1000).unwrap();
        assert_eq!(l.1, 0.35);
        assert_eq!(aura_config(9999).unwrap().1, 0.35);
    }

    #[test]
    fn water_colors_are_distinct() {
        let classes = [
            WaterClass::Ocean,
            WaterClass::Sea,
            WaterClass::Coast,
            WaterClass::Lake,
            WaterClass::River,
            WaterClass::Wetland,
            WaterClass::None,
        ];
        let colors: Vec<[f32; 3]> = classes.into_iter().map(water_color).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "water classes must be distinct");
            }
            let c = colors[i];
            assert!(c[2] > c[0] && c[2] > c[1], "water must be blue-ish: {c:?}");
        }
    }

    #[test]
    fn hex_outline_alpha_is_a_hollow_ring() {
        let img = hex_outline_image(18, 20, [1.0, 1.0, 0.0], 2.0);
        let data = img.data.expect("has pixels");
        let w = 18u32;
        let alpha = |x: u32, y: u32| data[((y * w + x) * 4 + 3) as usize];
        // Hollow interior.
        assert_eq!(alpha(9, 10), 0);
        // Opaque band at the edge midpoint (left vertex region).
        assert!(alpha(0, 10) > 200 || alpha(1, 10) > 200);
        // Outside corners stay empty.
        assert_eq!(alpha(0, 0), 0);
    }

    #[test]
    fn hex_tile_alpha_is_a_pointy_top_hexagon() {
        let img = hex_tile_image(18, 20, [1.0, 0.0, 0.0]);
        let data = img.data.expect("has pixels");
        let w = 18u32;
        let alpha = |x: u32, y: u32| data[((y * w + x) * 4 + 3) as usize];
        // Center is opaque, texture corners are transparent.
        assert!(alpha(9, 10) > 240);
        for (x, y) in [(0, 0), (17, 0), (0, 19), (17, 19)] {
            assert_eq!(alpha(x, y), 0, "corner ({x},{y})");
        }
        // Left/right edge midpoints are the side vertices: opaque just inside,
        // faded at the very tip (AA).
        assert!(alpha(2, 10) > 240);
        assert!(alpha(0, 10) < alpha(2, 10));
        assert!(alpha(15, 10) > 240);
        // Top/bottom vertices reach the texture edges (pointy-top); the very
        // tip pixel is AA-faded but not empty.
        assert!(alpha(9, 1) > 240);
        assert!(alpha(9, 0) > 0 && alpha(9, 0) < 255);
    }
}