//! Renders the streamed hex world as a Stardew-style square tile floor.
//!
//! One parent entity per loaded chunk. The ground is a grid of square slots
//! (`idlecore_core::slots`, one 16px Tiny* art tile each): every slot is
//! drawn as a square sprite tinted by the terrain of the hex that owns it
//! (flat blue squares for water). Hexes remain the gameplay grid — walkability,
//! actions and replication — but the visible floor is seamless squares.
//! Chunks spawn lazily as they stream in and despawn out of range.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use spacetimedb_sdk::Table;
use std::collections::HashMap;
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::slots::{slot_center, slot_hex, world_pos_to_slot, SLOT_SIZE};
use idlecore_core::terrain::TerrainType;
use idlecore_core::world_detail::{city_cell, floor_detail, CityCellKind};
use idlecore_core::world_gen::{WaterClass, WorldGenConfig, hex_to_chunk_coord};
use crate::player::PlayerTransform;
use crate::plugins::world::StreamingWorldResource;

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
// ============================================================================
// Action-target box (Stardew-style ground cursor)
// ============================================================================

/// Ground slot currently targeted for actions (plant/harvest/gather/clean),
/// driven by the mouse cursor and snapped to the 16px slot grid (see
/// `idlecore_core::slots`); defaults to the player's own slot. `q`/`r` are
/// the axial coords of the hex that owns the slot.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActionTarget {
    pub q: i32,
    pub r: i32,
    pub slot_x: i32,
    pub slot_y: i32,
}

#[derive(Component)]
pub(crate) struct ActionBoxMarker;

/// Square slot-outline cursor (Stardew-style), AA edges like the fill tiles.
fn slot_outline_image(width: u32, height: u32, srgb: [f32; 3], thickness_px: f32) -> Image {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let [r, g, b] = srgb.map(|c| (c * 255.0).round() as u8);
    for y in 0..height {
        for x in 0..width {
            // Distance from this pixel to the nearest square edge (px).
            let dx = (x as f32 + 0.5).min(width as f32 - x as f32 - 0.5);
            let dy = (y as f32 + 0.5).min(height as f32 - y as f32 - 0.5);
            let edge = dx.min(dy);
            // Hollow ring: opaque band of `thickness_px` fading over ~1 px.
            let alpha = (thickness_px - edge + 0.5).clamp(0.0, 1.0);
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
    let w = (SLOT_SIZE * 12.0).round() as u32;
    let handle = images.add(slot_outline_image(w, w, [1.0, 0.95, 0.5], 2.0));
    commands.spawn((
        Name::new("action-box"),
        ActionBoxMarker,
        Sprite {
            image: handle,
            // Smaller than the slot itself so it reads as a cursor, not a wall.
            custom_size: Some(Vec2::splat(SLOT_SIZE * 0.62)),
            ..default()
        },
        Transform::from_xyz(f32::MAX, f32::MAX, 0.0),
        Visibility::Hidden,
    ));
}

/// Drive the targeting box from the cursor and refresh the target resource.
/// The box snaps to the ground slot under the cursor (player's slot by
/// default), Stardew-style; actions apply to the slot's owning hex.
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

    // Default target: the slot the player stands on.
    let mut next = world_pos_to_slot(
        player_transform.translation.x,
        player_transform.translation.y,
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
                        next = world_pos_to_slot(world.x, world.y);
                    }
                }
            }
        }
    }
    let (sx, sy) = next;
    let (q, r) = slot_hex(sx, sy);
    target.slot_x = sx;
    target.slot_y = sy;
    target.q = q;
    target.r = r;

    let (cx, cy) = slot_center(sx, sy);
    transform.translation = Vec3::new(cx, cy, crate::world_floor::prop_depth(cy) + 0.4);
    *visibility = Visibility::Visible;

    // Green while the slot's hex is within the server's 1-hex interaction
    // range, dim otherwise.
    let player_hex = world_pos_to_hex(
        player_transform.translation.x,
        player_transform.translation.y,
        WorldGenConfig::HEX_SIZE,
    );
    let in_range = idlecore_core::hex_grid::HexGrid::distance(q, r, player_hex.0, player_hex.1) <= 1;
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
    /// Fallen log (Spec 022): Tiny Farm dead-wood tile.
    pub log: Handle<Image>,
    pub log_aspect: f32,
    /// Craft bench (Spec 022): Tiny Dungeon crate, taller than plants.
    pub bench: Handle<Image>,
    pub bench_aspect: f32,
    /// Square UI icons for inventory slots.
    pub icon_seed: Handle<Image>,
    pub icon_wood: Handle<Image>,
    pub icon_stone: Handle<Image>,
    pub icon_grass: Handle<Image>,
    pub icon_log: Handle<Image>,
    pub icon_pickaxe: Handle<Image>,
    pub icon_axe: Handle<Image>,
    pub icon_shovel: Handle<Image>,
    pub icon_hoe: Handle<Image>,
    /// Car (and other vehicles) inventory icon — Tiny Battle car tile.
    pub icon_car: Handle<Image>,
}

/// Build all prop sprites once. Runs at Startup. All props are 16x16 tiles
/// from the Tiny* packs; their baked backdrops are keyed by
/// `tiny::process_key_queue`.
pub fn init_prop_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut queue: ResMut<crate::tiny::TinyKeyQueue>,
) {
    fn prop(asset_server: &AssetServer, queue: &mut crate::tiny::TinyKeyQueue, path: &'static str) -> Handle<Image> {
        let h = asset_server.load::<Image>(path);
        queue.0.push(h.clone());
        h
    }
    commands.insert_resource(PropTextures {
        // Grass tufts: Tiny Farm round grass plant (tile_0056); tree: Tiny
        // Town round tree; sapling: Tiny Farm sapling; rock: Tiny Ski
        // boulder; wood: crate; seed: bag; log: Farm dead wood (Spec 022);
        // bench: Dungeon crate (Spec 022); tools: Town/Farm hand tools.
        // All chroma-keyed via TinyKeyQueue.
        grass: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0056.png"),
        grass_aspect: 1.0,
        tree: prop(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0004.png"),
        tree_aspect: 1.0,
        sapling: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0004.png"),
        sapling_aspect: 1.0,
        rock: prop(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0081.png"),
        rock_aspect: 1.0,
        log: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0002.png"),
        log_aspect: 1.0,
        bench: prop(&asset_server, &mut queue, "models/Tiny Dungeon/Tiles/tile_0075.png"),
        bench_aspect: 1.0,
        icon_seed: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0009.png"),
        icon_wood: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0076.png"),
        icon_stone: prop(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0081.png"),
        icon_grass: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0056.png"),
        icon_log: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0002.png"),
        icon_pickaxe: prop(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0115.png"),
        icon_axe: prop(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0127.png"),
        icon_shovel: prop(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0086.png"),
        icon_hoe: prop(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0129.png"),
        icon_car: prop(&asset_server, &mut queue, "models/Tiny Battle/Tiles/tile_0114.png"),
    });
}

// ============================================================================
// Ambient decorations — plants & critters from across the Tiny* packs
// ============================================================================

/// One decoration entry: sprite handle, on-screen height (world units).
pub struct Deco {
    pub image: Handle<Image>,
    pub height: f32,
}

/// Ambient decoration set per terrain: `plants` are common garnish,
/// `critters` are rare animals/props that make the world feel alive.
/// Purely visual — deterministic per slot, no server data involved.
#[derive(Resource, Default)]
pub struct DecoTextures {
    pub by_terrain: HashMap<TerrainType, DecoSet>,
}

#[derive(Default)]
pub struct DecoSet {
    pub plants: Vec<Deco>,
    pub critters: Vec<Deco>,
}

/// Load every decoration tile once (chroma-keyed via the TinyKeyQueue).
pub fn init_deco_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut queue: ResMut<crate::tiny::TinyKeyQueue>,
) {
    fn deco(
        asset_server: &AssetServer,
        queue: &mut crate::tiny::TinyKeyQueue,
        path: &'static str,
        height: f32,
    ) -> Deco {
        let h = asset_server.load::<Image>(path);
        queue.0.push(h.clone());
        Deco { image: h, height }
    }
    let mut sets: HashMap<TerrainType, DecoSet> = HashMap::new();
    let mut add = |terrain: TerrainType, critter: bool, d: Deco| {
        let set = sets.entry(terrain).or_default();
        if critter { set.critters.push(d) } else { set.plants.push(d) }
    };
    // Meadow: sprouts, wheat, berries — plus the odd chicken.
    for path in [
        "models/Tiny Town/Tiles/tile_0008.png",
        "models/Tiny Town/Tiles/tile_0009.png",
        "models/Tiny Town/Tiles/tile_0017.png",
        "models/Tiny Farm/Tiles/tile_0005.png",
        "models/Tiny Farm/Tiles/tile_0082.png",
    ] {
        add(TerrainType::Grass, false, deco(&asset_server, &mut queue, path, 1.0));
    }
    add(TerrainType::Grass, true, deco(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0122.png", 1.1));
    add(TerrainType::Grass, false, deco(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0029.png", 0.8));
    // Plains: wheat, with cows and sheep grazing.
    for path in [
        "models/Tiny Town/Tiles/tile_0009.png",
        "models/Tiny Farm/Tiles/tile_0066.png",
    ] {
        add(TerrainType::Grassland, false, deco(&asset_server, &mut queue, path, 1.1));
    }
    add(TerrainType::Grassland, true, deco(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0121.png", 1.7));
    add(TerrainType::Grassland, true, deco(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0120.png", 1.5));
    // Woods: mushrooms and ferns under the trees.
    for (path, h) in [
        ("models/Tiny Town/Tiles/tile_0029.png", 0.9),
        ("models/Tiny Town/Tiles/tile_0030.png", 0.8),
        ("models/Tiny Town/Tiles/tile_0017.png", 0.9),
    ] {
        add(TerrainType::Forest, false, deco(&asset_server, &mut queue, path, h));
    }
    // Jungle: gourds and golden cane.
    add(TerrainType::TropicalRainforest, false, deco(&asset_server, &mut queue, "models/Tiny Farm/Tiles/tile_0078.png", 1.1));
    add(TerrainType::TropicalRainforest, false, deco(&asset_server, &mut queue, "models/Tiny Town/Tiles/tile_0021.png", 1.3));
    // Desert: scattered stones.
    add(TerrainType::Desert, false, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0081.png", 0.9));
    // Tundra: ice blocks and the odd abandoned sled.
    add(TerrainType::Tundra, false, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0078.png", 1.0));
    add(TerrainType::Tundra, true, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0068.png", 1.0));
    // Taiga: dead trees, stones — and wolves.
    add(TerrainType::Taiga, false, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0007.png", 1.8));
    add(TerrainType::Taiga, false, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0081.png", 0.9));
    add(TerrainType::Taiga, true, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0072.png", 1.4));
    add(TerrainType::Taiga, true, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0076.png", 1.4));
    // Highlands: stones.
    add(TerrainType::Mountain, false, deco(&asset_server, &mut queue, "models/Tiny Ski/Tiles/tile_0081.png", 1.0));

    commands.insert_resource(DecoTextures { by_terrain: sets });
}

/// Deterministic per-slot hash with a salt (independent streams for
/// variant pick vs deco decision vs jitter).
fn slot_hash(sx: i32, sy: i32, salt: u32) -> u32 {
    let mut x = (sx as u32).wrapping_mul(0x9E37_79B9)
        ^ (sy as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 13;
    x = x.wrapping_mul(0x27D4_EB2F);
    x ^= x >> 16;
    x
}

// ============================================================================
// Solid square floor tiles
// ============================================================================

/// Square floor tiles per land terrain: each slot renders one 16px Tiny*
/// tile directly (no baking), tinted per variant. The per-slot pick is
/// hashed from the slot coordinates so the ground doesn't repeat.
#[derive(Resource, Default)]
pub struct SolidFloorTextures {
    pub by_terrain: HashMap<TerrainType, Vec<(Handle<Image>, [f32; 3])>>,
}

/// Load the floor tile handles (raw 16x16 art; streams in async).
pub fn init_solid_floor_textures(
    images: ResMut<Assets<Image>>,
    mut solid: ResMut<SolidFloorTextures>,
    asset_server: Res<AssetServer>,
    mut handles: Local<Option<Vec<(TerrainType, Handle<Image>, [f32; 3])>>>,
    mut solid_done: Local<bool>,
) {
    if !solid.by_terrain.is_empty() {
        return;
    }
    if *solid_done {
        return;
    }
    let handles = handles.get_or_insert_with(|| {
        let mut all: Vec<(TerrainType, Handle<Image>, [f32; 3])> = Vec::new();
        for terrain in terrains_iter() {
            for (path, tint) in floor_tiles_for(terrain) {
                let handle = asset_server.load::<Image>(*path);
                all.push((terrain, handle, *tint));
            }
        }
        all
    });
    if handles.iter().any(|(_, handle, _)| images.get(handle).is_none()) {
        return; // not streamed yet — retry next frame
    }
    for (terrain, handle, tint) in handles.iter() {
        solid
            .by_terrain
            .entry(*terrain)
            .or_default()
            .push((handle.clone(), *tint));
    }
    *solid_done = true;
}

/// Terrain order matching the handle list built above.
fn terrains_iter() -> impl Iterator<Item = TerrainType> {
    [
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
    ]
    .into_iter()
}

/// Seamless Tiny* tile variants per terrain (with per-variant tints), mixing
/// the whole Tiny* family — they share one visual identity. Only border-free
/// tiles qualify — bordered patch tiles would band when tiled.
fn floor_tiles_for(terrain: TerrainType) -> &'static [(&'static str, [f32; 3])] {
    match terrain {
        // Meadow grass: Tiny Town — plain green only (tile 2 has flowers
        // baked in; flowers belong to the deco layer, not the floor).
        TerrainType::Grass => &[
            ("models/Tiny Town/Tiles/tile_0001.png", [1.0, 1.0, 1.0]),
            ("models/Tiny Town/Tiles/tile_0000.png", [1.0, 1.0, 1.0]),
        ],
        // Dry plain: warm-tinted Town grass.
        TerrainType::Grassland => &[
            ("models/Tiny Town/Tiles/tile_0000.png", [1.05, 1.05, 0.95]),
            ("models/Tiny Town/Tiles/tile_0001.png", [1.05, 1.05, 0.95]),
        ],
        // Deep woods: the same grass, cooler and darker — growth biomes
        // stay pure grass (soil-looking tiles are reserved for future farm
        // plots).
        TerrainType::Forest => &[
            ("models/Tiny Town/Tiles/tile_0001.png", [0.66, 0.82, 0.62]),
            ("models/Tiny Town/Tiles/tile_0000.png", [0.66, 0.82, 0.62]),
        ],
        // Jungle: lush saturated grass.
        TerrainType::TropicalRainforest => &[
            ("models/Tiny Town/Tiles/tile_0001.png", [0.88, 1.06, 0.85]),
            ("models/Tiny Town/Tiles/tile_0000.png", [0.95, 1.1, 0.9]),
        ],
        // Dunes: Tiny Town seamless dirt (25 = border-free sand earth).
        TerrainType::Desert => &[
            ("models/Tiny Town/Tiles/tile_0025.png", [1.0, 1.0, 1.0]),
            ("models/Tiny Town/Tiles/tile_0025.png", [1.07, 0.97, 0.86]),
            ("models/Tiny Town/Tiles/tile_0025.png", [0.95, 0.9, 1.02]),
        ],
        // Snow: Tiny Ski.
        TerrainType::Tundra => &[
            ("models/Tiny Ski/Tiles/tile_0000.png", [1.0, 1.0, 1.0]),
            ("models/Tiny Ski/Tiles/tile_0002.png", [1.0, 1.0, 1.0]),
        ],
        // Boreal snow: colder blue tint.
        TerrainType::Taiga => &[
            ("models/Tiny Ski/Tiles/tile_0000.png", [0.82, 0.9, 1.08]),
            ("models/Tiny Ski/Tiles/tile_0002.png", [0.82, 0.9, 1.08]),
        ],
        // Rocky highlands: Dungeon scree, strongly greyed so the warm
        // brown base reads as bare rock.
        TerrainType::Mountain => &[
            ("models/Tiny Dungeon/Tiles/tile_0012.png", [0.66, 0.66, 0.76]),
            ("models/Tiny Dungeon/Tiles/tile_0013.png", [0.6, 0.6, 0.72]),
            ("models/Tiny Dungeon/Tiles/tile_0012.png", [0.78, 0.78, 0.88]),
        ],
        // Asphalt + road markings: Tiny Battle streets.
        TerrainType::City => &[
            ("models/Tiny Battle/Tiles/tile_0108.png", [1.0, 1.0, 1.0]),
            ("models/Tiny Battle/Tiles/tile_0109.png", [1.0, 1.0, 1.0]),
            ("models/Tiny Battle/Tiles/tile_0110.png", [1.0, 1.0, 1.0]),
        ],
        // Blighted ground: scree with a sickly green tint.
        TerrainType::Polluted => &[
            ("models/Tiny Dungeon/Tiles/tile_0012.png", [0.85, 1.0, 0.85]),
            ("models/Tiny Dungeon/Tiles/tile_0013.png", [0.75, 0.9, 0.75]),
        ],
        TerrainType::Water => &[
            ("models/Tiny Town/Tiles/tile_0000.png", [1.0, 1.0, 1.0]), // unused
        ],
    }
}

/// Deterministic per-slot variant pick: same slot, same variant, always.
fn cell_variant(q: i32, r: i32, n: usize) -> usize {
    let mut x = (q as u32).wrapping_mul(0x9E37_79B9) ^ (r as u32).wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    (x % n as u32) as usize
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
}

/// Target on-screen height (world units) per object kind.
fn kind_height(kind: &str, mature: bool) -> f32 {
    match kind {
        "Grass" if mature => 1.8,
        // Immature grass renders as a small sprout (Spec 022 §1).
        "Grass" => 0.9,
        "Rock" => 2.2,
        "Tree" if mature => 4.2,
        "Tree" => 2.4,
        // A fallen log sits low; the bench crate stands taller than plants.
        "Log" => 1.3,
        "CraftBench" => 2.6,
        _ => 2.0,
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
    // Rows needing a (re)spawn this pass, sorted near-first and capped so a
    // burst of replication can't create hundreds of sprites in one frame.
    let mut pending: Vec<(f32, u64)> = Vec::new();
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
        let key = (
            row.kind.clone(),
            mature,
            (wx * 16.0) as i32,
            (wy * 16.0) as i32,
        );
        if state.rendered.get(&row.object_id) == Some(&key) {
            continue;
        }
        let dx = wx - px;
        let dy = wy - py;
        pending.push((dx * dx + dy * dy, row.object_id));
    }

    if pending.len() > SPRITE_SPAWNS_PER_PASS {
        pending.sort_by(|a, b| a.0.total_cmp(&b.0));
        pending.truncate(SPRITE_SPAWNS_PER_PASS);
    }

    for (_, object_id) in pending {
        // Re-fetch the row so descriptors are computed only for spawned
        // sprites, not for every near row on every pass.
        let Some(row) = crate::net::gen::WorldObjectTableAccess::world_object(&conn.db)
            .object_id()
            .find(&object_id)
        else {
            continue;
        };
        let coord = idlecore_core::hex::HexCoord::from_id(row.hex_id);
        let (hx, hy) = row_world_center(coord.q, coord.r);
        let wx = hx + row.offset_x;
        let wy = hy + row.offset_y;
        let mature = row.mature_at == 0 || now >= row.mature_at;
        // Descriptor: sprite + target height + flip; trees grow sapling->full.
        let flip = row.object_id % 2 == 0;
        let (handle, height, aspect, tint) = match row.kind.as_str() {
            "Grass" => {
                if mature {
                    (&props.grass, kind_height("Grass", true), props.grass_aspect, Color::WHITE)
                } else {
                    // Planted grass sprouts via the existing sapling path.
                    (
                        &props.sapling,
                        kind_height("Grass", false),
                        props.sapling_aspect,
                        Color::srgb(0.65, 0.85, 0.5),
                    )
                }
            }
            "Rock" => (&props.rock, kind_height("Rock", true), props.rock_aspect, Color::WHITE),
            "Log" => (&props.log, kind_height("Log", true), props.log_aspect, Color::WHITE),
            "CraftBench" => (&props.bench, kind_height("CraftBench", true), props.bench_aspect, Color::WHITE),
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
            // Bottom-center anchor: the base sits a quarter-slot below the
            // slot center, so small plants read as inside their tile while
            // tall trees still grow upward from their slot.
            Transform::from_xyz(
                wx,
                wy - idlecore_core::slots::SLOT_SIZE * 0.25 + size.y * 0.5,
                prop_depth(wy) + 0.55,
            ),
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
// Water tiles
// ============================================================================

/// Water tint per class (deep blue ocean → light inland water), applied to a
/// shared white square. No art pack contains water tiles.
#[derive(Resource, Default)]
pub struct WaterTextures {
    pub by_class: HashMap<WaterClass, [f32; 3]>,
    /// Shared 8×8 white square tinted per class.
    pub white: Option<Handle<Image>>,
}

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

/// Build the water tints + shared white square once.
pub fn init_water_textures(
    mut images: ResMut<Assets<Image>>,
    mut water: ResMut<WaterTextures>,
) {
    if water.white.is_some() {
        return;
    }
    let mut white = vec![255u8; 8 * 8 * 4];
    let handle = images.add(Image::new(
        Extent3d { width: 8, height: 8, depth_or_array_layers: 1 },
        TextureDimension::D2,
        std::mem::take(&mut white),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    for class in [
        WaterClass::Ocean,
        WaterClass::Sea,
        WaterClass::Coast,
        WaterClass::Lake,
        WaterClass::River,
        WaterClass::Wetland,
    ] {
        water.by_class.insert(class, water_color(class));
    }
    water.white = Some(handle);
}

// ============================================================================
// Procedural city textures (Hybrid world-gen: real cities, generated streets)
// ============================================================================

/// Building facade tint palettes (one per generated style variant).
pub const BUILDING_TINTS: [[f32; 3]; 5] = [
    [0.80, 0.82, 0.86], // concrete
    [0.86, 0.80, 0.70], // tan
    [0.70, 0.75, 0.82], // blue-grey
    [0.90, 0.85, 0.80], // warm stone
    [0.74, 0.80, 0.76], // green-grey
];

/// Procedurally generated city art: pavement, sidewalk, and a few building
/// facade styles (no dependency on guessing Tiny* tile indices for cities).
#[derive(Resource, Default)]
pub struct CityTextures {
    pub pavement: Handle<Image>,
    pub sidewalk: Handle<Image>,
    pub buildings: Vec<Handle<Image>>,
}

/// Flat square image filled with a single color (pavement / sidewalk).
fn make_solid_image(
    images: &mut Assets<Image>,
    size: u32,
    color: [u8; 3],
) -> Handle<Image> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    for i in 0..(size * size) as usize {
        pixels[i * 4..i * 4 + 3].copy_from_slice(&color);
        pixels[i * 4 + 3] = 255;
    }
    images.add(Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

/// Building facade: a base color with a grid of lighter windows.
fn make_facade_image(
    images: &mut Assets<Image>,
    w: u32,
    h: u32,
    base: [u8; 3],
    window: [u8; 3],
) -> Handle<Image> {
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let (wr, wg, wb) = (window[0] as i32, window[1] as i32, window[2] as i32);
    for y in 0..h {
        for x in 0..w {
            // Window grid: every ~4px with a 2px lit pane, margins at edges.
            let in_window = x % 4 >= 1 && x % 4 <= 2 && y % 5 >= 1 && y % 5 <= 3
                && x > 0 && x < w as u32 - 1 && y > 0 && y < h as u32 - 1;
            let (r, g, b) = if in_window {
                (wr, wg, wb)
            } else {
                (base[0] as i32, base[1] as i32, base[2] as i32)
            };
            let i = ((y * w + x) * 4) as usize;
            pixels[i] = r.clamp(0, 255) as u8;
            pixels[i + 1] = g.clamp(0, 255) as u8;
            pixels[i + 2] = b.clamp(0, 255) as u8;
            pixels[i + 3] = 255;
        }
    }
    images.add(Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

/// Build the procedural city art once at startup.
pub fn init_city_textures(mut images: ResMut<Assets<Image>>, mut city: ResMut<CityTextures>) {
    if !city.buildings.is_empty() {
        return;
    }
    city.pavement = make_solid_image(&mut images, 16, [120, 120, 128]);
    city.sidewalk = make_solid_image(&mut images, 16, [150, 150, 156]);
    let bases: [[u8; 3]; 5] = [
        [110, 112, 120],
        [120, 110, 95],
        [95, 105, 120],
        [125, 118, 110],
        [105, 115, 108],
    ];
    let wins: [[u8; 3]; 5] = [
        [200, 210, 230],
        [225, 205, 150],
        [180, 205, 230],
        [230, 215, 190],
        [190, 215, 195],
    ];
    city.buildings = bases
        .iter()
        .zip(wins.iter())
        .map(|(b, wn)| make_facade_image(&mut images, 16, 24, *b, *wn))
        .collect();
}

/// Tracks live floor-tile entities by slot so only tiles near the player
/// exist at all (slot-granular streaming, no chunk parents).
#[derive(Resource, Default)]
pub struct FloorTiles {
    pub live: HashMap<(i32, i32), Entity>,
    /// Separate buildings spawned on city `Building` slots (tall props that
    /// live in the prop band, not as children of the floor tile).
    pub buildings: HashMap<(i32, i32), Entity>,
    /// Player slot of the last completed rebuild; unchanged → skip the pass.
    pub last_player_slot: Option<(i32, i32)>,
}

/// Floor tiles spawn within this many slots of the player (≈69 units —
/// comfortably more than a screen at the default zoom). Nothing beyond this
/// exists.
pub const FLOOR_SPAWN_RADIUS_SLOTS: i32 = 16;

/// Live tiles despawn past this radius (hysteresis so border-walking
/// doesn't churn spawns/despawns every slot crossing).
const FLOOR_DESPAWN_RADIUS_SLOTS: i32 = 19;

/// Max floor tiles spawned per rebuild pass; the pass re-runs until the
/// wanted set is complete, so teleports fill in over a few frames instead
/// of hitching once.
const FLOOR_SPAWNS_PER_PASS: usize = 512;

/// World-space radius around the player to show world-object/plant sprites.
const RENDER_RADIUS_HEXES: f32 = 12.0 * WorldGenConfig::HEX_SIZE;

/// Max world-object sprites created per 150 ms scan; spreads replication
/// bursts across frames instead of hitching once.
const SPRITE_SPAWNS_PER_PASS: usize = 32;

/// Spawn/despawn floor tiles around the player: only slots within
/// `FLOOR_SPAWN_RADIUS_SLOTS` exist; everything else is despawned.
pub fn update_world_floor(
    mut commands: Commands,
    streaming_world: Res<StreamingWorldResource>,
    player_transform: Res<PlayerTransform>,
    mut tiles: ResMut<FloorTiles>,
    water: Res<WaterTextures>,
    solid: Res<SolidFloorTextures>,
    deco: Option<Res<DecoTextures>>,
    city: Option<Res<CityTextures>>,
) {
    // The solid textures stream in async; building tiles before they exist
    // would register empty slots that then never spawn.
    if solid.by_terrain.is_empty() {
        return;
    }
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let player_slot = world_pos_to_slot(px, py);

    // Perf: rebuild only when the player crosses a slot boundary (or when a
    // previous pass hit the spawn cap and left work unfinished).
    if tiles.last_player_slot == Some(player_slot) {
        return;
    }

    let (psx, psy) = player_slot;
    let spawn_r = FLOOR_SPAWN_RADIUS_SLOTS as f32 * SLOT_SIZE;
    let spawn_r2 = spawn_r * spawn_r;

    // Terrain per hex for the nearby chunks — one map built per pass, so the
    // per-slot lookups below are O(1).
    let (hq, hr) = world_pos_to_hex(px, py, WorldGenConfig::HEX_SIZE);
    let (ccq, ccr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);
    // A chunk spans 32 hexes (~554 units); ±2 chunks always covers the
    // spawn radius with margin.
    let mut terrain_of: HashMap<u64, (TerrainType, WaterClass)> = HashMap::new();
    for cq in (ccq - 2)..=(ccq + 2) {
        for cr in (ccr - 2)..=(ccr + 2) {
            let Some(chunk) = streaming_world.chunks.chunks.get(&(cq, cr)) else { continue };
            for cell in &chunk.cells {
                let id = idlecore_core::hex::HexCoord::new(cell.q, cell.r).to_id();
                terrain_of.insert(id, (cell.terrain, cell.water));
            }
        }
    }

    // Wanted set: slots in the square around the player, culled to a circle.
    // Sorted near-first so a capped pass fills the view center first.
    let mut wanted: Vec<(f32, i32, i32, TerrainType, WaterClass)> = Vec::new();
    for sx in (psx - FLOOR_SPAWN_RADIUS_SLOTS)..=(psx + FLOOR_SPAWN_RADIUS_SLOTS) {
        for sy in (psy - FLOOR_SPAWN_RADIUS_SLOTS)..=(psy + FLOOR_SPAWN_RADIUS_SLOTS) {
            let (cx, cy) = slot_center(sx, sy);
            let dx = cx - px;
            let dy = cy - py;
            let d2 = dx * dx + dy * dy;
            if d2 > spawn_r2 {
                continue;
            }
            let (hq, hr) = slot_hex(sx, sy);
            let Some(&(terrain, wat)) =
                terrain_of.get(&idlecore_core::hex::HexCoord::new(hq, hr).to_id())
            else {
                continue; // chunk not streamed yet — picked up on a later pass
            };
            wanted.push((d2, sx, sy, terrain, wat));
        }
    }
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Spawn missing tiles, nearest-first, capped per pass.
    let mut spawned = 0usize;
    let mut complete = true;
    for (_, sx, sy, terrain, wat) in &wanted {
        if tiles.live.contains_key(&(*sx, *sy)) {
            continue;
        }
        if spawned >= FLOOR_SPAWNS_PER_PASS {
            complete = false;
            break;
        }
        let (cx, cy) = slot_center(*sx, *sy);
        let (image, tint) = if *terrain == TerrainType::Water {
            let Some(white) = water.white.clone() else { continue };
            let tint = water
                .by_class
                .get(&wat)
                .or_else(|| water.by_class.values().next())
                .copied()
                .unwrap_or([0.15, 0.36, 0.58]);
            (white, tint)
        } else if *terrain == TerrainType::City {
            // Procedural urban floor: roads / plazas / building bases.
            let cell = city_cell(cx, cy);
            let road_variants = solid
                .by_terrain
                .get(&TerrainType::City)
                .cloned()
                .unwrap_or_default();
            match cell.kind {
                CityCellKind::Road => {
                    if road_variants.is_empty() {
                        continue;
                    }
                    let (h, t) = road_variants[cell.variant % road_variants.len()].clone();
                    (h, t)
                }
                CityCellKind::Block => {
                    let Some(c) = city.as_ref() else { continue };
                    (c.pavement.clone(), [1.0, 1.0, 1.0])
                }
                CityCellKind::Building => {
                    let Some(c) = city.as_ref() else { continue };
                    (c.sidewalk.clone(), [1.0, 1.0, 1.0])
                }
            }
        } else {
            let Some(variants) = solid.by_terrain.get(&terrain) else { continue };
            if variants.is_empty() {
                continue;
            }
            // Noise-driven variant clumping + micro-tint (Hybrid biome detail).
            let d = floor_detail(cx, cy, *terrain, variants.len());
            let (handle, base_tint) = variants[d.variant].clone();
            let tint = [
                base_tint[0] * d.tint[0],
                base_tint[1] * d.tint[1],
                base_tint[2] * d.tint[2],
            ];
            (handle, tint)
        };
        let entity = commands
            .spawn((
                Name::new(format!("tile({sx},{sy})")),
                Sprite {
                    image,
                    custom_size: Some(Vec2::splat(SLOT_SIZE * 1.02)),
                    color: Color::srgb(tint[0], tint[1], tint[2]),
                    ..default()
                },
                Transform::from_xyz(cx, cy, floor_depth(cy)),
            ))
            .id();
        // Ambient decoration: a plant garnish on ~1 in 6 land slots, a rare
        // critter on ~1 in 29. Child of the tile so it despawns with it.
        if let Some(deco) = deco.as_ref() {
            if let Some(set) = deco.by_terrain.get(terrain) {
                let h = slot_hash(*sx, *sy, 0xDE_C0_DE0);
                let tier = if h % 29 == 0 && !set.critters.is_empty() {
                    Some((&set.critters, 1.15))
                } else if h % 6 == 0 && !set.plants.is_empty() {
                    Some((&set.plants, 1.0))
                } else {
                    None
                };
                if let Some((list, scale)) = tier {
                    let d = &list[(h >> 8) as usize % list.len()];
                    let jx = ((h >> 13) % 9) as f32 / 9.0 - 0.4;
                    let jy = ((h >> 17) % 9) as f32 / 9.0 - 0.4;
                    let dy = cy + jy * SLOT_SIZE;
                    commands.entity(entity).with_child((
                        Name::new("deco"),
                        Sprite {
                            image: d.image.clone(),
                            custom_size: Some(Vec2::splat(d.height * scale)),
                            ..default()
                        },
                        // Bottom-anchored at a jittered spot in the slot; the
                        // +2 z lift clears the next tile row to the south.
                        bevy::sprite::Anchor::BOTTOM_CENTER,
                        Transform::from_xyz(cx + jx * SLOT_SIZE, dy + SLOT_SIZE * 0.2, 2.0),
                    ));
                }
            }
        }
        tiles.live.insert((*sx, *sy), entity);
        spawned += 1;

        // Procedural buildings on city `Building` slots — tall props in the
        // prop band so the skyline reads above the streets.
        if *terrain == TerrainType::City {
            let cell = city_cell(cx, cy);
            if matches!(cell.kind, CityCellKind::Building)
                && !tiles.buildings.contains_key(&(*sx, *sy))
            {
                if let Some(city_res) = city.as_ref() {
                    if !city_res.buildings.is_empty() {
                        let v = cell.variant % city_res.buildings.len();
                        let img = city_res.buildings[v].clone();
                        let btint = BUILDING_TINTS[v % BUILDING_TINTS.len()];
                        let bheight = cell.height.max(1.5);
                        let b = commands
                            .spawn((
                                Name::new(format!("building({sx},{sy})")),
                                Sprite {
                                    image: img,
                                    custom_size: Some(Vec2::new(SLOT_SIZE * 0.92, bheight)),
                                    color: Color::srgb(btint[0], btint[1], btint[2]),
                                    ..default()
                                },
                                bevy::sprite::Anchor::BOTTOM_CENTER,
                                Transform::from_xyz(cx, cy, prop_depth(cy) + 0.6),
                                Visibility::Visible,
                            ))
                            .id();
                        tiles.buildings.insert((*sx, *sy), b);
                    }
                }
            }
        }
    }

    // Despawn tiles beyond the hysteresis radius.
    let despawn_r2 = (FLOOR_DESPAWN_RADIUS_SLOTS as f32 * SLOT_SIZE).powi(2);
    let px2 = px;
    let py2 = py;
    let mut removed_slots: Vec<(i32, i32)> = Vec::new();
    tiles.live.retain(|(sx, sy), entity| {
        let (cx, cy) = slot_center(*sx, *sy);
        let dx = cx - px2;
        let dy = cy - py2;
        if dx * dx + dy * dy > despawn_r2 {
            commands.entity(*entity).despawn();
            removed_slots.push((*sx, *sy));
            return false;
        }
        true
    });
    for (sx, sy) in removed_slots {
        if let Some(be) = tiles.buildings.remove(&(sx, sy)) {
            commands.entity(be).despawn();
        }
    }

    // Only latch the rebuild trigger once the wanted set is fully live.
    if complete {
        tiles.last_player_slot = Some(player_slot);
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
    // Plant/pollution discs are small; only build them for hexes close to
    // the player (8 hexes ≈ 140 units) so far replicated rows stay data-only.
    let max_dist = 8.0f32;

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    // Hexes whose plant/pollution visual needs (re)building this pass.
    let mut plant_pending: Vec<(f32, u64)> = Vec::new();
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
                    state.parsed.insert(row.hex_id, p);
                }
                None => {
                    state.raw.remove(&row.hex_id);
                    state.parsed.remove(&row.hex_id);
                }
            }
        }
        let mature = state
            .parsed
            .get(&row.hex_id)
            .map(|p| now >= p.mature_at)
            .unwrap_or(false);

        let cached = state.stage.get(&row.hex_id).cloned();
        let band = eco_band(row.eco_rating);
        if cached == Some((is_polluted, mature, band)) {
            continue;
        }

        // Defer the actual sprite work to the budgeted pass below.
        plant_pending.push((0.0f32, row.hex_id));
    }

    // Budgeted respawn pass: same cap as world objects so a burst of new
    // tiles can't create hundreds of sprites in one frame.
    if plant_pending.len() > SPRITE_SPAWNS_PER_PASS {
        plant_pending.truncate(SPRITE_SPAWNS_PER_PASS);
    }
    for (_, hex_id) in plant_pending {
        let Some(row) = crate::net::gen::HexTileTableAccess::hex_tile(&conn.db)
            .hex_id()
            .find(&hex_id)
        else {
            continue;
        };
        let is_polluted = row.is_polluted;

        // Desired visual: plant diamond when one exists, else pollution disc.
        let kind: Option<Sprite> = state
            .parsed
            .get(&hex_id)
            .map(|p| plant_sprite(&p.kind_name, now >= p.mature_at))
            .or_else(|| is_polluted.then(|| Sprite {
                color: Color::srgb(0.18, 0.2, 0.16),
                custom_size: Some(Vec2::splat(2.4)),
                ..default()
            }));
        let mature = state
            .parsed
            .get(&hex_id)
            .map(|p| now >= p.mature_at)
            .unwrap_or(false);
        let band = eco_band(row.eco_rating);

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
mod tests_floor_variants {
    use super::*;
    use idlecore_core::terrain::TerrainType;

    #[test]
    fn every_terrain_has_multiple_seamless_variants() {
        for terrain in [
            TerrainType::Grass,
            TerrainType::Grassland,
            TerrainType::Forest,
            TerrainType::TropicalRainforest,
            TerrainType::Desert,
            TerrainType::Tundra,
            TerrainType::Taiga,
            TerrainType::Mountain,
            TerrainType::City,
            TerrainType::Polluted,
        ] {
            let variants = floor_tiles_for(terrain);
            assert!(variants.len() >= 2, "{terrain:?} needs >= 2 floor variants");
            for (path, tint) in variants {
                assert!(path.contains("Tiny"), "{terrain:?} non-tiny tile {path}");
                assert!(tint.iter().all(|c| (0.5..=1.3).contains(c)));
            }
        }
    }

    #[test]
    fn cell_variant_is_deterministic_and_in_range() {
        for q in -50..50 {
            for r in -50..50 {
                let v = cell_variant(q, r, 3);
                assert!(v < 3);
                assert_eq!(v, cell_variant(q, r, 3));
            }
        }
        // Adjacent cells should (statistically) differ; spot-check a pair.
        assert_ne!(cell_variant(0, 0, 3), cell_variant(1, 0, 3));
    }
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
    fn slot_outline_alpha_is_a_hollow_square_ring() {
        let img = slot_outline_image(18, 20, [1.0, 1.0, 0.0], 2.0);
        let data = img.data.expect("has pixels");
        let w = 18u32;
        let alpha = |x: u32, y: u32| data[((y * w + x) * 4 + 3) as usize];
        // Hollow interior.
        assert_eq!(alpha(9, 10), 0);
        // Opaque band along each edge midpoint.
        assert!(alpha(0, 10) > 200 || alpha(1, 10) > 200);
        assert!(alpha(9, 0) > 200);
        // Corners are part of the ring (square, not hexagonal).
        assert!(alpha(0, 0) > 200);
    }
}