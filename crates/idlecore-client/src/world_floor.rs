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
use crate::time_ext::now_unix_secs;
use spacetimedb_sdk::Table;
use std::collections::HashMap;
use idlecore_core::grid::world_pos_to_tile;
use idlecore_core::slots::{slot_center, slot_tile, world_pos_to_slot, SLOT_SIZE};
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
    pub x: i32,
    pub y: i32,
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
    let (q, r) = slot_tile(sx, sy);
    target.slot_x = sx;
    target.slot_y = sy;
    target.x = q;
    target.y = r;

    let (cx, cy) = slot_center(sx, sy);
    let new_translation = Vec3::new(cx, cy, crate::world_floor::prop_depth(cy) + 0.4);
    if transform.translation != new_translation {
        transform.translation = new_translation;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }

    // Green while the slot's hex is within the server's 1-hex interaction
    // range, dim otherwise.
    let player_hex = world_pos_to_tile(
        player_transform.translation.x,
        player_transform.translation.y,
        WorldGenConfig::TILE_SIZE,
    );
    let in_range = idlecore_core::grid::WorldGrid::distance(q, r, player_hex.0, player_hex.1) <= 1;
    let tint = if in_range { Color::srgba(0.6, 1.0, 0.5, 0.95) } else { Color::srgba(0.9, 0.9, 0.9, 0.35) };
    if sprite.color != tint {
        sprite.color = tint;
    }
}

// ============================================================================
// Prop textures: generated tuft/icons + loaded tree/rock art
// ============================================================================

/// Handles + aspects for every prop sprite. Fields are filled in as their
/// cropped art streams in; `ready` flips once the whole set is present, and
/// the world-object spawner waits for it so it never renders a fallback.
#[derive(Resource, Default)]
pub struct PropTextures {
    /// Default placeholder handle used before everything is ready.
    pub ready: bool,
    pub grass: Handle<Image>,
    pub grass_aspect: f32,
    /// Mature grass/wheat stand — taller than the young tuft.
    pub grass_mature: Handle<Image>,
    pub grass_mature_aspect: f32,
    pub tree: Handle<Image>,
    pub tree_aspect: f32,
    pub sapling: Handle<Image>,
    pub sapling_aspect: f32,
    pub rock: Handle<Image>,
    pub rock_aspect: f32,
    /// Fallen log / wood planks (Spec 022).
    pub log: Handle<Image>,
    pub log_aspect: f32,
    /// Craft bench (Spec 022): the Workbench, taller than plants.
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
    pub icon_watering_can: Handle<Image>,
    /// Car (and other vehicles) inventory icon — a city bus.
    pub icon_car: Handle<Image>,
    /// Spec 027 §14: weapon-forge inventory icons (one per tier + arrows).
    pub icon_arrow_item: Handle<Image>,
    /// Spec 033: the material × weapon icon matrix, keyed by the exact item
    /// name ("Gold Sword", …); armor pieces in armor_icons.
    pub weapon_icons: std::collections::HashMap<String, Handle<Image>>,
    pub armor_icons: std::collections::HashMap<String, Handle<Image>>,
    /// Stardew plot tiles + wheat growth frames (per-slot farming).
    pub plot_tilled: Handle<Image>,
    pub plot_wet: Handle<Image>,
    pub crop_stages: [Handle<Image>; 3],
}

/// Aspect ratios (art width ÷ art height) for the crop regions above.
fn grass_aspect() -> f32 { 1.0 }
fn grass_mature_aspect() -> f32 { 1.0 }
fn tree_aspect() -> f32 { 38.0 / 34.0 }
fn sapling_aspect() -> f32 { 1.0 }
fn rock_aspect() -> f32 { 1.0 }
// LOG: trunk-sheet cell is 32×16 → 2:1 lying log.
fn log_aspect() -> f32 { 2.0 }
fn bench_aspect() -> f32 { 1.0 }
// Fruit trees (cropped mature tree: ~112x48)
fn cherry_tree_aspect() -> f32 { 112.0 / 48.0 }
fn orange_tree_aspect() -> f32 { 112.0 / 48.0 }
fn apple_tree_aspect() -> f32 { 112.0 / 48.0 }
// Farm props
fn hay_bales_aspect() -> f32 { 1.0 }
fn scarecrow_aspect() -> f32 { 1.0 }
fn birdhouse_aspect() -> f32 { 1.0 }
fn well_aspect() -> f32 { 1.0 }
fn feed_trough_aspect() -> f32 { 1.0 }
// Fences & paths
fn fence_wood_aspect() -> f32 { 1.0 }
fn fence_white_aspect() -> f32 { 1.0 }
fn fence_stone_aspect() -> f32 { 1.0 }
fn path_tile_aspect() -> f32 { 1.0 }
// FX
fn clouds_aspect() -> f32 { 1.0 }
fn leaves_fall_aspect() -> f32 { 1.0 }
fn snow_aspect() -> f32 { 1.0 }
fn bonfire_aspect() -> f32 { 1.0 }
fn water_edge_aspect() -> f32 { 1.0 }

/// Atlas keys for every cropped sprite used below and by the floor/deco/water
/// initialisers. `enqueue_model_slices` (startup) requests all of them; the
/// per-resource inits wait until their keys are present in `SlicedAtlas`.
pub mod atlas {
    // Props / world objects
    pub const GRASS: &str = "grass";
    pub const GRASS_MATURE: &str = "grass_mature";
    pub const TREE: &str = "tree";
    pub const SAPLING: &str = "sapling";
    pub const ROCK: &str = "rock";
    pub const LOG: &str = "log";
    pub const BENCH: &str = "bench";
    // Inventory icons
    pub const ICON_SEED: &str = "icon_seed";
    pub const ICON_WOOD: &str = "icon_wood";
    pub const ICON_STONE: &str = "icon_stone";
    pub const ICON_GRASS: &str = "icon_grass";
    pub const ICON_LOG: &str = "icon_log";
    pub const ICON_PICKAXE: &str = "icon_pickaxe";
    pub const ICON_AXE: &str = "icon_axe";
    pub const ICON_SHOVEL: &str = "icon_shovel";
    pub const ICON_HOE: &str = "icon_hoe";
    pub const ICON_WATERING_CAN: &str = "icon_watering_can";
    pub const ICON_CAR: &str = "icon_car";
    // Tower-defense layer (Spec 023) — capital/structures + icons.
    // Spec 025: enemy roster crops (idle + 2 walk frames per kind).
    pub const TD_SLIME: &str = "td_slime";
    pub const TD_SLIME_W0: &str = "td_slime_w0";
    pub const TD_SLIME_W1: &str = "td_slime_w1";
    pub const TD_SLIME_BLUE: &str = "td_slime_blue";
    pub const TD_SLIME_BLUE_W0: &str = "td_slime_blue_w0";
    pub const TD_SLIME_BLUE_W1: &str = "td_slime_blue_w1";
    pub const TD_SLIME_PINK: &str = "td_slime_pink";
    pub const TD_SLIME_PINK_W0: &str = "td_slime_pink_w0";
    pub const TD_SLIME_PINK_W1: &str = "td_slime_pink_w1";
    pub const TD_SLIME_PUPPLE: &str = "td_slime_pupple";
    pub const TD_SLIME_PUPPLE_W0: &str = "td_slime_pupple_w0";
    pub const TD_SLIME_PUPPLE_W1: &str = "td_slime_pupple_w1";
    pub const TD_SLIME_BLACK: &str = "td_slime_black";
    pub const TD_SLIME_BLACK_W0: &str = "td_slime_black_w0";
    pub const TD_SLIME_BLACK_W1: &str = "td_slime_black_w1";
    pub const TD_BIG_SLIME: &str = "td_big_slime";
    pub const TD_BIG_SLIME_W0: &str = "td_big_slime_w0";
    pub const TD_BIG_SLIME_W1: &str = "td_big_slime_w1";
    pub const TD_GOBLIN_SPEAR: &str = "td_goblin_spear";
    pub const TD_GOBLIN_SPEAR_W0: &str = "td_goblin_spear_w0";
    pub const TD_GOBLIN_SPEAR_W1: &str = "td_goblin_spear_w1";
    pub const TD_GOBLIN_ARCHER: &str = "td_goblin_archer";
    pub const TD_GOBLIN_ARCHER_W0: &str = "td_goblin_archer_w0";
    pub const TD_GOBLIN_ARCHER_W1: &str = "td_goblin_archer_w1";
    pub const TD_GOBLIN_BOMB: &str = "td_goblin_bomb";
    pub const TD_GOBLIN_BOMB_W0: &str = "td_goblin_bomb_w0";
    pub const TD_GOBLIN_BOMB_W1: &str = "td_goblin_bomb_w1";
    pub const TD_MYCONID: &str = "td_myconid";
    pub const TD_MYCONID_W0: &str = "td_myconid_w0";
    pub const TD_MYCONID_W1: &str = "td_myconid_w1";
    pub const TD_MYCONID_BLUE: &str = "td_myconid_blue";
    pub const TD_MYCONID_BLUE_W0: &str = "td_myconid_blue_w0";
    pub const TD_MYCONID_BLUE_W1: &str = "td_myconid_blue_w1";
    pub const TD_MYCONID_PINK: &str = "td_myconid_pink";
    pub const TD_MYCONID_PINK_W0: &str = "td_myconid_pink_w0";
    pub const TD_MYCONID_PINK_W1: &str = "td_myconid_pink_w1";
    pub const TD_MYCONID_PURPLE: &str = "td_myconid_purple";
    pub const TD_MYCONID_PURPLE_W0: &str = "td_myconid_purple_w0";
    pub const TD_MYCONID_PURPLE_W1: &str = "td_myconid_purple_w1";
    pub const TD_MYCONID_RED: &str = "td_myconid_red";
    pub const TD_MYCONID_RED_W0: &str = "td_myconid_red_w0";
    pub const TD_MYCONID_RED_W1: &str = "td_myconid_red_w1";
    pub const TD_VENOM_BLOOM: &str = "td_venom_bloom";
    pub const TD_CAPITAL: &str = "td_capital";
    pub const TD_BARRICADE: &str = "td_barricade";
    pub const TD_ARROW_TOWER: &str = "td_arrow_tower";
    pub const TD_BALLISTA: &str = "td_ballista";
    // REV4 buildable expansion arts (walls / towers / traps).
    pub const TD_WOOD_WALL: &str = "td_wood_wall";
    pub const TD_IRON_WALL: &str = "td_iron_wall";
    pub const TD_LANTERN: &str = "td_lantern";
    pub const TD_COCONUT: &str = "td_coconut";
    pub const TD_SNOWMAN: &str = "td_snowman";
    pub const TD_TRAP: &str = "td_trap";
    pub const TD_WEB: &str = "td_web";
    pub const TD_FIRE: &str = "td_fire";
    pub const ICON_CAPITAL_CORE: &str = "icon_capital_core";
    pub const ICON_METAL_PART: &str = "icon_metal_part";
    pub const ICON_BONE: &str = "icon_bone";
    pub const ICON_BONE_BARRICADE: &str = "icon_bone_barricade";
    pub const ICON_ARROW_TOWER: &str = "icon_arrow_tower";
    pub const ICON_BALLISTA: &str = "icon_ballista";
    // Spec 027 §14: real weapon + ammo icons (whole 16×16 left cells of the
    // two-cell RPG icon sheets — plain variant, not the outlined duplicate;
    // crops verified against the rendered sheet).
    pub const ICON_ARROW_ITEM: &str = "icon_arrow_item";
    // REV4 build-menu icons (same crops as the world arts).
    pub const ICON_WOOD_WALL: &str = "icon_wood_wall";
    pub const ICON_IRON_WALL: &str = "icon_iron_wall";
    pub const ICON_LANTERN: &str = "icon_lantern";
    pub const ICON_COCONUT: &str = "icon_coconut";
    pub const ICON_SNOWMAN: &str = "icon_snowman";
    pub const ICON_SPIKE_TRAP: &str = "icon_spike_trap";
    pub const ICON_WEB_TRAP: &str = "icon_web_trap";
    pub const ICON_FIRE_TRAP: &str = "icon_fire_trap";
    // Floor tiles
    pub const FLOOR_GRASS: &str = "floor_grass";
    pub const FLOOR_SAND: &str = "floor_sand";
    pub const FLOOR_SNOW: &str = "floor_snow";
    pub const FLOOR_STONE: &str = "floor_stone";
    // Living-ground variants: real per-season seamless 16×16 interiors so the
    // floor mixes actual art instead of only tint shifts.
    pub const FLOOR_GRASS_SUMMER2: &str = "floor_grass_summer2";
    pub const FLOOR_GRASS_SPRING: &str = "floor_grass_spring";
    pub const FLOOR_GRASS_SPRING2: &str = "floor_grass_spring2";
    pub const FLOOR_GRASS_FALL: &str = "floor_grass_fall";
    pub const FLOOR_GRASS_FALL2: &str = "floor_grass_fall2";
    pub const FLOOR_SNOW2: &str = "floor_snow2";
    pub const FLOOR_DIRT: &str = "floor_dirt";
    pub const WATER: &str = "water";
    // Living-ground garnish: foliage/mushroom/pebble/water decal crops from
    // the "ALL props seasons.png" prop sheet.
    pub const GARNISH_SHEET: &str = "models/Tileset/ALL props seasons.png";
    pub const G_TUFT: &str = "g_tuft";
    pub const G_FLOWERS_PINK: &str = "g_flowers_pink";
    pub const G_FLOWERS_PURPLE: &str = "g_flowers_purple";
    pub const G_FLOWER_WHITE: &str = "g_flower_white";
    pub const G_BLUE_BUSH: &str = "g_blue_bush";
    pub const G_BUSH: &str = "g_bush";
    pub const G_MUSH_TAN: &str = "g_mush_tan";
    pub const G_MUSH_TAN2: &str = "g_mush_tan2";
    pub const G_MUSH_BLUE: &str = "g_mush_blue";
    pub const G_PEBBLES: &str = "g_pebbles";
    pub const G_MUSH_BABY: &str = "g_mush_baby";
    pub const G_MUSH_BABY2: &str = "g_mush_baby2";
    pub const G_PEBBLES_STONE: &str = "g_pebbles_stone";
    pub const G_PEBBLES_BURNT: &str = "g_pebbles_burnt";
    pub const G_SPARKLE: &str = "g_sparkle";
    pub const G_DROPLET: &str = "g_droplet";
    pub const G_PLANKS: &str = "g_planks";
    pub const G_LEAF_SPRAY: &str = "g_leaf_spray";
    pub const G_REED: &str = "g_reed";
    pub const G_DRIFTWOOD: &str = "g_driftwood";
    pub const G_MOSS: &str = "g_moss";
    // Stardew plot tiles (tilled & watered soil) + crop growth frames
    pub const PLOT_TILLED: &str = "plot_tilled";
    pub const PLOT_WET: &str = "plot_wet";
    pub const CROP_STAGE_1: &str = "crop_stage_1";
    pub const CROP_STAGE_2: &str = "crop_stage_2";
    pub const CROP_STAGE_MATURE: &str = "crop_stage_mature";
    // Ambient decorations (plants)
    pub const DECO_WHEAT: &str = "deco_wheat";
    pub const DECO_WHEAT2: &str = "deco_wheat2";
    pub const DECO_CARROT: &str = "deco_carrot";
    pub const DECO_BROCCOLI: &str = "deco_broccoli";
    pub const DECO_POTATO: &str = "deco_potato";
    pub const DECO_ONION: &str = "deco_onion";
    pub const DECO_STRAWBERRY: &str = "deco_strawberry";
    pub const DECO_BLUEBERRY: &str = "deco_blueberry";
    pub const DECO_ROCK2: &str = "deco_rock2";
    pub const DECO_TREE2: &str = "deco_tree2";
    pub const DECO_STONE: &str = "deco_stone";
    // Ambient decorations (critters / animals)
    pub const CRIT_CHICKEN: &str = "crit_chicken";
    pub const CRIT_SHEEP: &str = "crit_sheep";
    pub const CRIT_COW: &str = "crit_cow";
    pub const CRIT_FOX: &str = "crit_fox";
    pub const CRIT_DEER: &str = "crit_deer";
    pub const CRIT_RABBIT: &str = "crit_rabbit";
    pub const CRIT_BUTTERFLY: &str = "crit_butterfly";
    pub const CRIT_PENGUIN: &str = "crit_penguin";
    pub const CRIT_CAPYBARA: &str = "crit_capybara";
    // New critters for a livelier world
    pub const CRIT_DUCK: &str = "crit_duck";
    pub const CRIT_PIG: &str = "crit_pig";
    pub const CRIT_GOAT: &str = "crit_goat";
    pub const CRIT_OSTRICH: &str = "crit_ostrich";
    pub const CRIT_BEE: &str = "crit_bee";
    pub const CRIT_CROW: &str = "crit_crow";
    pub const CRIT_FROG: &str = "crit_frog";
    pub const CRIT_TURTLE: &str = "crit_turtle";
    pub const CRIT_MONARCH: &str = "crit_monarch";
    // Fall crops + fruit trees
    pub const DECO_PUMPKIN: &str = "deco_pumpkin";
    pub const DECO_CORN: &str = "deco_corn";
    pub const DECO_EGGPLANT: &str = "deco_eggplant";
    pub const TREE_CHERRY: &str = "tree_cherry";
    pub const TREE_ORANGE: &str = "tree_orange";
    pub const TREE_APPLE: &str = "tree_apple";
    // Farm props
    pub const PROP_HAY_BALES: &str = "prop_hay_bales";
    pub const PROP_SCARECROW: &str = "prop_scarecrow";
    pub const PROP_BIRDHOUSE: &str = "prop_birdhouse";
    pub const PROP_WELL: &str = "prop_well";
    pub const PROP_FEED_TROUGH: &str = "prop_feed_trough";
    // Fences & paths
    pub const PROP_FENCE_WOOD: &str = "prop_fence_wood";
    pub const PROP_FENCE_WHITE: &str = "prop_fence_white";
    pub const PROP_FENCE_STONE: &str = "prop_fence_stone";
    pub const PROP_PATH_TILE: &str = "prop_path_tile";
    // Weather / water FX
    pub const FX_CLOUDS: &str = "fx_clouds";
    pub const FX_LEAVES_FALL: &str = "fx_leaves_fall";
    pub const FX_SNOW: &str = "fx_snow";
    pub const FX_BONFIRE: &str = "fx_bonfire";
    pub const PROP_WATER_EDGE: &str = "prop_water_edge";
    // Animated frame strips (for animated deco)
    pub const FX_LEAVES_FALL_0: &str = "fx_leaves_fall_0";
    pub const FX_LEAVES_FALL_1: &str = "fx_leaves_fall_1";
    pub const FX_LEAVES_FALL_2: &str = "fx_leaves_fall_2";
    pub const FX_LEAVES_FALL_3: &str = "fx_leaves_fall_3";
    pub const FX_LEAVES_FALL_4: &str = "fx_leaves_fall_4";
    pub const FX_LEAVES_FALL_5: &str = "fx_leaves_fall_5";
    pub const FX_SNOW_0: &str = "fx_snow_0";
    pub const FX_SNOW_1: &str = "fx_snow_1";
    pub const FX_SNOW_2: &str = "fx_snow_2";
    pub const FX_SNOW_3: &str = "fx_snow_3";
    pub const FX_SNOW_4: &str = "fx_snow_4";
    pub const FX_SNOW_5: &str = "fx_snow_5";
    pub const FX_BONFIRE_0: &str = "fx_bonfire_0";
    pub const FX_BONFIRE_1: &str = "fx_bonfire_1";
    pub const FX_BONFIRE_2: &str = "fx_bonfire_2";
    pub const FX_BONFIRE_3: &str = "fx_bonfire_3";
    pub const FX_BONFIRE_4: &str = "fx_bonfire_4";
    pub const FX_BONFIRE_5: &str = "fx_bonfire_5";
    pub const FX_CLOUDS_0: &str = "fx_clouds_0";
    pub const FX_CLOUDS_1: &str = "fx_clouds_1";
    pub const FX_CLOUDS_2: &str = "fx_clouds_2";
    pub const FX_CLOUDS_3: &str = "fx_clouds_3";
    pub const FX_CLOUDS_4: &str = "fx_clouds_4";
    pub const FX_CLOUDS_5: &str = "fx_clouds_5";
    pub const FX_CLOUDS_6: &str = "fx_clouds_6";
    pub const FX_CLOUDS_7: &str = "fx_clouds_7";
    pub const FX_CLOUDS_8: &str = "fx_clouds_8";
    pub const PROP_WATER_EDGE_0: &str = "prop_water_edge_0";
    pub const PROP_WATER_EDGE_1: &str = "prop_water_edge_1";
    pub const PROP_WATER_EDGE_2: &str = "prop_water_edge_2";
    pub const PROP_WATER_EDGE_3: &str = "prop_water_edge_3";
    pub const CRIT_BUTTERFLY_0: &str = "crit_butterfly_0";
    pub const CRIT_BUTTERFLY_1: &str = "crit_butterfly_1";
    pub const CRIT_BUTTERFLY_2: &str = "crit_butterfly_2";
    pub const CRIT_BUTTERFLY_3: &str = "crit_butterfly_3";
    pub const CRIT_BUTTERFLY_4: &str = "crit_butterfly_4";
    pub const CRIT_BUTTERFLY_5: &str = "crit_butterfly_5";
    pub const CRIT_BUTTERFLY_6: &str = "crit_butterfly_6";
    pub const CRIT_MONARCH_0: &str = "crit_monarch_0";
    pub const CRIT_MONARCH_1: &str = "crit_monarch_1";
    pub const CRIT_MONARCH_2: &str = "crit_monarch_2";
    pub const CRIT_MONARCH_3: &str = "crit_monarch_3";
    pub const CRIT_MONARCH_4: &str = "crit_monarch_4";
    pub const CRIT_MONARCH_5: &str = "crit_monarch_5";
    pub const CRIT_MONARCH_6: &str = "crit_monarch_6";
    pub const CRIT_BEE_0: &str = "crit_bee_0";
    pub const CRIT_BEE_1: &str = "crit_bee_1";
    pub const CRIT_BEE_2: &str = "crit_bee_2";
    pub const CRIT_BEE_3: &str = "crit_bee_3";
}

/// Permanently allocate a dynamic path string (Spec 033 crop matrix paths
/// are built once at startup; the count is bounded by ~110 icons).
fn leak_path(path: String) -> &'static str {
    Box::leak(path.into_boxed_str())
}

/// Queue every sprite crop the game needs, against the new EmanuelleDev art.
/// Runs once at startup; `slice::pump_slices` fulfils them as sheets stream
/// in. Classic paths (`models/Tiny */...`) are gone.
pub fn enqueue_model_slices(
    mut requests: ResMut<crate::slice::SliceRequests>,
    asset_server: Res<AssetServer>,
) {
    use crate::slice::{request, CropRect as C};
    let mut r = |key: &str, path: &'static str, x: u32, y: u32, w: u32, h: u32| {
        request(&mut requests, &asset_server, key, path, C::new(x, y, w, h));
    };
    // ---- Props & world objects ----
    // Grass: tall stand of blades, tinted green at render time so it reads
    // as grass rather than golden wheat.
    r(atlas::GRASS, "models/Crops/Summer/Wheat.png", 0, 0, 16, 16);
    r(atlas::GRASS_MATURE, "models/Crops/Summer/Wheat.png", 80, 0, 16, 16);
    // Tree: a clean full-canopy mahogany tree. The sheet stacks a sapling
    // (with a pale trunk) above the mature tree, so the crop must take only
    // the bottom mature tree to avoid a stray white trunk.
    r(atlas::TREE, "models/Objects/Tree/Common/Shadow/Mahogany Tree.png", 123, 60, 38, 34);
    // Sapling: a young blade sprout, tinted green at render time.
    r(atlas::SAPLING, "models/Crops/Summer/Wheat.png", 0, 0, 16, 16);
    // Rock: a single rounded ground stone (not a two-stone cluster).
    r(atlas::ROCK, "models/Objects/Props/Spring/Ground stones.png", 16, 16, 16, 16);
    // Log: a proper cut-log with bark rim (not the flat plank it used to be).
    // The trunk sheet is 128×32 = 32×16 cells; the log spans the full first
    // cell (x4..29), so a 16×16 crop sliced it in half.
    r(atlas::LOG, "models/Objects/Tree/TREE TRUNKS copiar.png", 0, 0, 32, 16);
    // Craft bench: the whole Workbench sprite.
    r(atlas::BENCH, "models/Objects/Work Benches/Workbench.png", 0, 0, 32, 32);

    // ---- Inventory icons ----
    r(atlas::ICON_SEED, "models/Crops/Summer/Adzuki Bean.png", 0, 16, 16, 16);
    r(atlas::ICON_WOOD, "models/Icons/RPG icons/Extras/Wood.png", 0, 0, 32, 16);
    r(atlas::ICON_STONE, "models/Objects/Props/Spring/Ground stones.png", 0, 0, 16, 16);
    r(atlas::ICON_GRASS, "models/Crops/Summer/Wheat.png", 80, 0, 16, 16);
    r(atlas::ICON_LOG, "models/Objects/Tree/TREE TRUNKS copiar.png", 0, 0, 32, 16);
    r(atlas::ICON_PICKAXE, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Pickaxe.png", 2, 2, 28, 13);
    r(atlas::ICON_AXE, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Axe.png", 2, 0, 27, 15);
    r(atlas::ICON_SHOVEL, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Shovel.png", 2, 2, 28, 13);
    r(atlas::ICON_HOE, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Hoe.png", 2, 2, 28, 13);
    r(atlas::ICON_WATERING_CAN, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Watering can.png", 2, 2, 28, 13);
    r(atlas::ICON_CAR, "models/Objects/Exterior/Bus.png", 8, 6, 99, 55);

    // ---- Tower-defense layer (Spec 023) — existing art, tinted at render ----
    // Mobs: first standing frame of each animation strip (16×16 cells).
    // Spec 025 enemy roster: idle frame 0 + two walk frames per kind from the
    // shipped 32×32 Enemy sheets (Venom Bloom: rooted, 48×48, no walk anim).
    r(atlas::TD_SLIME, "models/Enemy/Slimes/Green/Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_W0, "models/Enemy/Slimes/Green/Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_W1, "models/Enemy/Slimes/Green/Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_SLIME_BLUE, "models/Enemy/Slimes/Blue/Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_BLUE_W0, "models/Enemy/Slimes/Blue/Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_BLUE_W1, "models/Enemy/Slimes/Blue/Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_SLIME_PINK, "models/Enemy/Slimes/Pink/Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_PINK_W0, "models/Enemy/Slimes/Pink/Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_PINK_W1, "models/Enemy/Slimes/Pink/Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_SLIME_PUPPLE, "models/Enemy/Slimes/Pupple/Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_PUPPLE_W0, "models/Enemy/Slimes/Pupple/Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_PUPPLE_W1, "models/Enemy/Slimes/Pupple/Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_SLIME_BLACK, "models/Enemy/Slimes/Black/Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_BLACK_W0, "models/Enemy/Slimes/Black/Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_SLIME_BLACK_W1, "models/Enemy/Slimes/Black/Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_BIG_SLIME, "models/Enemy/Slimes/Golden/Big Slime/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_BIG_SLIME_W0, "models/Enemy/Slimes/Golden/Big Slime/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_BIG_SLIME_W1, "models/Enemy/Slimes/Golden/Big Slime/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_GOBLIN_SPEAR, "models/Enemy/Goblins/Spear Goblin/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_SPEAR_W0, "models/Enemy/Goblins/Spear Goblin/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_SPEAR_W1, "models/Enemy/Goblins/Spear Goblin/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_GOBLIN_ARCHER, "models/Enemy/Goblins/Archer Goblin/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_ARCHER_W0, "models/Enemy/Goblins/Archer Goblin/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_ARCHER_W1, "models/Enemy/Goblins/Archer Goblin/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_GOBLIN_BOMB, "models/Enemy/Goblins/Bomb Goblin/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_BOMB_W0, "models/Enemy/Goblins/Bomb Goblin/Walk bomb.png", 0, 0, 32, 32);
    r(atlas::TD_GOBLIN_BOMB_W1, "models/Enemy/Goblins/Bomb Goblin/Walk bomb.png", 32, 0, 32, 32);
    r(atlas::TD_MYCONID, "models/Enemy/Myconid/Green/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_W0, "models/Enemy/Myconid/Green/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_W1, "models/Enemy/Myconid/Green/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_MYCONID_BLUE, "models/Enemy/Myconid/Blue/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_BLUE_W0, "models/Enemy/Myconid/Blue/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_BLUE_W1, "models/Enemy/Myconid/Blue/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_MYCONID_PINK, "models/Enemy/Myconid/Pink/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_PINK_W0, "models/Enemy/Myconid/Pink/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_PINK_W1, "models/Enemy/Myconid/Pink/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_MYCONID_PURPLE, "models/Enemy/Myconid/Purple/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_PURPLE_W0, "models/Enemy/Myconid/Purple/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_PURPLE_W1, "models/Enemy/Myconid/Purple/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_MYCONID_RED, "models/Enemy/Myconid/Red/Idle.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_RED_W0, "models/Enemy/Myconid/Red/Walk.png", 0, 0, 32, 32);
    r(atlas::TD_MYCONID_RED_W1, "models/Enemy/Myconid/Red/Walk.png", 32, 0, 32, 32);
    r(atlas::TD_VENOM_BLOOM, "models/Enemy/Venom Bloom/Purple/Idle.png", 0, 0, 48, 48);
    // Capital: one full house (128×112) — a city hall reads better than a well.
    r(atlas::TD_CAPITAL, "models/Objects/Exterior/Houses/1.png", 0, 0, 128, 112);
    // Barricade: the standing stone chunk at the sheet's center (verified
    // bbox 8,20 → 40,38; the old 24,32 crop straddled two 16px columns and
    // stitched two half-walls together).
    r(atlas::TD_BARRICADE, "models/Objects/Exterior/Fence and Bridge/Fence Stone.png", 8, 20, 32, 18);
    // Tower: a whole scarecrow — the sheet packs 8 figures at 32px stride
    // (each figure ~16px wide centered on every other 16px cell boundary),
    // so a 16px-wide crop grabbed the LEFT HALF of the figure.
    r(atlas::TD_ARROW_TOWER, "models/Objects/Exterior/Scarescrow.png", 4, 0, 24, 32);
    // Ballista: hay-bale stack, first cell.
    r(atlas::TD_BALLISTA, "models/Objects/Exterior/Hay Bales.png", 0, 0, 16, 16);
    // Icons: metal ingot (2nd cell of the bars row), bone (cell 1), and the
    // same placeholders as the world arts above.
    r(atlas::ICON_CAPITAL_CORE, "models/Objects/Exterior/Houses/1.png", 0, 0, 128, 112);
    r(atlas::ICON_METAL_PART, "models/Icons/RPG icons/Extras/Bars and ores.png", 16, 0, 16, 16);
    r(atlas::ICON_BONE, "models/Icons/RPG icons/Extras/Bones.png", 0, 0, 16, 16);
    r(atlas::ICON_BONE_BARRICADE, "models/Objects/Exterior/Fence and Bridge/Fence Stone.png", 8, 20, 32, 18);
    r(atlas::ICON_ARROW_TOWER, "models/Objects/Exterior/Scarescrow.png", 4, 0, 24, 32);
    r(atlas::ICON_BALLISTA, "models/Objects/Exterior/Hay Bales.png", 0, 0, 16, 16);
    // Spec 027 §14 weapon-forge icons (single 16×16 plain cells).
    r(atlas::ICON_ARROW_ITEM, "models/Icons/RPG icons/Weapons and Armor/1. Wood/Arrow.png", 0, 0, 16, 16);
    // Spec 033: the material × weapon matrix — every `Weapons and Armor`
    // folder (Wood…Obsidian) × every icon file. Single 16×16 plain cells,
    // verified against the rendered sheets (REV4). Keys mirror the core
    // item names: `wae_{mat}_{file}` with the material index and the
    // lowercase, space-stripped file stem.
    {
        use idlecore_core::player_combat::{WEA_KINDS, WEA_MATERIALS};
        let dirs = WEA_MATERIALS.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        for (mi, dir) in dirs.iter().enumerate() {
            for ki in 0..WEA_KINDS.len() {
                let stem = WEA_KINDS[ki].name;
                let path = format!("models/Icons/RPG icons/Weapons and Armor/{dir}/{stem}.png");
                r(
                    &format!("wae_{mi}_{ki}"),
                    leak_path(path),
                    0,
                    0,
                    16,
                    16,
                );
            }
        }
    }
    // Armor icons: Helmet/Chestplate/Leggings/Boots per material.
    {
        use idlecore_core::player_combat::{ARMOR_PIECES, WEA_MATERIALS};
        for (mi, (_, dir)) in WEA_MATERIALS.iter().enumerate() {
            for (pi, (piece, _)) in ARMOR_PIECES.iter().enumerate() {
                let path = format!("models/Icons/RPG icons/Weapons and Armor/{dir}/{piece}.png");
                r(
                    &format!("wae_armor_{mi}_{pi}"),
                    leak_path(path),
                    0,
                    0,
                    16,
                    16,
                );
            }
        }
    }
    // REV4 buildables — verified crops (world art = icon art).
    r(atlas::TD_WOOD_WALL, "models/Objects/Exterior/Fence and Bridge/Fence Wood.png", 48, 0, 32, 32);
    r(atlas::TD_IRON_WALL, "models/Objects/Exterior/Fence and Bridge/Fence Iron.png", 0, 0, 16, 32);
    r(atlas::TD_LANTERN, "models/Objects/Exterior/Street Lamp.png", 32, 0, 32, 48);
    r(atlas::TD_COCONUT, "models/Objects/Exterior/Beach/Coconut Tree.png", 0, 0, 48, 48);
    r(atlas::TD_SNOWMAN, "models/Objects/Exterior/Snowman.png", 0, 8, 16, 24);
    r(atlas::TD_TRAP, "models/Objects/Exterior/Mine and Dungeon/Trap.png", 32, 0, 16, 32);
    r(atlas::TD_WEB, "models/Objects/Exterior/Mine and Dungeon/web spider.png", 0, 0, 16, 16);
    r(atlas::TD_FIRE, "models/Objects/Exterior/Mine and Dungeon/bonfire.png", 16, 0, 16, 32);
    r(atlas::ICON_WOOD_WALL, "models/Objects/Exterior/Fence and Bridge/Fence Wood.png", 48, 0, 32, 32);
    r(atlas::ICON_IRON_WALL, "models/Objects/Exterior/Fence and Bridge/Fence Iron.png", 0, 0, 16, 32);
    r(atlas::ICON_LANTERN, "models/Objects/Exterior/Street Lamp.png", 32, 0, 32, 48);
    r(atlas::ICON_COCONUT, "models/Objects/Exterior/Beach/Coconut Tree.png", 0, 0, 48, 48);
    r(atlas::ICON_SNOWMAN, "models/Objects/Exterior/Snowman.png", 0, 8, 16, 24);
    r(atlas::ICON_SPIKE_TRAP, "models/Objects/Exterior/Mine and Dungeon/Trap.png", 32, 0, 16, 32);
    r(atlas::ICON_WEB_TRAP, "models/Objects/Exterior/Mine and Dungeon/web spider.png", 0, 0, 16, 16);
    r(atlas::ICON_FIRE_TRAP, "models/Objects/Exterior/Mine and Dungeon/bonfire.png", 16, 0, 16, 32);

    // ---- Floor tiles (plain seamless 16×16 crops) ----
    r(atlas::FLOOR_GRASS, "models/Tileset/Tileset Grass Summer.png", 144, 32, 16, 16);
    r(atlas::FLOOR_SAND, "models/Tileset/Tileset Grass Summer.png", 144, 224, 16, 16);
    r(atlas::FLOOR_SNOW, "models/Tileset/Tileset Grass Winter.png", 144, 32, 16, 16);
    r(atlas::FLOOR_STONE, "models/Tileset/Dungeon tileset.png", 144, 128, 16, 16);
    // Living-ground variants: the same seamless interior cell of the Spring
    // and Fall sheets (light/dark per sheet) — real pixel textures, not tint
    // approximations, so the ground mixes seasons instead of only shades.
    r(atlas::FLOOR_GRASS_SUMMER2, "models/Tileset/Tileset Grass Summer.png", 336, 32, 16, 16);
    r(atlas::FLOOR_GRASS_SPRING, "models/Tileset/Tileset Grass Spring.png", 144, 32, 16, 16);
    r(atlas::FLOOR_GRASS_SPRING2, "models/Tileset/Tileset Grass Spring.png", 336, 32, 16, 16);
    r(atlas::FLOOR_GRASS_FALL, "models/Tileset/Tileset Grass Fall.png", 144, 32, 16, 16);
    r(atlas::FLOOR_GRASS_FALL2, "models/Tileset/Tileset Grass Fall.png", 336, 32, 16, 16);
    r(atlas::FLOOR_SNOW2, "models/Tileset/Tileset Grass Winter.png", 336, 32, 16, 16);
    // Dirt: plain tilled dry-earth cell (furrowed brown soil).
    r(atlas::FLOOR_DIRT, "models/Tileset/Tilled Soil and wet soil.png", 64, 0, 16, 16);
    // ---- Living-ground garnish (prop crops from ALL props seasons) ----
    // Sheet bands verified on the sheet map: y64=flat garnish, y96=pebbles,
    // y112=water sparkles, y128=planks/reeds, y144=driftwood/flowers row.
    r(atlas::G_TUFT, atlas::GARNISH_SHEET, 17, 65, 5, 7);
    r(atlas::G_FLOWERS_PINK, atlas::GARNISH_SHEET, 146, 65, 13, 15);
    r(atlas::G_FLOWERS_PURPLE, atlas::GARNISH_SHEET, 208, 65, 16, 15);
    r(atlas::G_FLOWER_WHITE, atlas::GARNISH_SHEET, 131, 72, 9, 4);
    r(atlas::G_BLUE_BUSH, atlas::GARNISH_SHEET, 98, 66, 13, 12);
    r(atlas::G_BUSH, atlas::GARNISH_SHEET, 65, 68, 13, 9);
    r(atlas::G_MUSH_TAN, atlas::GARNISH_SHEET, 272, 65, 16, 15);
    r(atlas::G_MUSH_TAN2, atlas::GARNISH_SHEET, 304, 65, 16, 15);
    r(atlas::G_MUSH_BLUE, atlas::GARNISH_SHEET, 289, 82, 15, 13);
    r(atlas::G_PEBBLES, atlas::GARNISH_SHEET, 113, 87, 14, 7);
    r(atlas::G_MUSH_BABY, atlas::GARNISH_SHEET, 49, 103, 13, 8);
    r(atlas::G_MUSH_BABY2, atlas::GARNISH_SHEET, 66, 97, 12, 8);
    r(atlas::G_PEBBLES_STONE, atlas::GARNISH_SHEET, 128, 96, 16, 16);
    r(atlas::G_PEBBLES_BURNT, atlas::GARNISH_SHEET, 304, 96, 16, 16);
    r(atlas::G_SPARKLE, atlas::GARNISH_SHEET, 192, 112, 16, 16);
    r(atlas::G_DROPLET, atlas::GARNISH_SHEET, 0, 112, 16, 16);
    r(atlas::G_PLANKS, atlas::GARNISH_SHEET, 16, 128, 16, 16);
    r(atlas::G_LEAF_SPRAY, atlas::GARNISH_SHEET, 176, 128, 12, 13);
    r(atlas::G_REED, atlas::GARNISH_SHEET, 208, 128, 14, 15);
    r(atlas::G_DRIFTWOOD, atlas::GARNISH_SHEET, 144, 144, 32, 16);
    r(atlas::G_MOSS, atlas::GARNISH_SHEET, 0, 144, 32, 16);
    r(atlas::WATER, "models/Tileset/Water tile.png", 0, 0, 16, 16);

    // ---- Stardew plots (tilled/wet soil + wheat growth frames) ----
    r(atlas::PLOT_TILLED, "models/Tileset/Tilled Soil and wet soil.png", 16, 16, 16, 16);
    r(atlas::PLOT_WET, "models/Tileset/Tilled Soil and wet soil.png", 16, 80, 16, 16);
    r(atlas::CROP_STAGE_1, "models/Crops/Summer/Wheat.png", 0, 0, 16, 16);
    r(atlas::CROP_STAGE_2, "models/Crops/Summer/Wheat.png", 32, 0, 16, 16);
    r(atlas::CROP_STAGE_MATURE, "models/Crops/Summer/Wheat.png", 80, 0, 16, 16);

    // ---- Ambient decoration plants (crop strips, one frame each) ----
    r(atlas::DECO_WHEAT, "models/Crops/Summer/Wheat.png", 0, 0, 16, 16);
    r(atlas::DECO_WHEAT2, "models/Crops/Summer/Wheat.png", 48, 0, 16, 16);    r(atlas::DECO_CARROT, "models/Crops/Spring/Carrot.png", 64, 0, 16, 16);
    r(atlas::DECO_BROCCOLI, "models/Crops/Spring/Broccoli.png", 64, 0, 16, 16);
    r(atlas::DECO_POTATO, "models/Crops/Spring/Potato.png", 64, 0, 16, 16);
    r(atlas::DECO_ONION, "models/Crops/Spring/Onion.png", 112, 0, 16, 16);
    r(atlas::DECO_STRAWBERRY, "models/Crops/Spring/Strawberry.png", 64, 0, 16, 16);
    r(atlas::DECO_BLUEBERRY, "models/Crops/Spring/Blueberry.png", 82, 16, 16, 16);
    // Fall crops (new)
    r(atlas::DECO_PUMPKIN, "models/Crops/Fall/Pumpkin.png", 0, 0, 16, 16);
    r(atlas::DECO_CORN, "models/Crops/Fall/Corn.png", 0, 0, 16, 16);
    r(atlas::DECO_EGGPLANT, "models/Crops/Fall/Eggplant.png", 0, 0, 16, 16);
    r(atlas::DECO_ROCK2, "models/Objects/Props/Summer/Stones Summer.png", 36, 19, 12, 11);
    // Fruit trees: crop the mature tree from sheets that stack sapling + mature
    r(atlas::TREE_CHERRY, "models/Crops/Fruits Tree/Spring/Cherry Tree.png", 128, 0, 112, 48);
    r(atlas::TREE_ORANGE, "models/Crops/Fruits Tree/Summer/Orange Tree.png", 128, 0, 112, 48);
    r(atlas::TREE_APPLE, "models/Crops/Fruits Tree/Fall/Apple Tree.png", 144, 0, 112, 48);
    r(atlas::DECO_TREE2, "models/Objects/Tree/Common/Shadow/Mahogany Tree.png", 123, 60, 38, 34);
    r(atlas::DECO_STONE, "models/Objects/Props/Spring/Ground stones.png", 0, 0, 16, 16);

    // ---- Critters / animals ----
    r(atlas::CRIT_CHICKEN, "models/Animals/Farm/Chicken/Chicken White.png", 1, 0, 32, 48);
    r(atlas::CRIT_SHEEP, "models/Animals/Farm/Sheep/Sheep Male.png", 2, 15, 32, 62);
    r(atlas::CRIT_COW, "models/Animals/Farm/Cow/Common Cow/Female Cow Brown.png", 0, 13, 32, 62);
    r(atlas::CRIT_FOX, "models/Animals/Forest/Fox/Red Fox.png", 6, 17, 32, 62);
    r(atlas::CRIT_DEER, "models/Animals/Forest/Deer/Female/Idle.png", 5, 11, 32, 52);
    r(atlas::CRIT_RABBIT, "models/Animals/Forest/Rabbit/Rabbit Brown.png", 1, 3, 30, 28);
    r(atlas::CRIT_BUTTERFLY, "models/Animals/Forest/Bugs/Butterfly/Common Butterfly.png", 4, 4, 16, 12);
    r(atlas::CRIT_PENGUIN, "models/Animals/Forest/Penguin/Penguin.png", 1, 3, 16, 16);
    r(atlas::CRIT_CAPYBARA, "models/Animals/Forest/Capybara/Brown Capybara.png", 6, 13, 32, 62);
    // New critters
    r(atlas::CRIT_DUCK, "models/Animals/Farm/Ducks/Duck White.png", 0, 0, 16, 22);
    r(atlas::CRIT_PIG, "models/Animals/Farm/Pig/Pig Pink.png", 0, 0, 16, 22);
    r(atlas::CRIT_GOAT, "models/Animals/Farm/Goat/Goat Male Brown.png", 0, 0, 16, 22);
    r(atlas::CRIT_OSTRICH, "models/Animals/Farm/Ostrich/Ostrich Brown.png", 0, 0, 16, 16);
    r(atlas::CRIT_BEE, "models/Animals/Forest/Bugs/Bee/Bees.png", 0, 0, 16, 16);
    r(atlas::CRIT_CROW, "models/Animals/Forest/Crow/Crow.png", 0, 0, 16, 16);
    r(atlas::CRIT_FROG, "models/Animals/Forest/Frog/Frogs-Sheet.png", 0, 0, 16, 16);
    r(atlas::CRIT_TURTLE, "models/Animals/Forest/Turttle/Green/Idle.png", 0, 0, 16, 16);
    r(atlas::CRIT_MONARCH, "models/Animals/Forest/Bugs/Butterfly/Monarch Butterfly.png", 0, 0, 16, 16);
    // Farm props
    r(atlas::PROP_HAY_BALES, "models/Objects/Exterior/Hay Bales.png", 0, 0, 16, 16);
    r(atlas::PROP_SCARECROW, "models/Objects/Exterior/Scarescrow.png", 0, 0, 16, 16);
    r(atlas::PROP_BIRDHOUSE, "models/Objects/Exterior/Birdhouse.png", 0, 0, 32, 32);
    r(atlas::PROP_WELL, "models/Objects/Exterior/Well .png", 0, 0, 16, 16);
    r(atlas::PROP_FEED_TROUGH, "models/Objects/Exterior/Feed Trough.png", 0, 0, 16, 16);
    // Fences & paths
    r(atlas::PROP_FENCE_WOOD, "models/Objects/Exterior/Fence and Bridge/Fence Wood.png", 0, 0, 16, 16);
    r(atlas::PROP_FENCE_WHITE, "models/Objects/Exterior/Fence and Bridge/White Fence.png", 0, 0, 16, 16);
    r(atlas::PROP_FENCE_STONE, "models/Objects/Exterior/Fence and Bridge/Fence Stone.png", 0, 0, 16, 16);
    r(atlas::PROP_PATH_TILE, "models/Tileset/Path tiles.png", 0, 0, 16, 16);
    // Weather / water FX
    r(atlas::FX_CLOUDS, "models/Objects/Props/clouds.png", 0, 0, 16, 16);
    r(atlas::FX_LEAVES_FALL, "models/Objects/Tree/Common/Effects/FX Effects Orange Leafs Fall 2.png", 0, 0, 16, 16);
    r(atlas::FX_SNOW, "models/Objects/Tree/Common/Effects/FX Effects Snow Leafs Winter 2.png", 0, 0, 16, 16);
    r(atlas::FX_BONFIRE, "models/Objects/Exterior/Mine and Dungeon/bonfire.png", 0, 0, 16, 16);
    r(atlas::PROP_WATER_EDGE, "models/Objects/Props/Water props.png", 0, 0, 16, 16);
    // Animated frame strips
    // Leaves fall: 96x48 = 6x3 frames (16x16 each), row 0
    for i in 0..6 {
        r(
            match i {
                0 => atlas::FX_LEAVES_FALL_0,
                1 => atlas::FX_LEAVES_FALL_1,
                2 => atlas::FX_LEAVES_FALL_2,
                3 => atlas::FX_LEAVES_FALL_3,
                4 => atlas::FX_LEAVES_FALL_4,
                _ => atlas::FX_LEAVES_FALL_5,
            },
            "models/Objects/Tree/Common/Effects/FX Effects Orange Leafs Fall 2.png",
            i * 16, 0, 16, 16,
        );
    }
    // Snow: 96x48 = 6x3 frames, row 0
    for i in 0..6 {
        r(
            match i {
                0 => atlas::FX_SNOW_0,
                1 => atlas::FX_SNOW_1,
                2 => atlas::FX_SNOW_2,
                3 => atlas::FX_SNOW_3,
                4 => atlas::FX_SNOW_4,
                _ => atlas::FX_SNOW_5,
            },
            "models/Objects/Tree/Common/Effects/FX Effects Snow Leafs Winter 2.png",
            i * 16, 0, 16, 16,
        );
    }
    // Bonfire: 96x32 = 6x2 frames, row 0
    for i in 0..6 {
        r(
            match i {
                0 => atlas::FX_BONFIRE_0,
                1 => atlas::FX_BONFIRE_1,
                2 => atlas::FX_BONFIRE_2,
                3 => atlas::FX_BONFIRE_3,
                4 => atlas::FX_BONFIRE_4,
                _ => atlas::FX_BONFIRE_5,
            },
            "models/Objects/Exterior/Mine and Dungeon/bonfire.png",
            i * 16, 0, 16, 16,
        );
    }
    // Clouds: 144x96 = 9x6 frames, row 0 (first 9 frames)
    for i in 0..9 {
        r(
            match i {
                0 => atlas::FX_CLOUDS_0,
                1 => atlas::FX_CLOUDS_1,
                2 => atlas::FX_CLOUDS_2,
                3 => atlas::FX_CLOUDS_3,
                4 => atlas::FX_CLOUDS_4,
                5 => atlas::FX_CLOUDS_5,
                6 => atlas::FX_CLOUDS_6,
                7 => atlas::FX_CLOUDS_7,
                _ => atlas::FX_CLOUDS_8,
            },
            "models/Objects/Props/clouds.png",
            i * 16, 0, 16, 16,
        );
    }
    // Water edge: 64x16 = 4 frames
    for i in 0..4 {
        r(
            match i {
                0 => atlas::PROP_WATER_EDGE_0,
                1 => atlas::PROP_WATER_EDGE_1,
                2 => atlas::PROP_WATER_EDGE_2,
                _ => atlas::PROP_WATER_EDGE_3,
            },
            "models/Objects/Props/Water props.png",
            i * 16, 0, 16, 16,
        );
    }
    // Common Butterfly: 112x32 = 7x2 frames, row 0
    for i in 0..7 {
        r(
            match i {
                0 => atlas::CRIT_BUTTERFLY_0,
                1 => atlas::CRIT_BUTTERFLY_1,
                2 => atlas::CRIT_BUTTERFLY_2,
                3 => atlas::CRIT_BUTTERFLY_3,
                4 => atlas::CRIT_BUTTERFLY_4,
                5 => atlas::CRIT_BUTTERFLY_5,
                _ => atlas::CRIT_BUTTERFLY_6,
            },
            "models/Animals/Forest/Bugs/Butterfly/Common Butterfly.png",
            i * 16, 0, 16, 12,
        );
    }
    // Monarch Butterfly: assume similar 7 frames
    for i in 0..7 {
        r(
            match i {
                0 => atlas::CRIT_MONARCH_0,
                1 => atlas::CRIT_MONARCH_1,
                2 => atlas::CRIT_MONARCH_2,
                3 => atlas::CRIT_MONARCH_3,
                4 => atlas::CRIT_MONARCH_4,
                5 => atlas::CRIT_MONARCH_5,
                _ => atlas::CRIT_MONARCH_6,
            },
            "models/Animals/Forest/Bugs/Butterfly/Monarch Butterfly.png",
            i * 16, 0, 16, 16,
        );
    }
    // Bee: 64x16 = 4 frames
    for i in 0..4 {
        r(
            match i {
                0 => atlas::CRIT_BEE_0,
                1 => atlas::CRIT_BEE_1,
                2 => atlas::CRIT_BEE_2,
                _ => atlas::CRIT_BEE_3,
            },
            "models/Animals/Forest/Bugs/Bee/Bees.png",
            i * 16, 0, 16, 16,
        );
    }
}

/// Build the prop/icon handle set once every cropped sprite has streamed in.
/// Polls `SlicedAtlas` (filled by `slice::pump_slices`) and inserts
/// `PropTextures` a single time.
pub fn init_prop_textures(
    mut commands: Commands,
    atlas: Res<crate::slice::SlicedAtlas>,
    mut props: ResMut<PropTextures>,
) {
    if props.ready {
        return;
    }
    let Some(grass) = atlas.items.get(atlas::GRASS).cloned() else { return };
    let Some(grass_mature) = atlas.items.get(atlas::GRASS_MATURE).cloned() else { return };
    let Some(tree) = atlas.items.get(atlas::TREE).cloned() else { return };
    let Some(sapling) = atlas.items.get(atlas::SAPLING).cloned() else { return };
    let Some(rock) = atlas.items.get(atlas::ROCK).cloned() else { return };
    let Some(log) = atlas.items.get(atlas::LOG).cloned() else { return };
    let Some(bench) = atlas.items.get(atlas::BENCH).cloned() else { return };
    let Some(icon_seed) = atlas.items.get(atlas::ICON_SEED).cloned() else { return };
    let Some(icon_wood) = atlas.items.get(atlas::ICON_WOOD).cloned() else { return };
    let Some(icon_stone) = atlas.items.get(atlas::ICON_STONE).cloned() else { return };
    let Some(icon_grass) = atlas.items.get(atlas::ICON_GRASS).cloned() else { return };
    let Some(icon_log) = atlas.items.get(atlas::ICON_LOG).cloned() else { return };
    let Some(icon_pickaxe) = atlas.items.get(atlas::ICON_PICKAXE).cloned() else { return };
    let Some(icon_axe) = atlas.items.get(atlas::ICON_AXE).cloned() else { return };
    let Some(icon_shovel) = atlas.items.get(atlas::ICON_SHOVEL).cloned() else { return };
    let Some(icon_hoe) = atlas.items.get(atlas::ICON_HOE).cloned() else { return };
    let Some(icon_watering_can) = atlas.items.get(atlas::ICON_WATERING_CAN).cloned() else { return };
    let Some(icon_car) = atlas.items.get(atlas::ICON_CAR).cloned() else { return };
    let Some(icon_arrow_item) = atlas.items.get(atlas::ICON_ARROW_ITEM).cloned() else { return };
    // Spec 033: the matrix slips in only once every crop landed.
    let mut weapon_icons = std::collections::HashMap::new();
    let mut armor_icons = std::collections::HashMap::new();
    {
        use idlecore_core::player_combat::{ARMOR_PIECES, WEA_KINDS, WEA_MATERIALS};
        for (mi, (mat, _)) in WEA_MATERIALS.iter().enumerate() {
            for (ki, kind) in WEA_KINDS.iter().enumerate() {
                let key = format!("wae_{mi}_{ki}");
                let Some(handle) = atlas.items.get(&key).cloned() else { return };
                weapon_icons.insert(format!("{mat} {}", kind.name), handle);
            }
            for (pi, (piece, _)) in ARMOR_PIECES.iter().enumerate() {
                let key = format!("wae_armor_{mi}_{pi}");
                let Some(handle) = atlas.items.get(&key).cloned() else { return };
                armor_icons.insert(format!("{mat} {piece}"), handle);
            }
        }
    }
    let Some(plot_tilled) = atlas.items.get(atlas::PLOT_TILLED).cloned() else { return };
    let Some(plot_wet) = atlas.items.get(atlas::PLOT_WET).cloned() else { return };
    let Some(crop_stage_1) = atlas.items.get(atlas::CROP_STAGE_1).cloned() else { return };
    let Some(crop_stage_2) = atlas.items.get(atlas::CROP_STAGE_2).cloned() else { return };
    let Some(crop_stage_mature) = atlas.items.get(atlas::CROP_STAGE_MATURE).cloned() else { return };

    props.grass = grass;
    props.grass_aspect = grass_aspect();
    props.grass_mature = grass_mature;
    props.grass_mature_aspect = grass_mature_aspect();
    props.tree = tree;
    props.tree_aspect = tree_aspect();
    props.sapling = sapling;
    props.sapling_aspect = sapling_aspect();
    props.rock = rock;
    props.rock_aspect = rock_aspect();
    props.log = log;
    props.log_aspect = log_aspect();
    props.bench = bench;
    props.bench_aspect = bench_aspect();
    props.icon_seed = icon_seed;
    props.icon_wood = icon_wood;
    props.icon_stone = icon_stone;
    props.icon_grass = icon_grass;
    props.icon_log = icon_log;
    props.icon_pickaxe = icon_pickaxe;
    props.icon_axe = icon_axe;
    props.icon_shovel = icon_shovel;
    props.icon_hoe = icon_hoe;
    props.icon_watering_can = icon_watering_can;
    props.icon_car = icon_car;
    props.icon_arrow_item = icon_arrow_item;
    props.weapon_icons = weapon_icons;
    props.armor_icons = armor_icons;
    props.plot_tilled = plot_tilled;
    props.plot_wet = plot_wet;
    props.crop_stages = [crop_stage_1, crop_stage_2, crop_stage_mature];
    props.ready = true;

    // No decoration depends on these; emit a log once so it's clear the
    // new-art pipeline is live.
    info!("Prop/icon textures built from cropped EmanuelleDev art");
    let _ = commands;
}

// ============================================================================
// Ambient decorations — plants & critters from across the Tiny* packs
// ============================================================================

/// One decoration entry: sprite handle, on-screen height (world units),
/// render tint, and crop aspect (width/height — Deco sprites are
/// bottom-anchored, so non-square crops keep their proportions).
pub struct Deco {
    pub image: Handle<Image>,
    pub height: f32,
    pub tint: [f32; 3],
    pub aspect: f32,
    /// Optional base key for animated deco (e.g., "fx_leaves_fall").
    /// If present, the spawned deco will get an AnimatedDeco component.
    pub anim_key: Option<String>,
}

/// Ambient decoration set per terrain: `plants` are common garnish,
/// `critters` are rare animals/props that make the world feel alive.
/// Purely visual — deterministic per slot, no server data involved.
#[derive(Resource, Default)]
pub struct DecoTextures {
    pub by_terrain: HashMap<TerrainType, DecoSet>,
    /// True once the cropped art has been assembled.
    pub built: bool,
}

#[derive(Default)]
pub struct DecoSet {
    pub plants: Vec<Deco>,
    pub critters: Vec<Deco>,
}

/// Frame handles for animated deco sprites.
#[derive(Resource, Default)]
pub struct AnimatedDecoFrames {
    /// Map from base key to vector of frame handles.
    pub frames: HashMap<String, Vec<Handle<Image>>>,
}

/// Component marking a deco entity as animated.
#[derive(Component)]
pub struct AnimatedDeco {
    pub base_key: String,
    pub frame_keys: Vec<String>,
    pub frame_index: usize,
    pub timer: Timer,
}

/// Initialize animated deco frame handles from the sliced atlas.
pub fn init_animated_deco_frames(
    mut commands: Commands,
    atlas: Res<crate::slice::SlicedAtlas>,
    mut anim: ResMut<AnimatedDecoFrames>,
) {
    if !anim.frames.is_empty() {
        return;
    }
    let get = |k: &str| atlas.items.get(k).expect("animated frame missing").clone();
    // Leaves fall: 6 frames
    anim.frames.insert(
        "fx_leaves_fall".into(),
        (0..6).map(|i| get(&format!("fx_leaves_fall_{}", i))).collect(),
    );
    // Snow: 6 frames
    anim.frames.insert(
        "fx_snow".into(),
        (0..6).map(|i| get(&format!("fx_snow_{}", i))).collect(),
    );
    // Bonfire: 6 frames
    anim.frames.insert(
        "fx_bonfire".into(),
        (0..6).map(|i| get(&format!("fx_bonfire_{}", i))).collect(),
    );
    // Clouds: 9 frames
    anim.frames.insert(
        "fx_clouds".into(),
        (0..9).map(|i| get(&format!("fx_clouds_{}", i))).collect(),
    );
    // Water edge: 4 frames
    anim.frames.insert(
        "prop_water_edge".into(),
        (0..4).map(|i| get(&format!("prop_water_edge_{}", i))).collect(),
    );
    // Common butterfly: 7 frames
    anim.frames.insert(
        "crit_butterfly".into(),
        (0..7).map(|i| get(&format!("crit_butterfly_{}", i))).collect(),
    );
    // Monarch butterfly: 7 frames
    anim.frames.insert(
        "crit_monarch".into(),
        (0..7).map(|i| get(&format!("crit_monarch_{}", i))).collect(),
    );
    // Bee: 4 frames
    anim.frames.insert(
        "crit_bee".into(),
        (0..4).map(|i| get(&format!("crit_bee_{}", i))).collect(),
    );
}

/// Animate deco sprites by cycling through frames.
pub fn animate_deco(
    time: Res<Time>,
    anim_frames: Res<AnimatedDecoFrames>,
    mut query: Query<(&mut Sprite, &mut AnimatedDeco)>,
) {
    let dt = time.delta();
    for (mut sprite, mut anim) in &mut query {
        anim.timer.tick(dt);
        if anim.timer.just_finished() {
            anim.frame_index = (anim.frame_index + 1) % anim.frame_keys.len();
            if let Some(frames) = anim_frames.frames.get(&anim.base_key) {
                if anim.frame_index < frames.len() {
                    sprite.image = frames[anim.frame_index].clone();
                }
            }
        }
    }
}

/// Build the per-terrain decoration sets once all their cropped sprites have
/// streamed in. Polls `SlicedAtlas`; inserts `DecoTextures` a single time.
pub fn init_deco_textures(
    mut commands: Commands,
    atlas: Res<crate::slice::SlicedAtlas>,
    mut deco: ResMut<DecoTextures>,
) {
    if !deco.by_terrain.is_empty() || deco.built {
        return;
    }
    let need = |keys: &[&str]| keys.iter().all(|k| atlas.items.contains_key(*k));
    if !need(&[
        atlas::DECO_WHEAT, atlas::DECO_WHEAT2, atlas::DECO_CARROT,
        atlas::DECO_BROCCOLI, atlas::DECO_POTATO, atlas::DECO_ONION,
        atlas::DECO_STRAWBERRY, atlas::DECO_BLUEBERRY, atlas::DECO_ROCK2,
        atlas::DECO_TREE2, atlas::DECO_STONE,
        // Fall crops + fruit trees
        atlas::DECO_PUMPKIN, atlas::DECO_CORN, atlas::DECO_EGGPLANT,
        atlas::TREE_CHERRY, atlas::TREE_ORANGE, atlas::TREE_APPLE,
        // New critters
        atlas::CRIT_CHICKEN, atlas::CRIT_SHEEP, atlas::CRIT_COW,
        atlas::CRIT_FOX, atlas::CRIT_DEER, atlas::CRIT_RABBIT,
        atlas::CRIT_BUTTERFLY, atlas::CRIT_PENGUIN, atlas::CRIT_CAPYBARA,
        atlas::CRIT_DUCK, atlas::CRIT_PIG, atlas::CRIT_GOAT,
        atlas::CRIT_OSTRICH, atlas::CRIT_BEE, atlas::CRIT_CROW,
        atlas::CRIT_FROG, atlas::CRIT_TURTLE, atlas::CRIT_MONARCH,
        // Farm props
        atlas::PROP_HAY_BALES, atlas::PROP_SCARECROW, atlas::PROP_BIRDHOUSE,
        atlas::PROP_WELL, atlas::PROP_FEED_TROUGH,
        // Living-ground garnish (ALL props seasons crops)
        atlas::G_TUFT, atlas::G_FLOWERS_PINK, atlas::G_FLOWERS_PURPLE,
        atlas::G_FLOWER_WHITE, atlas::G_BLUE_BUSH, atlas::G_BUSH,
        atlas::G_MUSH_TAN, atlas::G_MUSH_TAN2, atlas::G_MUSH_BLUE,
        atlas::G_PEBBLES, atlas::G_MUSH_BABY, atlas::G_MUSH_BABY2,
        atlas::G_PEBBLES_STONE, atlas::G_PEBBLES_BURNT, atlas::G_SPARKLE, atlas::G_DROPLET,
        atlas::G_PLANKS, atlas::G_LEAF_SPRAY, atlas::G_REED,
        atlas::G_DRIFTWOOD, atlas::G_MOSS,
        // Fences & paths
        atlas::PROP_FENCE_WOOD, atlas::PROP_FENCE_WHITE, atlas::PROP_FENCE_STONE,
        atlas::PROP_PATH_TILE,
        // Weather / water FX
        atlas::FX_CLOUDS, atlas::FX_LEAVES_FALL, atlas::FX_SNOW,
        atlas::FX_BONFIRE, atlas::PROP_WATER_EDGE,
        // Animated frame strips
        atlas::FX_LEAVES_FALL_0, atlas::FX_LEAVES_FALL_1, atlas::FX_LEAVES_FALL_2,
        atlas::FX_LEAVES_FALL_3, atlas::FX_LEAVES_FALL_4, atlas::FX_LEAVES_FALL_5,
        atlas::FX_SNOW_0, atlas::FX_SNOW_1, atlas::FX_SNOW_2,
        atlas::FX_SNOW_3, atlas::FX_SNOW_4, atlas::FX_SNOW_5,
        atlas::FX_BONFIRE_0, atlas::FX_BONFIRE_1, atlas::FX_BONFIRE_2,
        atlas::FX_BONFIRE_3, atlas::FX_BONFIRE_4, atlas::FX_BONFIRE_5,
        atlas::FX_CLOUDS_0, atlas::FX_CLOUDS_1, atlas::FX_CLOUDS_2,
        atlas::FX_CLOUDS_3, atlas::FX_CLOUDS_4, atlas::FX_CLOUDS_5,
        atlas::FX_CLOUDS_6, atlas::FX_CLOUDS_7, atlas::FX_CLOUDS_8,
        atlas::PROP_WATER_EDGE_0, atlas::PROP_WATER_EDGE_1,
        atlas::PROP_WATER_EDGE_2, atlas::PROP_WATER_EDGE_3,
        atlas::CRIT_BUTTERFLY_0, atlas::CRIT_BUTTERFLY_1, atlas::CRIT_BUTTERFLY_2,
        atlas::CRIT_BUTTERFLY_3, atlas::CRIT_BUTTERFLY_4, atlas::CRIT_BUTTERFLY_5,
        atlas::CRIT_BUTTERFLY_6,
        atlas::CRIT_MONARCH_0, atlas::CRIT_MONARCH_1, atlas::CRIT_MONARCH_2,
        atlas::CRIT_MONARCH_3, atlas::CRIT_MONARCH_4, atlas::CRIT_MONARCH_5,
        atlas::CRIT_MONARCH_6,
        atlas::CRIT_BEE_0, atlas::CRIT_BEE_1, atlas::CRIT_BEE_2, atlas::CRIT_BEE_3,
    ]) {
        return;
    }
    let get = |k: &str| atlas.items.get(k).expect("checked").clone();
    let to_deco = |k: &str, height: f32| Deco {
        image: get(k),
        height,
        tint: [1.0, 1.0, 1.0],
        aspect: 1.0,
        anim_key: None,
    };
    // Non-square garnish crops keep their proportions (bottom-anchored).
    let to_garnish = |k: &str, height: f32, w: f32, h: f32, tint: [f32; 3]| Deco {
        image: get(k),
        height,
        tint,
        aspect: w / h,
        anim_key: None,
    };
    let to_deco_anim = |k: &str, height: f32, anim_key: &str| Deco {
        image: get(k),
        height,
        tint: [1.0, 1.0, 1.0],
        aspect: 1.0,
        anim_key: Some(anim_key.into()),
    };

    let mut sets: HashMap<TerrainType, DecoSet> = HashMap::new();
    let mut add = |terrain: TerrainType, critter: bool, d: Deco| {
        let set = sets.entry(terrain).or_default();
        if critter { set.critters.push(d) } else { set.plants.push(d) }
    };
    // Meadow: young wheat, strawberry, broccoli, fall crops — plus farm animals and props,
    // and the prop-sheet garnish (flowers / tufts / blue bush / mushrooms).
    for k in [atlas::DECO_WHEAT2, atlas::DECO_STRAWBERRY, atlas::DECO_BROCCOLI,
              atlas::DECO_PUMPKIN, atlas::DECO_CORN, atlas::DECO_EGGPLANT] {
        add(TerrainType::Grass, false, to_deco(k, 1.0));
    }
    add(TerrainType::Grass, false, to_garnish(atlas::G_FLOWERS_PINK, 0.9, 13.0, 15.0, [1.0, 1.0, 1.0]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_FLOWERS_PURPLE, 0.85, 16.0, 15.0, [1.0, 1.0, 1.0]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_FLOWER_WHITE, 0.5, 9.0, 4.0, [1.0, 1.0, 1.0]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_BLUE_BUSH, 0.9, 13.0, 12.0, [1.0, 1.05, 1.1]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_BUSH, 0.8, 13.0, 9.0, [1.0, 1.0, 1.0]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_MUSH_TAN, 0.8, 16.0, 15.0, [1.05, 1.0, 0.95]));
    add(TerrainType::Grass, false, to_garnish(atlas::G_TUFT, 0.55, 5.0, 7.0, [1.0, 1.0, 1.0]));
    // Farm props on meadow
    for k in [atlas::PROP_HAY_BALES, atlas::PROP_SCARECROW, atlas::PROP_BIRDHOUSE,
              atlas::PROP_WELL, atlas::PROP_FEED_TROUGH] {
        add(TerrainType::Grass, false, to_deco(k, 1.2));
    }
    // Fences & paths on meadow
    for k in [atlas::PROP_FENCE_WOOD, atlas::PROP_FENCE_WHITE, atlas::PROP_PATH_TILE] {
        add(TerrainType::Grass, false, to_deco(k, 1.0));
    }
    // Critters on meadow: chickens, ducks, pigs, goats, ostriches, bees, crows
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_CHICKEN, 1.1));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_DUCK, 0.9));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_PIG, 1.0));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_GOAT, 1.0));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_OSTRICH, 1.3));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_BEE, 0.5));
    add(TerrainType::Grass, true, to_deco(atlas::CRIT_CROW, 0.8));
    // Plains: golden wheat, with cows, sheep, and farm animals + dry garnish.
    add(TerrainType::Grassland, false, to_deco(atlas::DECO_WHEAT, 1.1));
    add(TerrainType::Grassland, false, to_deco(atlas::DECO_PUMPKIN, 1.1));
    add(TerrainType::Grassland, false, to_garnish(atlas::G_TUFT, 0.6, 5.0, 7.0, [1.2, 1.05, 0.72]));
    add(TerrainType::Grassland, false, to_garnish(atlas::G_FLOWER_WHITE, 0.5, 9.0, 4.0, [1.15, 1.05, 0.8]));
    add(TerrainType::Grassland, false, to_garnish(atlas::G_PEBBLES, 0.5, 14.0, 7.0, [1.05, 1.0, 0.95]));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_SHEEP, 1.7));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_COW, 1.5));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_DUCK, 0.9));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_PIG, 1.0));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_GOAT, 1.0));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_OSTRICH, 1.3));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_BEE, 0.5));
    add(TerrainType::Grassland, true, to_deco(atlas::CRIT_CROW, 0.8));
    // Woods: carrot, onions, fall crops under trees; deer, fox, monarch butterflies, frogs, crows; fruit trees.
    add(TerrainType::Forest, false, to_deco(atlas::DECO_CARROT, 0.9));
    add(TerrainType::Forest, false, to_deco(atlas::DECO_ONION, 0.8));
    add(TerrainType::Forest, false, to_deco(atlas::DECO_PUMPKIN, 0.9));
    add(TerrainType::Forest, false, to_deco(atlas::DECO_CORN, 0.9));
    add(TerrainType::Forest, false, to_deco(atlas::DECO_EGGPLANT, 0.9));
    add(TerrainType::Forest, false, to_deco(atlas::TREE_CHERRY, 2.5));
    add(TerrainType::Forest, false, to_deco(atlas::TREE_ORANGE, 2.5));
    add(TerrainType::Forest, false, to_deco(atlas::TREE_APPLE, 2.5));
    add(TerrainType::Forest, false, to_garnish(atlas::G_BUSH, 0.85, 13.0, 9.0, [0.85, 1.0, 0.92]));
    add(TerrainType::Forest, false, to_garnish(atlas::G_MUSH_TAN, 0.8, 16.0, 15.0, [0.92, 0.95, 1.0]));
    add(TerrainType::Forest, false, to_garnish(atlas::G_MUSH_TAN2, 0.75, 16.0, 15.0, [0.9, 0.95, 1.0]));
    add(TerrainType::Forest, false, to_garnish(atlas::G_LEAF_SPRAY, 0.9, 12.0, 13.0, [0.9, 1.0, 0.95]));
    add(TerrainType::Forest, false, to_garnish(atlas::G_PEBBLES, 0.45, 14.0, 7.0, [0.8, 0.85, 0.9]));
    add(TerrainType::Forest, true, to_deco(atlas::CRIT_DEER, 1.4));
    add(TerrainType::Forest, true, to_deco(atlas::CRIT_FOX, 1.1));
    add(TerrainType::Forest, true, to_deco_anim(atlas::CRIT_MONARCH, 0.6, "crit_monarch"));
    add(TerrainType::Forest, true, to_deco(atlas::CRIT_FROG, 0.7));
    add(TerrainType::Forest, true, to_deco(atlas::CRIT_CROW, 0.8));
    // Jungle: potatoes, blueberries, capybaras by the water; frogs, turtles, fruit trees.
    add(TerrainType::TropicalRainforest, false, to_deco(atlas::DECO_POTATO, 1.1));
    add(TerrainType::TropicalRainforest, false, to_deco(atlas::DECO_BLUEBERRY, 1.0));
    add(TerrainType::TropicalRainforest, false, to_deco(atlas::TREE_CHERRY, 2.5));
    add(TerrainType::TropicalRainforest, false, to_deco(atlas::TREE_ORANGE, 2.5));
    add(TerrainType::TropicalRainforest, false, to_deco(atlas::TREE_APPLE, 2.5));
    add(TerrainType::TropicalRainforest, true, to_deco(atlas::CRIT_CAPYBARA, 1.6));
    add(TerrainType::TropicalRainforest, true, to_deco_anim(atlas::CRIT_BUTTERFLY, 0.6, "crit_butterfly"));
    add(TerrainType::TropicalRainforest, true, to_deco(atlas::CRIT_FROG, 0.7));
    add(TerrainType::TropicalRainforest, true, to_deco(atlas::CRIT_TURTLE, 0.6));
    add(TerrainType::TropicalRainforest, false, to_deco_anim(atlas::PROP_WATER_EDGE, 0.8, "prop_water_edge"));
    add(TerrainType::TropicalRainforest, false, to_garnish(atlas::G_BUSH, 0.9, 13.0, 9.0, [0.8, 1.15, 0.9]));
    add(TerrainType::TropicalRainforest, false, to_garnish(atlas::G_LEAF_SPRAY, 0.95, 12.0, 13.0, [0.85, 1.1, 0.9]));
    add(TerrainType::TropicalRainforest, false, to_garnish(atlas::G_MUSH_TAN2, 0.8, 16.0, 15.0, [0.85, 1.05, 0.95]));
    // Desert: scattered stones and rock breaks, driftwood, dry tufts.
    add(TerrainType::Desert, false, to_deco(atlas::DECO_STONE, 0.9));
    add(TerrainType::Desert, false, to_deco(atlas::DECO_ROCK2, 1.1));
    add(TerrainType::Desert, false, to_deco(atlas::PROP_FENCE_STONE, 1.0));
    add(TerrainType::Desert, false, to_garnish(atlas::G_PEBBLES_STONE, 1.1, 16.0, 16.0, [1.1, 1.0, 0.9]));
    add(TerrainType::Desert, false, to_garnish(atlas::G_PEBBLES, 0.55, 14.0, 7.0, [1.05, 1.0, 0.95]));
    add(TerrainType::Desert, false, to_garnish(atlas::G_DRIFTWOOD, 0.8, 32.0, 16.0, [1.1, 1.05, 0.95]));
    // Tundra: frozen stones, penguins, snow FX + frost garnish.
    add(TerrainType::Tundra, false, to_deco(atlas::DECO_ROCK2, 1.0));
    add(TerrainType::Tundra, false, to_deco_anim(atlas::FX_SNOW, 1.5, "fx_snow"));
    add(TerrainType::Tundra, true, to_deco(atlas::CRIT_PENGUIN, 1.0));
    add(TerrainType::Tundra, false, to_garnish(atlas::G_SPARKLE, 0.5, 16.0, 16.0, [1.0, 1.0, 1.15]));
    add(TerrainType::Tundra, false, to_garnish(atlas::G_PEBBLES, 0.5, 13.0, 8.0, [1.05, 1.2, 1.3]));
    // Taiga: dead trees (old-tree variants), foxes, rabbits, snow FX, fruit trees.
    add(TerrainType::Taiga, false, to_deco(atlas::DECO_TREE2, 1.8));
    add(TerrainType::Taiga, false, to_deco(atlas::DECO_STONE, 0.9));
    add(TerrainType::Taiga, false, to_deco(atlas::TREE_CHERRY, 2.5));
    add(TerrainType::Taiga, false, to_deco(atlas::TREE_APPLE, 2.5));
    add(TerrainType::Taiga, false, to_deco_anim(atlas::FX_SNOW, 1.5, "fx_snow"));
    add(TerrainType::Taiga, true, to_deco(atlas::CRIT_FOX, 1.2));
    add(TerrainType::Taiga, true, to_deco(atlas::CRIT_RABBIT, 0.9));
    add(TerrainType::Taiga, true, to_deco(atlas::CRIT_CROW, 0.8));
    add(TerrainType::Taiga, false, to_garnish(atlas::G_MUSH_BLUE, 0.8, 15.0, 13.0, [1.0, 1.05, 1.1]));
    add(TerrainType::Taiga, false, to_garnish(atlas::G_SPARKLE, 0.55, 16.0, 16.0, [0.95, 1.0, 1.1]));
    add(TerrainType::Taiga, false, to_garnish(atlas::G_MUSH_BABY2, 0.75, 12.0, 8.0, [0.95, 1.05, 1.15]));
    // Highlands: stones + violet pebbles, bonfire, clouds.
    add(TerrainType::Mountain, false, to_deco(atlas::DECO_STONE, 1.0));
    add(TerrainType::Mountain, false, to_deco(atlas::DECO_ROCK2, 1.2));
    add(TerrainType::Mountain, false, to_deco_anim(atlas::FX_BONFIRE, 1.5, "fx_bonfire"));
    add(TerrainType::Mountain, false, to_deco_anim(atlas::FX_CLOUDS, 2.0, "fx_clouds"));
    add(TerrainType::Mountain, false, to_garnish(atlas::G_PEBBLES_STONE, 1.2, 16.0, 16.0, [1.0, 1.0, 1.05]));
    add(TerrainType::Mountain, false, to_garnish(atlas::G_MUSH_BABY, 0.55, 13.0, 8.0, [1.0, 1.0, 1.05]));
    // Polluted: scorched/charred ground garnish, toxic splash.
    add(TerrainType::Polluted, false, to_garnish(atlas::G_PEBBLES_BURNT, 0.9, 16.0, 16.0, [0.75, 1.0, 0.8]));
    add(TerrainType::Polluted, false, to_garnish(atlas::G_PEBBLES, 0.6, 13.0, 8.0, [0.72, 0.95, 0.78]));
    add(TerrainType::Polluted, false, to_garnish(atlas::G_DROPLET, 0.6, 16.0, 16.0, [0.6, 1.1, 0.7]));
    // Water: sparkle + droplet decals (tinted blue), so rivers/sea foam.
    add(TerrainType::Water, false, to_garnish(atlas::G_SPARKLE, 0.45, 16.0, 16.0, [0.95, 1.0, 1.1]));
    add(TerrainType::Water, false, to_garnish(atlas::G_DROPLET, 0.55, 16.0, 16.0, [1.15, 1.1, 1.0]));
    // City: paths, benches, wells.
    add(TerrainType::City, false, to_deco(atlas::PROP_PATH_TILE, 1.0));
    add(TerrainType::City, false, to_deco(atlas::BENCH, 1.2));
    add(TerrainType::City, false, to_deco(atlas::PROP_WELL, 1.2));
    add(TerrainType::City, true, to_deco(atlas::CRIT_CROW, 0.8));

    deco.by_terrain = sets;
    deco.built = true;
    let _ = commands;
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

/// Load the floor tile handles from the cropped atlas once the art has
/// streamed in. Polls `SlicedAtlas`; fills `SolidFloorTextures` a single time.
pub fn init_solid_floor_textures(
    mut solid: ResMut<SolidFloorTextures>,
    atlas: Res<crate::slice::SlicedAtlas>,
    mut done: Local<bool>,
) {
    if *done || !solid.by_terrain.is_empty() {
        return;
    }
    // Land tiles all come from the atlas crops that must all be ready.
    let keys = [
        atlas::FLOOR_GRASS, atlas::FLOOR_GRASS_SUMMER2, atlas::FLOOR_GRASS_SPRING,
        atlas::FLOOR_GRASS_SPRING2, atlas::FLOOR_GRASS_FALL, atlas::FLOOR_GRASS_FALL2,
        atlas::FLOOR_SAND, atlas::FLOOR_SNOW, atlas::FLOOR_SNOW2, atlas::FLOOR_STONE,
        atlas::FLOOR_DIRT,
    ];
    if keys.iter().any(|k| !atlas.items.contains_key(*k)) {
        return;
    }
    let get = |k: &str| atlas.items.get(k).expect("checked").clone();
    for terrain in terrains_iter() {
        let variants: Vec<(Handle<Image>, [f32; 3])> = floor_tiles_for(terrain)
            .into_iter()
            .map(|(key, tint)| (get(key), *tint))
            .collect();
        if !variants.is_empty() {
            solid.by_terrain.insert(terrain, variants);
        }
    }
    *done = true;
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

/// Atlas key + tint per terrain variant. Only seamless plain tiles qualify —
/// the EmanuelleDev tilesets are authored seamless, and the sampled crops are
/// flat enough to tile without banding.
fn floor_tiles_for(terrain: TerrainType) -> &'static [(&'static str, [f32; 3])] {
    match terrain {
        // Meadow: dark summer base + bright spring tiles mixed by hash —
        // real art from both sheets, tinted near-neutral.
        TerrainType::Grass => &[
            (atlas::FLOOR_GRASS, [1.0, 1.0, 1.0]),
            (atlas::FLOOR_GRASS_SPRING, [0.95, 1.0, 0.92]),
            (atlas::FLOOR_GRASS_SUMMER2, [0.97, 1.0, 0.93]),
            (atlas::FLOOR_GRASS_SPRING2, [0.9, 1.0, 0.88]),
        ],
        // Dry plain: warm summer greens + the fall sheet's straw-green.
        TerrainType::Grassland => &[
            (atlas::FLOOR_GRASS, [1.05, 1.02, 0.87]),
            (atlas::FLOOR_GRASS_FALL, [1.12, 1.0, 0.82]),
            (atlas::FLOOR_GRASS_FALL2, [1.08, 0.98, 0.84]),
            (atlas::FLOOR_DIRT, [1.0, 1.0, 0.95]),
        ],
        // Deep woods: cooler, darker — spring dark cell + summer.
        TerrainType::Forest => &[
            (atlas::FLOOR_GRASS, [0.68, 0.84, 0.64]),
            (atlas::FLOOR_GRASS_SPRING2, [0.6, 0.82, 0.62]),
            (atlas::FLOOR_GRASS_SPRING, [0.64, 0.8, 0.58]),
        ],
        // Jungle: lush saturated greens.
        TerrainType::TropicalRainforest => &[
            (atlas::FLOOR_GRASS_SPRING, [0.88, 1.08, 0.82]),
            (atlas::FLOOR_GRASS_SPRING2, [0.82, 1.12, 0.78]),
            (atlas::FLOOR_GRASS, [0.95, 1.12, 0.88]),
        ],
        // Dunes: seamless sand + pale dirt mixes.
        TerrainType::Desert => &[
            (atlas::FLOOR_SAND, [1.0, 1.0, 1.0]),
            (atlas::FLOOR_DIRT, [1.05, 0.96, 0.9]),
            (atlas::FLOOR_SAND, [1.07, 0.97, 0.86]),
        ],
        // Snow: white + icy shaded sheet mixes.
        TerrainType::Tundra => &[
            (atlas::FLOOR_SNOW, [1.0, 1.0, 1.0]),
            (atlas::FLOOR_SNOW2, [0.95, 0.98, 1.05]),
        ],
        // Boreal snow: colder blue tint.
        TerrainType::Taiga => &[
            (atlas::FLOOR_SNOW, [0.82, 0.9, 1.08]),
            (atlas::FLOOR_SNOW2, [0.78, 0.88, 1.1]),
            (atlas::FLOOR_SNOW, [0.86, 0.94, 1.12]),
        ],
        // Rocky highlands: warm stone from the Dungeon tileset, greyed so it
        // reads as bare rock.
        TerrainType::Mountain => &[
            (atlas::FLOOR_STONE, [0.66, 0.66, 0.76]),
            (atlas::FLOOR_STONE, [0.6, 0.6, 0.72]),
            (atlas::FLOOR_STONE, [0.78, 0.78, 0.88]),
        ],
        // City streets/procedural: filled by init_city_textures, not here.
        TerrainType::City => &[],
        // Blighted ground: scorched dirt with a sickly green tint.
        TerrainType::Polluted => &[
            (atlas::FLOOR_DIRT, [0.7, 0.85, 0.7]),
            (atlas::FLOOR_STONE, [0.85, 1.0, 0.85]),
            (atlas::FLOOR_DIRT, [0.62, 0.78, 0.65]),
        ],
        TerrainType::Water => &[(atlas::WATER, [1.0, 1.0, 1.0])],
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
    extent: Res<crate::plugins::camera::ViewExtent>,
    props: Res<PropTextures>,
    time: Res<Time>,
    mut next_scan: Local<f64>,
    mut last_px: Local<f32>,
    mut last_py: Local<f32>,
    mut state: ResMut<WorldObjectState>,
) {
    // The cropped prop art streams in over the first few frames; don't spawn
    // world-object visuals until the whole set is ready.
    if !props.ready {
        return;
    }
    // Perf: this scans every replicated world_object row; at planet scale that
    // set can be huge, so throttle it. While the player is moving we rescan at
    // ~7 Hz (objects pop in as you walk), but while standing still we only
    // rescan at ~1 Hz — maturity changes are time-based and infrequent, so the
    // per-frame cost drops to near zero when idle.
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let moved = ((px - *last_px).powi(2) + (py - *last_py).powi(2)).sqrt();
    let interval = if moved > 2.0 { 0.15 } else { 1.0 };
    if time.elapsed_secs_f64() < *next_scan {
        return;
    }
    *next_scan = time.elapsed_secs_f64() + interval;
    *last_px = px;
    *last_py = py;
    let conn_guard = net.conn.lock().unwrap();
    let Some(conn) = conn_guard.as_ref() else { return };
    let max_dist = (RENDER_RADIUS_HEXES + 2.0 * WorldGenConfig::TILE_SIZE)
        .max((extent.half_diagonal * 1.1).min(230.0));

    let now = now_unix_secs();

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    // Rows needing a (re)spawn this pass, sorted near-first and capped so a
    // burst of replication can't create hundreds of sprites in one frame.
    let mut pending: Vec<(f32, u64)> = Vec::new();
    for row in crate::net::gen::WorldObjectTableAccess::world_object(&conn.db).iter() {
        let coord = idlecore_core::grid::TileCoord::from_id(row.hex_id);
        let (hx, hy) = row_world_center(coord.x, coord.y);
        let wx = hx + row.offset_x;
        let wy = hy + row.offset_y;
        if (wx - px).powi(2) + (wy - py).powi(2) > max_dist * max_dist {
            continue;
        }
        seen.insert(row.object_id);

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
        let coord = idlecore_core::grid::TileCoord::from_id(row.hex_id);
        let (hx, hy) = row_world_center(coord.x, coord.y);
        let wx = hx + row.offset_x;
        let wy = hy + row.offset_y;
        let mature = row.mature_at == 0 || now >= row.mature_at;
        // Descriptor: sprite + target height + flip; trees grow sapling->full.
        let flip = row.object_id % 2 == 0;
        let (handle, height, aspect, tint) = match row.kind.as_str() {
            "Grass" => {
                if mature {
                    (
                        &props.grass_mature,
                        kind_height("Grass", true),
                        props.grass_mature_aspect,
                        Color::srgb(0.42, 0.72, 0.38),
                    )
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

/// Build the water tints + the shared water tile once. Replaces the old
/// tinted white square with the actual `Water tile.png` art, recoloured per
/// water class.
pub fn init_water_textures(
    mut water: ResMut<WaterTextures>,
    atlas: Res<crate::slice::SlicedAtlas>,
) {
    if water.white.is_some() {
        return;
    }
    let Some(tile) = atlas.items.get(atlas::WATER).cloned() else {
        return; // water tile not cropped yet — retry next frame
    };
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
    water.white = Some(tile);
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
    /// Spawn radius of the last completed rebuild. The pass re-runs whenever
    /// this changes — zooming in/out while standing still must regrow the
    /// floor ring (the dead zoom band where nothing rendered).
    pub last_spawn_r: Option<f32>,
}

/// Floor tiles spawn within this many slots of the player at the default
/// zoom (≈69 units). Zoomed out, the radius grows to cover the viewport
/// (capped at [`FLOOR_MAX_RADIUS_UNITS`] — tile count stays bounded).
pub const FLOOR_SPAWN_RADIUS_SLOTS: i32 = 16;

/// Absolute cap for the zoom-expanded spawn radius (world units):
/// 260 covers a 2560×1440 window at MIN_ZOOM (half-diag 176 ·1.15 ≈ 202)
/// with margin; ~10k floor sprites worst case.
pub const FLOOR_MAX_RADIUS_UNITS: f32 = 260.0;

/// Live tiles despawn past this radius (hysteresis so border-walking
/// doesn't churn spawns/despawns every slot crossing). Computed from the
/// dynamic spawn radius (+2 slots slack) at update time now.

/// Max floor tiles spawned per rebuild pass; the pass re-runs until the
/// wanted set is complete, so teleports fill in over a few frames instead
/// of hitching once.
const FLOOR_SPAWNS_PER_PASS: usize = 512;

/// World-space radius around the player to show world-object/plant sprites.
const RENDER_RADIUS_HEXES: f32 = 12.0 * WorldGenConfig::TILE_SIZE;

/// Max world-object sprites created per 150 ms scan; spreads replication
/// bursts across frames instead of hitching once.
const SPRITE_SPAWNS_PER_PASS: usize = 32;

/// Spawn/despawn floor tiles around the player: only slots within
/// `FLOOR_SPAWN_RADIUS_SLOTS` exist; everything else is despawned.
pub fn update_world_floor(
    mut commands: Commands,
    streaming_world: Res<StreamingWorldResource>,
    player_transform: Res<PlayerTransform>,
    extent: Res<crate::plugins::camera::ViewExtent>,
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

    let (psx, psy) = player_slot;
    // Zoom-aware radius: the default ring grown to cover the viewport half
    // diagonal (+15% margin) whenever zooming out would outrun it.
    let spawn_r = (FLOOR_SPAWN_RADIUS_SLOTS as f32 * SLOT_SIZE)
        .max((extent.half_diagonal * 1.15).min(FLOOR_MAX_RADIUS_UNITS));
    let spawn_slots = (spawn_r / SLOT_SIZE).ceil() as i32;
    let spawn_r2 = spawn_r * spawn_r;

    // Perf: rebuild only when the player crosses a slot boundary OR the
    // zoom-driven radius changed (zooming while standing still must
    // regrow the floor ring), or when a previous pass hit the spawn cap
    // and left work unfinished.
    if tiles.last_player_slot == Some(player_slot)
        && tiles.last_spawn_r == Some(spawn_r)
    {
        return;
    }

    // Terrain per hex for the nearby chunks — one map built per pass, so the
    // per-slot lookups below are O(1).
    let (hq, hr) = world_pos_to_tile(px, py, WorldGenConfig::TILE_SIZE);
    let (ccq, ccr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);
    // A chunk spans 32 hexes (~554 units); ±2 chunks always covers the
    // spawn radius with margin.
    let mut terrain_of: HashMap<u64, (TerrainType, WaterClass)> = HashMap::new();
    for cq in (ccq - 2)..=(ccq + 2) {
        for cr in (ccr - 2)..=(ccr + 2) {
            let Some(chunk) = streaming_world.chunks.chunks.get(&(cq, cr)) else { continue };
            for cell in &chunk.cells {
                let id = idlecore_core::grid::TileCoord::new(cell.x, cell.y).to_id();
                terrain_of.insert(id, (cell.terrain, cell.water));
            }
        }
    }

    // Shallow-band map: every tile within one step (8-way) of land. Water
    // slots on these tiles render lighter — the sea reads near shore.
    let mut shallow: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for (&id, &(t, _)) in &terrain_of {
        if t == TerrainType::Water {
            continue;
        }
        let tile = idlecore_core::grid::TileCoord::from_id(id);
        for (nx, ny) in tile.neighbors() {
            shallow.insert(idlecore_core::grid::TileCoord::new(nx, ny).to_id());
        }
    }

    // Wanted set: slots in the square around the player, culled to a circle.
    // Sorted near-first so a capped pass fills the view center first.
    let mut wanted: Vec<(f32, i32, i32, TerrainType, WaterClass)> = Vec::new();
    for sx in (psx - spawn_slots)..=(psx + spawn_slots) {
        for sy in (psy - spawn_slots)..=(psy + spawn_slots) {
            let (cx, cy) = slot_center(sx, sy);
            let dx = cx - px;
            let dy = cy - py;
            let d2 = dx * dx + dy * dy;
            if d2 > spawn_r2 {
                continue;
            }
            let (hq, hr) = slot_tile(sx, sy);
            let Some(&(terrain, wat)) =
                terrain_of.get(&idlecore_core::grid::TileCoord::new(hq, hr).to_id())
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
            let base = water
                .by_class
                .get(&wat)
                .or_else(|| water.by_class.values().next())
                .copied()
                .unwrap_or([0.15, 0.36, 0.58]);
            // Living-water: slots on land-neighboring tiles (shallow) render
            // lighter; open water splits into three shimmer tones.
            let tile = slot_tile(*sx, *sy);
            let tint = if shallow.contains(&idlecore_core::grid::TileCoord::new(tile.0, tile.1).to_id()) {
                [base[0] * 1.5, base[1] * 1.35, base[2] * 1.2]
            } else {
                let h = slot_hash(*sx, *sy, 0x5E17_A17D);
                match h % 3 {
                    0 => [base[0] * 0.92, base[1] * 0.95, base[2] * 0.98],
                    1 => base,
                    _ => [base[0] * 1.06, base[1] * 1.05, base[2] * 1.04],
                }
            };
            (white, tint)
        } else if *terrain == TerrainType::City {
            // Procedural urban floor: roads / plazas / building bases.
            let cell = city_cell(cx, cy);
            match cell.kind {
                CityCellKind::Road => {
                    // Roads share the procedural pavement (grey asphalt);
                    // slightly varied tint so the grid reads as streets.
                    let Some(c) = city.as_ref() else { continue };
                    let v = cell.variant as f32;
                    let t = [0.9 + 0.1 * (v % 3.0) / 2.0, 0.9 + 0.1 * (v % 3.0) / 2.0, 0.95];
                    (c.pavement.clone(), t)
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
                    let child_entity = commands.spawn((
                        Name::new("deco"),
                        Sprite {
                            image: d.image.clone(),
                            // Non-square crops keep proportions; tint drives
                            // the palette per terrain (dry/frost/toxic).
                            custom_size: Some(Vec2::new(
                                d.height * scale * d.aspect,
                                d.height * scale,
                            )),
                            color: Color::srgb(d.tint[0], d.tint[1], d.tint[2]),
                            ..default()
                        },
                        bevy::sprite::Anchor::BOTTOM_CENTER,
                        Transform::from_xyz(cx + jx * SLOT_SIZE, dy + SLOT_SIZE * 0.2, 2.0),
                    )).id();
                    if let Some(ref anim_key) = d.anim_key {
                        let frame_keys: Vec<String> = match anim_key.as_str() {
                            "fx_leaves_fall" => (0..6).map(|i| format!("fx_leaves_fall_{}", i)).collect(),
                            "fx_snow" => (0..6).map(|i| format!("fx_snow_{}", i)).collect(),
                            "fx_bonfire" => (0..6).map(|i| format!("fx_bonfire_{}", i)).collect(),
                            "fx_clouds" => (0..9).map(|i| format!("fx_clouds_{}", i)).collect(),
                            "prop_water_edge" => (0..4).map(|i| format!("prop_water_edge_{}", i)).collect(),
                            "crit_butterfly" => (0..7).map(|i| format!("crit_butterfly_{}", i)).collect(),
                            "crit_monarch" => (0..7).map(|i| format!("crit_monarch_{}", i)).collect(),
                            "crit_bee" => (0..4).map(|i| format!("crit_bee_{}", i)).collect(),
                            _ => vec![],
                        };
                        if !frame_keys.is_empty() {
                            commands.entity(child_entity).insert(AnimatedDeco {
                                base_key: anim_key.clone(),
                                frame_keys,
                                frame_index: 0,
                                timer: Timer::from_seconds(0.15, TimerMode::Repeating),
                            });
                        }
                    }
                    commands.entity(entity).add_child(child_entity);
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

    // Despawn tiles beyond the hysteresis radius (spawn radius + slack —
    // both scale with zoom so border-walking never churns).
    let despawn_r2 = (spawn_r + 2.0 * SLOT_SIZE).powi(2);
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
        tiles.last_spawn_r = Some(spawn_r);
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

/// Per-slot visual cache for Stardew plots (tilled dirt + growing crops).
#[derive(Resource, Default)]
pub struct PlotFloorState {
    /// slot → root entity holding the dirt tile sprite.
    pub visuals: HashMap<(i32, i32), Entity>,
    /// slot → encoded visual (wet + crop stage), to respawn only on real change.
    pub stage: HashMap<(i32, i32), (bool, i8)>,
    /// Last raw `HexTile.plots` JSON per hex, so unchanged hexes skip parsing.
    pub hex_raw: HashMap<u64, u64>,
    /// Cached per-hex plot maps (parsed once per hex change).
    pub parsed: HashMap<u64, std::collections::HashMap<String, ClientPlot>>,
}

/// Root entity rendering one slot's plot (dirt tile + crop child).
#[derive(Component)]
pub struct PlotVisual;

/// Mirror of a slot's `PlotState` parsed out of the hex `plots` JSON map.
#[derive(serde::Deserialize, Clone)]
struct ClientPlot {
    #[serde(default)]
    tilled: bool,
    kind: Option<String>,
    #[serde(default)]
    planted_at: u64,
    watered_at: Option<u64>,
    #[serde(default)]
    growth_time: u64,
    #[serde(default)]
    planted_by: Option<String>,
}

impl ClientPlot {
    fn mature_at(&self, watered_at: u64) -> u64 {
        watered_at.saturating_add(self.growth_time.max(1))
    }
}

/// Scan the hexes near the player and render each tilled plot as a dirt tile
/// (dry, or wet once watered) plus a crop at the current growth stage.
///
/// Cost is bounded by the number of *replicated hexes with plots* (not by the
/// whole slot grid): each hex is parsed once per change, and a slot's sprite
/// is respawned only when its visual (wet + crop stage) actually changes.
pub fn update_plot_visuals(
    mut commands: Commands,
    net: Res<crate::net::plugin::Net>,
    player_transform: Res<crate::player::PlayerTransform>,
    time: Res<Time>,
    props: Option<Res<PropTextures>>,
    mut state: ResMut<PlotFloorState>,
    mut next_scan: Local<f64>,
    mut last_px: Local<f32>,
    mut last_py: Local<f32>,
) {
    let Some(props) = props.as_ref() else { return };
    if !props.ready {
        return;
    }
    // Idle-aware throttle: moving ~7 Hz (plots pop in), standing ~1 Hz (only
    // crop-stage maturity changes — cheap, since hexes are cached & skipped).
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let moved = ((px - *last_px).powi(2) + (py - *last_py).powi(2)).sqrt();
    let interval = if moved > 2.0 { 0.15 } else { 1.0 };
    if time.elapsed_secs_f64() < *next_scan {
        return;
    }
    *next_scan = time.elapsed_secs_f64() + interval;
    *last_px = px;
    *last_py = py;

    let now = now_unix_secs();
    let conn_guard = net.conn.lock().unwrap();
    let Some(conn) = conn_guard.as_ref() else { return };
    let (hq, hr) = world_pos_to_tile(px, py, WorldGenConfig::TILE_SIZE);
    const MAX_DIST: f32 = 8.0;

    // Every plot slot still in range this pass (for stale cleanup).
    let mut seen: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();

    for row in crate::net::gen::HexTileTableAccess::hex_tile(&conn.db).iter() {
        let dq = (row.hex_q - hq).abs() as f32;
        let dr = (row.hex_r - hr).abs() as f32;
        let ds = ((row.hex_q + row.hex_r) - (hq + hr)).abs() as f32;
        if dq.max(dr).max(ds) > MAX_DIST {
            continue;
        }
        let plots_json = row.plots.as_deref();
        let empty = plots_json.map_or(true, str::is_empty);
        if empty {
            // Hex no longer has plots (e.g. scrolled out / all plots cleared).
            state.hex_raw.remove(&row.hex_id);
            state.parsed.remove(&row.hex_id);
            continue;
        }
        let raw_hash = fnv_hash(plots_json.unwrap_or(""));
        // Parse the hex's plot map only once until the JSON actually changes.
        if state.hex_raw.get(&row.hex_id) != Some(&raw_hash) {
            match serde_json::from_str::<std::collections::HashMap<String, ClientPlot>>(plots_json.unwrap_or("")) {
                Ok(map) => {
                    state.hex_raw.insert(row.hex_id, raw_hash);
                    state.parsed.insert(row.hex_id, map);
                }
                Err(_) => {
                    state.hex_raw.remove(&row.hex_id);
                    state.parsed.remove(&row.hex_id);
                    continue;
                }
            }
        }
        // Materialise the desired visuals (drops the `map` borrow so `state`
        // can be mutated below without aliasing).
        let mut desired: Vec<(i32, i32, (bool, i8))> = Vec::new();
        if let Some(map) = state.parsed.get(&row.hex_id) {
            for (key, plot) in map {
                if let Some((sx, sy)) = parse_slot_key(key) {
                    if let Some(vis) = plot_stage(plot, now) {
                        desired.push((sx, sy, vis));
                    }
                }
            }
        }

        for (sx, sy, vis) in desired {
            seen.insert((sx, sy));
            if state.stage.get(&(sx, sy)) == Some(&vis) {
                continue; // unchanged — no respawn
            }
            let (cx, cy) = slot_center(sx, sy);
            if let Some(root) = state.visuals.remove(&(sx, sy)) {
                commands.entity(root).despawn();
            }
            if let Some(entity) = spawn_slot_plot(&mut commands, props, &vis, cx, cy) {
                state.visuals.insert((sx, sy), entity);
                state.stage.insert((sx, sy), vis);
            }
        }
    }

    // Despawn plots that left the scan radius or belong to a now-empty hex.
    let stale: Vec<(i32, i32)> = state
        .visuals
        .keys()
        .chain(state.stage.keys())
        .filter(|k| !seen.contains(k))
        .copied()
        .collect();
    for k in stale {
        if let Some(e) = state.visuals.remove(&k) {
            commands.entity(e).despawn();
        }
        state.stage.remove(&k);
    }
}

/// The encoded visual for a plot: `(wet, crop_stage)` where `crop_stage` is
/// -1 = no crop (bare dirt), else 0/1/2 = young / mid / mature. `None` when the
/// plot isn't tilled (nothing to draw).
fn plot_stage(plot: &ClientPlot, now: u64) -> Option<(bool, i8)> {
    if !plot.tilled {
        return None;
    }
    let wet = plot.watered_at.is_some();
    let mut stage: i8 = -1;
    if let Some(wa) = plot.watered_at {
        if plot.kind.is_some() {
            let growth = plot.growth_time.max(1) as f64;
            let progress = now.saturating_sub(wa) as f64 / growth;
            stage = if progress >= 1.0 {
                2
            } else if progress >= 0.5 {
                1
            } else {
                0
            };
        }
    }
    Some((wet, stage))
}

/// Parse a `"{sx}:{sy}"` slot key back into coordinates.
fn parse_slot_key(key: &str) -> Option<(i32, i32)> {
    let (a, b) = key.split_once(':')?;
    let sx = a.parse::<i32>().ok()?;
    let sy = b.parse::<i32>().ok()?;
    Some((sx, sy))
}

/// Fast 64-bit FNV-1a hash, used only to detect plot-JSON changes.
fn fnv_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Spawn the dirt tile (+ wet overlay via texture + crop child) for one slot.
fn spawn_slot_plot(
    commands: &mut Commands,
    props: &PropTextures,
    vis: &(bool, i8),
    cx: f32,
    cy: f32,
) -> Option<Entity> {
    let (wet, stage) = *vis;
    let dirt = if wet {
        props.plot_wet.clone()
    } else {
        props.plot_tilled.clone()
    };
    let crop: Option<Handle<Image>> = if stage >= 0 {
        Some(props.crop_stages[stage as usize].clone())
    } else {
        None
    };

    let root = commands
        .spawn((
            Name::new(format!("plot-{cx:.0}-{cy:.0}")),
            PlotVisual,
            Sprite {
                image: dirt,
                custom_size: Some(Vec2::splat(SLOT_SIZE)),
                ..default()
            },
            Transform::from_xyz(cx, cy, floor_depth(cy)),
            Visibility::Visible,
        ))
        .id();

    if let Some(image) = crop {
        commands.entity(root).with_child((
            Name::new("plot-crop"),
            Sprite {
                image,
                custom_size: Some(Vec2::splat(SLOT_SIZE * 0.9)),
                ..default()
            },
            Transform::from_xyz(0.0, SLOT_SIZE * 0.18, prop_depth(cy) + 0.5),
            Visibility::Visible,
        ));
    }
    Some(root)
}

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
    mut last_px: Local<f32>,
    mut last_py: Local<f32>,
    mut state: ResMut<FloorPlantState>,
) {
    // Perf: full hex_tile scan — while moving rescan at ~7 Hz, while standing
    // still at ~1 Hz (only maturity/eco changes matter when idle).
    let px = player_transform.translation.x;
    let py = player_transform.translation.y;
    let moved = ((px - *last_px).powi(2) + (py - *last_py).powi(2)).sqrt();
    let interval = if moved > 2.0 { 0.15 } else { 1.0 };
    if time.elapsed_secs_f64() < *next_scan {
        return;
    }
    *next_scan = time.elapsed_secs_f64() + interval;
    *last_px = px;
    *last_py = py;
    let conn_guard = net.conn.lock().unwrap();
    let Some(conn) = conn_guard.as_ref() else { return };

    let (hq, hr) = world_pos_to_tile(px, py, WorldGenConfig::TILE_SIZE);
    // Plant/pollution discs are small; only build them for hexes close to
    // the player (8 hexes ≈ 140 units) so far replicated rows stay data-only.
    let max_dist = 8.0f32;

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    // Hexes whose plant/pollution visual needs (re)building this pass.
    let mut plant_pending: Vec<(f32, u64)> = Vec::new();
    let now = now_unix_secs();
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
    idlecore_core::grid::WorldGrid::tile_center(q, r, WorldGenConfig::TILE_SIZE)
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
            for (key, tint) in variants {
                // Every key must be a real atlas constant (no leftover file
                // paths) and every tint must stay a plausible shade.
                assert!(!key.contains('/'), "{terrain:?} leaked a file path: {key}");
                assert!(
                    matches!(
                        *key,
                        atlas::FLOOR_GRASS
                            | atlas::FLOOR_GRASS_SUMMER2
                            | atlas::FLOOR_GRASS_SPRING
                            | atlas::FLOOR_GRASS_SPRING2
                            | atlas::FLOOR_GRASS_FALL
                            | atlas::FLOOR_GRASS_FALL2
                            | atlas::FLOOR_SAND
                            | atlas::FLOOR_SNOW
                            | atlas::FLOOR_SNOW2
                            | atlas::FLOOR_STONE
                            | atlas::FLOOR_DIRT
                            | atlas::WATER
                    ),
                    "{terrain:?} unknown floor key {key}"
                );
                assert!(tint.iter().all(|c| (0.5..=1.3).contains(c)));
            }
            // City streets are procedurally generated (no atlas floor), every
            // other land terrain needs art to paint.
            if terrain != TerrainType::City {
                assert!(variants.len() >= 1, "{terrain:?} needs >= 1 floor variant");
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