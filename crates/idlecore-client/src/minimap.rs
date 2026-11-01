//! Minimap system — Minecraft-style fog-of-war exploration map.
//!
//! Features:
//! - Player-centered camera (player marker stays fixed at center)
//! - Current-vision fog of war (terrain hides when player moves away)
//! - Zoom levels via mouse wheel (Local, Area, World)
//! - Rotation modes via N key (NorthUp, PlayerUp)
//! - Waypoints via right-click on minimap
//! - Navigation markers (POI, objectives)
//! - Toggle expanded/compact with M key
//! - Toggle visibility with M key (when not expanded)

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::math::Rot2;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::ImageNode;
use idlecore_core::grid::{world_pos_to_tile, TileCoord};
use idlecore_core::grid::WorldGrid;
use idlecore_core::minimap::{map_pixel_to_world, world_to_map_pixel, RotationMode};
use idlecore_core::world_gen::{WorldCell, WorldGenConfig};
use std::collections::{HashMap, HashSet};

use crate::plugins::world::StreamingWorldResource;
use crate::player::Player;
use crate::fps_counter::FpsText;
use crate::time_ext::Instant;

use crate::net::plugin::Net;

// ============================================================================
// Constants
// ============================================================================

const COMPACT_SIZE: f32 = 200.0;
const EXPANDED_SIZE: f32 = 400.0;
const MINIMAP_PADDING: f32 = 10.0;
const HEX_SIZE: f32 = WorldGenConfig::TILE_SIZE;

// ============================================================================
// Zoom
// ============================================================================

/// Zoom level controlling pixel density on the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinimapZoom {
    #[default]
    Local,
    Area,
    World,
    /// First added zoom-in level (2× local).
    Close,
    /// Second added zoom-in level (4× local).
    Max,
}

impl MinimapZoom {
    /// Pixels per world unit. Tuned for hex radius 10 (server scale): the
    /// World level shows a ~5300-unit region (~300+ hexes) of the planet
    /// in the 200px minimap; at 1:100 scale no zoom level fits the globe.
    pub fn pixel_scale(&self) -> f32 {
        match self {
            MinimapZoom::Local => 0.15,
            MinimapZoom::Area => 0.075,
            MinimapZoom::World => 0.0375,
            MinimapZoom::Close => 0.3,
            MinimapZoom::Max => 0.6,
        }
    }

    pub fn zoom_in(&self) -> Option<Self> {
        match self {
            MinimapZoom::Local => Some(MinimapZoom::Close),
            MinimapZoom::Area => Some(MinimapZoom::Local),
            MinimapZoom::World => Some(MinimapZoom::Area),
            MinimapZoom::Close => Some(MinimapZoom::Max),
            MinimapZoom::Max => None,
        }
    }

    pub fn zoom_out(&self) -> Option<Self> {
        match self {
            MinimapZoom::Local => Some(MinimapZoom::Area),
            MinimapZoom::Area => Some(MinimapZoom::World),
            MinimapZoom::World => None,
            MinimapZoom::Close => Some(MinimapZoom::Local),
            MinimapZoom::Max => Some(MinimapZoom::Close),
        }
    }

}

const SQRT_3: f32 = 1.7320508075688772;

// ============================================================================
// Configuration & State Resources
// ============================================================================

/// Runtime minimap state.
#[derive(Resource)]
pub struct MinimapState {
    pub zoom: MinimapZoom,
    pub rotation: RotationMode,
    pub expanded: bool,
    pub player_pos: Option<Vec2>,
    pub facing_angle: f32,
    /// Click-to-teleport target: axial (q, r) of the last left-clicked hex.
    pub selected_hex: Option<(i32, i32)>,
    /// Pixel position (minimap-space) at which to draw the selection ring.
    pub selected_px: Option<(f32, f32)>,
    /// Last rendered selection ring position; respawn is skipped when equal.
    pub last_selection_px: Option<(f32, f32)>,
    /// Key of the last tile render; `render_visible_tiles` skips a frame when
    /// nothing about the view (position/zoom/rotation/discovery/streaming)
    /// changed, so standing still costs nothing.
    pub last_tiles_key: Option<(f32, f32, f32, f32, u8, bool, usize, usize, u64)>,
    /// True while the merged-terrain bitmap replaces the per-hex tile nodes
    /// (Area/World zooms). Toggles node despawn/respawn on the transition.
    pub merged_active: bool,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            zoom: MinimapZoom::Local,
            rotation: RotationMode::NorthUp,
            expanded: false,
            player_pos: None,
            facing_angle: 0.0,
            selected_hex: None,
            selected_px: None,
            last_selection_px: None,
            last_tiles_key: None,
            merged_active: false,
        }
    }
}

impl MinimapState {
    /// Current minimap size in pixels.
    pub fn mm_size(&self) -> f32 {
        if self.expanded { EXPANDED_SIZE } else { COMPACT_SIZE }
    }

    /// Pixels per world unit at current zoom.
    pub fn pixel_scale(&self) -> f32 {
        self.zoom.pixel_scale()
    }

    /// Render radius (vision + soft border) in world units, scaled to
    /// minimap pixels. Vision is ~1.5 hexes (hex radius 10).
    pub fn render_radius_px(&self) -> f32 {
        (15.0 + 3.0) * self.pixel_scale()
    }
}

/// One merged "zoomed-out" map image (Spec 025 fix): the per-hex tile grid
/// can't paint Area/World zooms (thousands of 3px nodes, sub-pixel hexes), so
/// far zooms render the streamed terrain into a single texture instead.
#[derive(Resource, Default)]
pub struct MergedMapImage {
    pub handle: Option<Handle<Image>>,
    /// Sample-grid size the texture was built for (w, h).
    pub grid: u32,
}

#[derive(Component)]
pub struct MergedMapNode;

/// Above this tile count a zoom level uses the merged bitmap instead of
/// per-tile UI nodes. Local ≈ 5.9k tiles < budget; Area and the expanded
/// (M) Local ≈ 23.5k flip to the bitmap; Close ≈ 1.5k and Max ≈ 1.0k stay
/// nodes. (REV10 square grid: cells are 17.32 wu on a side, ~3.5× denser
/// than the old hex pitch, so the budget scaled with the cell count.)
const FAR_TILES_BUDGET: usize = 12_000;

/// Estimated explored-hex count the visible minimap box would need as
/// per-hex UI tiles. Pure so the budget split is unit-testable.
fn estimated_tile_count(mm_size: f32, pixel_scale: f32) -> usize {
    let half_w = mm_size * 0.5 / pixel_scale; // world units (split across)
    let half_h = mm_size * 0.5 / pixel_scale;
    let tile = WorldGenConfig::TILE_SIZE;
    ((2.0 * half_w / tile) * (2.0 * half_h / tile)) as usize
}

/// Image row → the map-space y it samples. Bevy `Image` row 0 paints the
/// TOP of the node and map space is y-down, so both grow together — no
/// additional flip. A mistake here mirrors the zoomed-out map N/S (the
/// "top and bottom inverted" report).
fn merged_row_y(ry: u32, step: f32) -> f32 {
    (ry as f32 + 0.5) * step
}

/// Paint the zoomed-out map: one texture covering the whole minimap box,
/// refreshed only on key change (~10 Hz max), sampling the streamed chunk
/// cells through the same pixel↔world math as the tiles. Fog semantics:
/// explored hexes render bright, streamed-but-unexplored dim, unstreamed
/// transparent (the box edge around the prefetch ring).
#[allow(clippy::too_many_arguments)]
fn render_merged_map(
    commands: &mut Commands,
    minimap_state: &mut MinimapState,
    streaming_world: &StreamingWorldResource,
    explored: &ExploredHexes,
    map_content_entity: Entity,
    merged: &mut MergedMapImage,
    images: &mut Assets<Image>,
    pixel_scale: f32,
    player_pos: Vec2,
) {
    let mm = minimap_state.mm_size();
    // Sample every 2 px — ~10k samples for the compact box, ~40k expanded.
    let step = 2.0_f32.max((mm / 128.0).ceil());
    let grid = (mm / step).ceil() as u32;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let mm_center = mm / 2.0;

    let mut pixels = vec![0u8; (grid * grid * 4) as usize];
    for ry in 0..grid {
        for rx in 0..grid {
            // Sample each grid cell's center. Bevy Image row 0 paints the
            // TOP of the node, so row ry samples map y = ry·step (map space
            // is y-down — the same convention the tile nodes and markers
            // share). A flip here mirrors the whole zoomed-out map N/S.
            let sample_x = (rx as f32 + 0.5) * step;
            let sample_y = merged_row_y(ry, step);
            let (world_x, world_y) = map_pixel_to_world(
                (sample_x, sample_y),
                (player_pos.x, player_pos.y),
                pixel_scale,
                mm_center,
                rotation,
            );
            let (q, r) =
                idlecore_core::grid::world_pos_to_tile(world_x, world_y, WorldGenConfig::TILE_SIZE);
            let Some(cell) = streaming_world.chunks.get_cell(q, r) else {
                continue;
            };
            let hex_id = TileCoord::new(q, r).to_id();
            // Fog: visited hexes paint full terrain; streamed-but-unexplored
            // stays a dim silhouette.
            let visited = explored.explored.contains_key(&hex_id);
            let alpha: f32 = if visited { 255.0 } else { 80.0 };
            let dim = if visited { 1.0 } else { 0.5 };
            let c = terrain_color(&cell.terrain).to_srgba();
            let idx = ((ry * grid + rx) * 4) as usize;
            pixels[idx] = (c.red * 255.0 * dim) as u8;
            pixels[idx + 1] = (c.green * 255.0 * dim) as u8;
            pixels[idx + 2] = (c.blue * 255.0 * dim) as u8;
            pixels[idx + 3] = alpha as u8;
        }
    }
    merged_texture(merged, images, grid, pixels, commands, map_content_entity, mm);
}

/// Replace the merged-map texture (rebuild on size change) and show its
/// ImageNode as the bottom layer of the map content.
fn merged_texture(
    merged: &mut MergedMapImage,
    images: &mut Assets<Image>,
    grid: u32,
    pixels: Vec<u8>,
    commands: &mut Commands,
    map_content_entity: Entity,
    mm: f32,
) {
    let image = Image::new(
        Extent3d { width: grid, height: grid, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    if merged.grid != grid || merged.handle.is_none() {
        if let Some(old) = merged.handle.take() {
            images.remove(&old);
        }
        let handle = images.add(image);
        commands
            .spawn((
                Name::new("merged-map"),
                MergedMapNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(mm),
                    height: Val::Px(mm),
                    ..default()
                },
                ImageNode {
                    image: handle.clone(),
                    ..default()
                },
            ))
            .insert(ChildOf(map_content_entity));
        merged.handle = Some(handle);
        merged.grid = grid;
    } else if let Some(handle) = merged.handle.clone() {
        if let Some(mut existing) = images.get_mut(&handle) {
            *existing = image;
        }
    }
}




/// Track of placed waypoints.
#[derive(Resource, Default)]
pub struct MinimapWaypoints {
    pub waypoints: Vec<Waypoint>,
    pub next_id: u64,
}

#[derive(Debug, Clone)]
pub struct Waypoint {
    pub id: u64,
    pub position: Vec2,
}

/// Navigation POI markers (objectives, landmarks, etc.).
#[derive(Resource, Default)]
pub struct MinimapMarkers {
    pub markers: Vec<NavMarker>,
}

#[derive(Debug, Clone)]
pub struct NavMarker {
    pub position: Vec2,
    pub color: Color,
}

/// Cached image handles for minimap textures.
#[derive(Resource)]
pub struct MinimapAssets {
    pub dot_texture: Handle<Image>,
    /// Cached hexagon tile textures: (width_px, height_px, terrain) → image.
    pub hex_tiles: HashMap<(u32, u32, idlecore_core::terrain::TerrainType), Handle<Image>>,
}

/// Track tile entities for proper despawn lifecycle.
#[derive(Resource, Default)]
pub struct HexEntityMap {
    pub hex_entities: HashMap<u64, Entity>,
}

/// Data needed to render an explored minimap tile even after its chunk is unloaded.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ExploredCell {
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: idlecore_core::terrain::TerrainType,
}

/// Tracks which hexes have been discovered by the player (persistent fog-of-war).
#[derive(Resource, Default)]
pub struct ExploredHexes {
    pub explored: HashMap<u64, ExploredCell>,
}

// ============================================================================
// Discovery persistence (logout → next launch restores the fog-of-war map)
// ============================================================================

/// Disk path for one wallet's discovered-hex cache (native only; on wasm the
/// discovery set is kept in memory since there is no filesystem).
#[cfg(not(target_arch = "wasm32"))]
fn explored_path(address: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(format!(".idlebot_discovered_{address}.json"))
}

/// Persist discovered hexes for `address` (best effort, like the token cache).
/// No-op on wasm (no filesystem); discoveries live in memory for the session.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
pub fn save_explored(address: &str, explored: &ExploredHexes) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(json) = serde_json::to_string(&explored.explored) {
            let _ = std::fs::write(explored_path(address), json);
        }
    }
}

/// Load previously discovered hexes for `address`, if any.
#[cfg(not(target_arch = "wasm32"))]
fn load_explored(address: &str) -> Option<HashMap<u64, ExploredCell>> {
    let text = std::fs::read_to_string(explored_path(address)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Restore the persisted discovery set once the wallet binds (once per
/// session). Merge-only: fresh discoveries this session always win.
/// Native only — on wasm there is no filesystem to restore from.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables, unused_mut))]
pub fn restore_explored_on_login(
    net: Res<crate::net::plugin::Net>,
    mut explored_hexes: ResMut<ExploredHexes>,
    mut loaded_for: Local<Option<String>>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(address) = net.address.clone() else { return };
        if *loaded_for == Some(address.clone()) {
            return;
        }
        *loaded_for = Some(address.clone());
        let Some(saved) = load_explored(&address) else { return };
        let before = explored_hexes.explored.len();
        for (hex_id, cell) in saved {
            explored_hexes.explored.entry(hex_id).or_insert(cell);
        }
        let restored = explored_hexes.explored.len() - before;
        if restored > 0 {
            info!("restored {restored} discovered tiles for {address}");
        }
    }
}

/// Autosave discoveries every 10 s while exploring, so logout or a crash
/// keeps the fog-of-war map for the next session.
pub fn autosave_explored(
    net: Res<crate::net::plugin::Net>,
    explored_hexes: Res<ExploredHexes>,
    time: Res<Time>,
    mut next_save: Local<f64>,
    mut last_count: Local<usize>,
) {
    let now = time.elapsed_secs_f64();
    if now < *next_save || explored_hexes.explored.len() == *last_count {
        return;
    }
    *next_save = now + 10.0;
    let Some(address) = net.address.as_ref() else { return };
    save_explored(address, &explored_hexes);
    *last_count = explored_hexes.explored.len();
}

/// Final flush of discovered tiles when the game quits (logout), closing the
/// gap the periodic autosave would otherwise leave behind.
pub fn save_explored_on_exit(
    net: Res<crate::net::plugin::Net>,
    explored_hexes: Res<ExploredHexes>,
    mut exit_reader: MessageReader<bevy::app::AppExit>,
) {
    for _ in exit_reader.read() {
        if let Some(address) = net.address.as_ref() {
            save_explored(address, &explored_hexes);
        }
    }
}

/// Track waypoint entities for proper despawn lifecycle.
#[derive(Resource, Default)]
pub struct WaypointEntityMap {
    pub entities: HashMap<u64, Entity>,
}

/// Track the last streamed player hex (avoid redundant chunk operations).
#[derive(Resource, Default)]
pub struct ChunkLoadState {
    pub last_chunk: Option<(i32, i32)>,
}

// ============================================================================
// Components
// ============================================================================

#[derive(Component)]
pub struct MinimapRoot;

/// Container for all map elements (tiles, waypoints, markers).
#[derive(Component)]
pub struct MapContent;

/// Player direction marker at the center of the minimap.
#[derive(Component)]
pub struct PlayerArrow;

/// A single hex tile rendered in the minimap.
#[derive(Component)]
pub struct MapTileNode;

/// A waypoint marker node.
#[derive(Component)]
pub struct WaypointNode;

/// A navigation marker node.
#[derive(Component)]
pub struct NavMarkerNode;

/// A diamond marker for a placed capital (Spec 023); entity map keys it by
/// capital hex id.
#[derive(Component)]
pub struct CapitalMarkerNode;

/// Track capital-marker entities by capital hex id for in-place updates.
#[derive(Resource, Default)]
pub struct CapitalMarkerEntityMap {
    pub entities: HashMap<u64, Entity>,
}

/// A dot representing a remote player on the minimap.
#[derive(Component)]
pub struct RemotePlayerDot {
    pub address: String,
}

/// Selection ring marking the click-to-teleport target hex.
#[derive(Component)]
pub struct SelectionMarker;

// ============================================================================
// Texture Generation
// ============================================================================

/// Create a fog overlay texture: circular gradient transparent at center,
/// opaque at edges, with a soft transition at the vision boundary.

/// Create a triangle texture pointing upward, for the player marker.
fn create_arrow_image(size: u32, color: Color) -> Image {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    let srgba = color.to_srgba();
    let c = [
        (srgba.red * 255.0) as u8,
        (srgba.green * 255.0) as u8,
        (srgba.blue * 255.0) as u8,
        255u8,
    ];

    // Triangle vertices (pointing up)
    let ax = s * 0.5;
    let ay = s * 0.25;
    let bx = s * 0.2;
    let by = s * 0.8;
    let cx = s * 0.8;
    let cy = s * 0.8;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32;
            let py = y as f32;
            let idx = ((y * size + x) * 4) as usize;

            let d1 = (px - cx) * (ay - cy) - (py - cy) * (ax - cx);
            let d2 = (px - ax) * (by - ay) - (py - ay) * (bx - ax);
            let d3 = (px - bx) * (cy - by) - (py - by) * (cx - bx);

            let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;

            if !(has_neg && has_pos) {
                pixels[idx] = c[0];
                pixels[idx + 1] = c[1];
                pixels[idx + 2] = c[2];
                pixels[idx + 3] = c[3];
            }
        }
    }

    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Convert a TerrainType to a Bevy Color.
fn terrain_color(terrain: &idlecore_core::terrain::TerrainType) -> Color {
    let [r, g, b] = terrain.minimap_color();
    Color::srgb(r, g, b)
}

/// Create a filled hexagon texture with a ±2px transparent padding, so
/// adjacent hex tiles on the minimap fit edge-to-edge with no gaps.
fn create_square_tile_image(width: u32, height: u32, color: Color) -> Image {
    let pad = 2u32;
    let cw = width + pad * 2;
    let ch = height + pad * 2;
    let mut pixels = vec![0u8; (cw * ch * 4) as usize];
    let srgba = color.to_srgba();
    let px = (srgba.red * 255.0) as u8;
    let py = (srgba.green * 255.0) as u8;
    let pz = (srgba.blue * 255.0) as u8;
    let alpha = (srgba.alpha * 255.0) as u8;

    // Square tile: the full interior rect, alpha only in the padding ring.
    for y in 0..ch {
        for x in 0..cw {
            let idx = ((y * cw + x) * 4) as usize;
            let inside = x >= pad && x < cw - pad && y >= pad && y < ch - pad;
            if inside {
                pixels[idx] = px;
                pixels[idx + 1] = py;
                pixels[idx + 2] = pz;
                pixels[idx + 3] = alpha;
            }
        }
    }

    Image::new(
        Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Get (or create) a square tile texture matching the current zoom, so tile
/// node sizes and textures stay consistent when the minimap zooms.
fn hex_tile_handle(
    minimap_assets: &mut MinimapAssets,
    images: &mut Assets<Image>,
    width: u32,
    height: u32,
    terrain: idlecore_core::terrain::TerrainType,
) -> Handle<Image> {
    let key = (width, height, terrain);
    if let Some(handle) = minimap_assets.hex_tiles.get(&key) {
        return handle.clone();
    }
    let handle = images.add(create_square_tile_image(width, height, terrain_color(&terrain)));
    minimap_assets.hex_tiles.insert(key, handle.clone());
    handle
}

/// Get (or create) a fog-colored hexagon tile texture for the current zoom.

/// Create a filled-circle texture for remote player dots.
fn create_dot_image(size: u32, color: Color) -> Image {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let srgba = color.to_srgba();
    let cr = (srgba.red * 255.0) as u8;
    let cg = (srgba.green * 255.0) as u8;
    let cb = (srgba.blue * 255.0) as u8;
    let ca = (srgba.alpha * 255.0) as u8;
    let r = size as f32 / 2.0 - 0.5;
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            if dx * dx + dy * dy <= r * r {
                let idx = ((y * size + x) * 4) as usize;
                pixels[idx] = cr;
                pixels[idx + 1] = cg;
                pixels[idx + 2] = cb;
                pixels[idx + 3] = ca;
            }
        }
    }
    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

// ============================================================================
// Startup / Spawn
// ============================================================================

/// Spawn the minimap UI at the top-right corner of the screen.
pub fn spawn_minimap_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    let font_clone = font.clone();

    let arrow_img = create_arrow_image(16, Color::srgba(0.2, 0.9, 1.0, 0.95));
    let arrow_texture = images.add(arrow_img);
    let dot_img = create_dot_image(16, Color::srgba(0.95, 0.55, 0.2, 0.95));
    let dot_texture = images.add(dot_img);

    commands.insert_resource(MinimapAssets {
        dot_texture: dot_texture.clone(),
        hex_tiles: HashMap::new(),
    });

    let mm_size = COMPACT_SIZE;
    let mm_center = mm_size / 2.0;

    commands.spawn((
        Name::new("minimap-ui"),
        MinimapRoot,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(MINIMAP_PADDING),
            top: Val::Px(MINIMAP_PADDING),
            width: Val::Px(mm_size),
            height: Val::Px(mm_size),
            overflow: Overflow::clip(),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
    ))
    .with_children(|parent| {
        // Background
        parent.spawn((
            Name::new("minimap-bg"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.065, 0.95)),
        ));

        // Map content container (parent of tile/waypoint/marker nodes)
        parent.spawn((
            Name::new("map-content"),
            MapContent,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            UiTransform::IDENTITY,
        ));


        // Player marker (fixed at center)
        parent.spawn((
            Name::new("player-marker"),
            PlayerArrow,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                left: Val::Px(mm_center - 8.0),
                top: Val::Px(mm_center - 8.0),
                ..default()
            },
            UiTransform::IDENTITY,
            ImageNode::new(arrow_texture),
        ));

        // Coords label (bottom-left)
        parent.spawn((
            Name::new("coords-label"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                bottom: Val::Px(2.0),
                ..default()
            },
        ))
        .with_child((
            Text::default(),
            TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(9.0), ..default() },
            TextColor(Color::srgba(0.7, 0.8, 1.0, 1.0)),
        ))
        .with_child(TextSpan::new("0, 0"));

        // Zoom indicator (top-left)
        parent.spawn((
            Name::new("zoom-indicator"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                top: Val::Px(2.0),
                ..default()
            },
        ))
        .with_child((
            Text::default(),
            TextFont { font: FontSource::Handle(font), font_size: FontSize::Px(9.0), ..default() },
            TextColor(Color::srgba(0.7, 0.8, 1.0, 1.0)),
        ))
        .with_child(TextSpan::new("Zoom: Local"));

        // FPS counter — bottom-right of minimap
        parent.spawn((
            Name::new("minimap-fps"),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(4.0),
                bottom: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|fps_parent| {
            fps_parent.spawn((
                Name::new("minimap-fps-text"),
                FpsText,
                Text::new("FPS: 0"),
                TextFont {
                    font: FontSource::Handle(font_clone),
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 1.0, 0.8)),
            ));
        });
    });
}

// ============================================================================
// Player State Sync
// ============================================================================

/// Sync player position and facing into MinimapState.
pub fn sync_player_state(
    player_query: Query<&Transform, With<Player>>,
    orientation: Res<crate::player::PlayerOrientation>,
    mut minimap_state: ResMut<MinimapState>,
) {
    // Perf: only write when the value changed, so downstream systems that
    // run off `ResMut::is_changed` (resize, render gates) skip idle frames.
    let new_pos = player_query
        .iter()
        .next()
        .map(|transform| Vec2::new(transform.translation.x, transform.translation.y));
    if new_pos != minimap_state.player_pos {
        minimap_state.player_pos = new_pos;
    }
    if orientation.facing_angle != minimap_state.facing_angle {
        minimap_state.facing_angle = orientation.facing_angle;
    }
}

// ============================================================================
// Chunk Loading
// ============================================================================

/// Load/unload world chunks around the player position.
pub fn load_nearby_chunks(
    mut streaming_world: ResMut<StreamingWorldResource>,
    minimap_state: Res<MinimapState>,
    mut chunk_state: ResMut<ChunkLoadState>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };

    let (q, r) = world_pos_to_tile(player_pos.x, player_pos.y, HEX_SIZE);
    let chunk_size = WorldGenConfig::CHUNK_SIZE;

    // Perf: re-stream once the player has moved half a chunk (16 hexes) from
    // the last center — frequent enough that the cache keeps up with the
    // render radius, rare enough to be cheap (generation is idempotent).
    let re_stream = match chunk_state.last_chunk {
        None => true,
        Some((cq, cr)) => (q - cq).abs() >= chunk_size / 2 || (r - cr).abs() >= chunk_size / 2,
    };
    if !re_stream {
        return;
    }

    let config = streaming_world.config;
    streaming_world.chunks.stream_around(&config, q, r);

    chunk_state.last_chunk = Some((q, r));
}

// ============================================================================
// Input Handling
// ============================================================================

/// Handle minimap input: zoom (mouse wheel), rotation (N), expand (M),
/// waypoints (right-click). Left-click selects a hex for teleporting — on
/// the minimap itself or anywhere on the world floor — and a double-click
/// on the world floor fires the teleport immediately.
#[allow(clippy::too_many_arguments)]
pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut minimap_state: ResMut<MinimapState>,
    mut minimap_waypoints: ResMut<MinimapWaypoints>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    widgets: Query<&Interaction>,
    mut net: ResMut<crate::net::plugin::Net>,
    mut latency: ResMut<crate::net::plugin::ServerLatency>,
    mut zoom_budget: Local<f32>,
    mut last_click: Local<Option<(Instant, Vec2)>>,
) {
    if keyboard.just_pressed(KeyCode::KeyN) {
        minimap_state.rotation = minimap_state.rotation.toggle();
    }

    if keyboard.just_pressed(KeyCode::KeyM) {
        minimap_state.expanded = !minimap_state.expanded;
    }

    // Spec 009 T1.12: +/- keyboard zoom.
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        if let Some(new_zoom) = minimap_state.zoom.zoom_in() {
            minimap_state.zoom = new_zoom;
        }
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if let Some(new_zoom) = minimap_state.zoom.zoom_out() {
            minimap_state.zoom = new_zoom;
        }
    }

    let on_minimap = get_minimap_mouse_pos(&windows, minimap_state.mm_size()).is_some();
    // Spec 025 fix: zoom by scroll travel, not event count — the OS streams
    // coalesced/tail events (trackpad momentum) that used to flip every zoom
    // level in one flick. Line notches stay discrete; pixel gestures budget
    // one level per ~0.35 s at full tilt.
    const ZOOM_TRACK_RATE: f32 = 3.0;
    let mut line_notches = 0.0f32;
    let mut track_vel = 0.0f32;
    for event in scroll.read() {
        if !on_minimap {
            continue;
        }
        match event.unit {
            MouseScrollUnit::Line => line_notches += event.y,
            MouseScrollUnit::Pixel => track_vel += -event.y / 100.0,
        }
    }
    if on_minimap {
        let flips = line_notches.round().clamp(-1.0, 1.0);
        if flips > 0.0 {
            if let Some(new_zoom) = minimap_state.zoom.zoom_in() {
                minimap_state.zoom = new_zoom;
            }
        } else if flips < 0.0 {
            if let Some(new_zoom) = minimap_state.zoom.zoom_out() {
                minimap_state.zoom = new_zoom;
            }
        }
        *zoom_budget += track_vel.clamp(-1.0, 1.0) * ZOOM_TRACK_RATE * time.delta_secs();
        while *zoom_budget >= 1.0 {
            *zoom_budget -= 1.0;
            if let Some(new_zoom) = minimap_state.zoom.zoom_in() {
                minimap_state.zoom = new_zoom;
            }
        }
        while *zoom_budget <= -1.0 {
            *zoom_budget += 1.0;
            if let Some(new_zoom) = minimap_state.zoom.zoom_out() {
                minimap_state.zoom = new_zoom;
            }
        }
        // Idle drift decays so a long pause never carries stale budget.
        *zoom_budget *= (1.0 - time.delta_secs()).max(0.0);
    }

    if mouse_buttons.just_pressed(MouseButton::Right) {
        if let (Some(player_pos), Some(mm_pos)) =
            (minimap_state.player_pos, get_minimap_mouse_pos(&windows, minimap_state.mm_size()))
        {
            let pixel_scale = minimap_state.pixel_scale();
            let mm_center = minimap_state.mm_size() / 2.0;
            let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);

            let (world_x, world_y) = map_pixel_to_world(
                mm_pos,
                (player_pos.x, player_pos.y),
                pixel_scale,
                mm_center,
                rotation,
            );

            let new_id = minimap_waypoints.next_id;
            minimap_waypoints.waypoints.push(Waypoint {
                id: new_id,
                position: Vec2::new(world_x, world_y),
            });
            minimap_waypoints.next_id += 1;
        }
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        let window = windows.single().ok();
        let cursor = window.as_ref().and_then(|w| w.cursor_position());
        let mut selected = false;
        if let (Some(player_pos), Some(mm_pos)) =
            (minimap_state.player_pos, get_minimap_mouse_pos(&windows, minimap_state.mm_size()))
        {
            // Click on the minimap: select the hex under the cursor.
            let pixel_scale = minimap_state.pixel_scale();
            let mm_center = minimap_state.mm_size() / 2.0;
            let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
            let (wx, wz) = map_pixel_to_world(
                mm_pos,
                (player_pos.x, player_pos.y),
                pixel_scale,
                mm_center,
                rotation,
            );
            let (q, r) = world_pos_to_tile(wx, wz, WorldGenConfig::TILE_SIZE);
            minimap_state.selected_hex = Some((q, r));
            minimap_state.selected_px = Some(mm_pos);
            selected = true;
        } else if let Some(cursor) = cursor {
            // Click on the world floor: pick the hex under the camera.
            // Never wipe an existing selection here — unrelated clicks (e.g.
            // the Teleport button itself) used to race it away before the
            // button handler could read it.
            let over_widget = widgets
                .iter()
                .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
            if !over_widget {
                if let Ok((camera, cam_transform)) = cameras.single() {
                    if let Ok(world) = camera.viewport_to_world_2d(cam_transform, cursor) {
                        let (q, r) = world_pos_to_tile(world.x, world.y, WorldGenConfig::TILE_SIZE);
                        minimap_state.selected_hex = Some((q, r));
                        // Keep the selection ring on the minimap in sync.
                        if let Some(player_pos) = minimap_state.player_pos {
                            let (mx, my) = world_to_map_pixel(
                                (world.x, world.y),
                                (player_pos.x, player_pos.y),
                                minimap_state.pixel_scale(),
                                minimap_state.mm_size() / 2.0,
                                minimap_state.rotation.map_rotation(minimap_state.facing_angle),
                            );
                            minimap_state.selected_px = Some((mx, my));
                        }
                        selected = true;
                    }
                }
            }
        }
        // A fresh selection on either surface: a second click within 450 ms
        // and ~14 px of the first fires the teleport immediately.
        if selected {
            let now = Instant::now();
            let at = cursor.unwrap_or_default();
            let double = matches!(*last_click, Some((t0, p0))
                if now.duration_since(t0).as_millis() < 450 && at.distance(p0) < 14.0);
            if double {
                crate::net::hud::try_send_teleport(&mut net, &mut minimap_state, &mut latency);
                *last_click = None;
            } else {
                *last_click = Some((now, at));
            }
        } else {
            // UI/empty-space clicks reset the double-click chain.
            *last_click = None;
        }
    }
}

/// Convert screen cursor position to minimap-local pixel coordinates.
fn get_minimap_mouse_pos(windows: &Query<&Window>, mm_size: f32) -> Option<(f32, f32)> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;

    let mm_left = window.width() - MINIMAP_PADDING - mm_size;
    let mm_top = MINIMAP_PADDING;

    let rel_x = cursor.x - mm_left;
    let rel_y = cursor.y - mm_top;

    if rel_x >= 0.0 && rel_x < mm_size && rel_y >= 0.0 && rel_y < mm_size {
        Some((rel_x, rel_y))
    } else {
        None
    }
}

// ============================================================================
// Tile Rendering
// ============================================================================

/// Spawn/despawn/update minimap tile nodes based on loaded chunks and player position.
pub fn render_visible_tiles(
    mut commands: Commands,
    mut minimap_state: ResMut<MinimapState>,
    streaming_world: Res<StreamingWorldResource>,
    mut hex_entity_map: ResMut<HexEntityMap>,
    mut explored_hexes: ResMut<ExploredHexes>,
    map_content_query: Query<Entity, With<MapContent>>,
    mut minimap_assets: ResMut<MinimapAssets>,
    mut images: ResMut<Assets<Image>>,
    mut tile_query: Query<(&mut Node, &mut ImageNode, &mut BorderColor), With<MapTileNode>>,
    mut next_render: Local<f64>,
    time: Res<Time>,
    mut merged: ResMut<MergedMapImage>,
    merged_q: Query<Entity, With<MergedMapNode>>,
    // Spec 023: claimed-land tint + raid pulse (net::td cache).
    net_td: Res<crate::net::td::TdTerritory>,
td_view: Res<crate::net::td::TdView>,
    net: Res<Net>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };

    // Perf: while moving, the view key changes every frame and this pass
    // relayouts every visible minimap node. Gate to ~10 Hz; the skip-key
    // below still avoids all work when standing still.
    if time.elapsed_secs_f64() < *next_render {
        return;
    }
    *next_render = time.elapsed_secs_f64() + 0.1;

    let map_content_entity = match map_content_query.single() {
        Ok(e) => e,
        Err(_) => return,
    };

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let render_radius_world = minimap_state.render_radius_px() / pixel_scale;

    // Square tile bounding box: TILE_SIZE per side in world units, scaled
    // to pixels.
    let tile_w = (WorldGenConfig::TILE_SIZE * pixel_scale).ceil().max(3.0);
    let tile_h = tile_w;

    // Perf: skip the whole pass when the view is unchanged since last frame
    // (standing still → zero work; any movement/zoom/rotation/discovery
    // changes the key and re-renders everything correctly).
    let td_owners_len = net_td.owners.len() as u64;
    let key = (
        player_pos.x,
        player_pos.y,
        pixel_scale,
        minimap_state.mm_size(),
        minimap_state.rotation as u8,
        minimap_state.expanded,
        explored_hexes.explored.len(),
        streaming_world.chunks.chunks.len(),
        td_owners_len,
    );
    if minimap_state.last_tiles_key == Some(key) {
        return;
    }
    minimap_state.last_tiles_key = Some(key);

    // Spec 025 fix: the per-hex tile grid can't paint far zooms (thousands
    // of 3px nodes; hexes sub-pixel) — those levels render one merged
    // terrain bitmap instead. Near zooms keep the crisp tile nodes.
    let far = estimated_tile_count(minimap_state.mm_size(), pixel_scale) > FAR_TILES_BUDGET;
    if far {
        render_merged_map(
            &mut commands,
            &mut minimap_state,
            &streaming_world,
            &explored_hexes,
            map_content_entity,
            &mut merged,
            &mut images,
            pixel_scale,
            player_pos,
        );
        if !minimap_state.merged_active {
            // Entering the bitmap mode: wipe the stale per-hex nodes; they
            // respawn fresh when the player zooms back in.
            for entity in hex_entity_map.hex_entities.values() {
                commands.entity(*entity).despawn();
            }
            hex_entity_map.hex_entities.clear();
            minimap_state.merged_active = true;
        }
        return;
    }
    if minimap_state.merged_active {
        if let Ok(node) = merged_q.single() {
            commands.entity(node).despawn();
        }
        merged.handle = None;
        merged.grid = 0;
        minimap_state.merged_active = false;
    }

    // ---- Phase 1: discover hexes from currently-streamed chunks ----

    // ---- Phase 2: render every explored hex (persistent, even past streaming) ----
    let mm_w = minimap_state.mm_size() + 64.0;
    let mut explored_to_despawn: Vec<u64> = Vec::new();

    for (&hex_id, explored) in explored_hexes.explored.iter() {
        let (screen_x, screen_y) = world_to_map_pixel(
            (explored.center_x, explored.center_y),
            (player_pos.x, player_pos.y),
            pixel_scale,
            mm_center,
            rotation,
        );

        let tile_left = screen_x - tile_w / 2.0;
        let tile_top = screen_y - tile_h / 2.0;

        if screen_x < -64.0 || screen_x > mm_w || screen_y < -64.0 || screen_y > mm_w {
            explored_to_despawn.push(hex_id);
            continue;
        }

        let handle = hex_tile_handle(
            &mut minimap_assets,
            &mut images,
            tile_w as u32,
            tile_h as u32,
            explored.terrain,
        );

        if let Some(&entity) = hex_entity_map.hex_entities.get(&hex_id) {
            if let Ok((mut node, mut image, mut border)) = tile_query.get_mut(entity) {
                node.left = Val::Px(tile_left);
                node.top = Val::Px(tile_top);
                node.width = Val::Px(tile_w);
                node.height = Val::Px(tile_h);
                image.image = handle.clone();
                // Spec 023: claimed hexes tint (gold = mine, violet = other);
                // the capital hex flashes red while a wave is open.
                image.color = td_tile_tint(hex_id, &net_td, &td_view, net.address.clone());
                // Spec 023 §3.9: territory edges get an outline so borders
                // read at a glance on 200+-hex discs.
                *border = BorderColor::all(
                    td_tile_border(hex_id, &net_td, net.address.as_deref())
                        .unwrap_or(Color::NONE),
                );
            }
        } else {
            let color = td_tile_tint(hex_id, &net_td, &td_view, net.address.clone());
            let border = td_tile_border(hex_id, &net_td, net.address.as_deref());
            let entity = commands.spawn((
                Name::new("minimap-tile"),
                MapTileNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(tile_left),
                    top: Val::Px(tile_top),
                    width: Val::Px(tile_w),
                    height: Val::Px(tile_h),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                ImageNode {
                    image: handle,
                    color,
                    ..default()
                },
                BorderColor::all(border.unwrap_or(Color::NONE)),
            )).id();
            commands.entity(entity).insert(ChildOf(map_content_entity));
            hex_entity_map.hex_entities.insert(hex_id, entity);
        }
    }

    // Despawn tile entities whose hexes are off-screen (data stays in `explored`).
    for hex_id in explored_to_despawn {
        if let Some(entity) = hex_entity_map.hex_entities.remove(&hex_id) {
            commands.entity(entity).despawn();
        }
    }

    // ---- Phase 1: discover & per-frame fog on visible chunk hexes ----
    for chunk in streaming_world.chunks.chunks.values() {
        for cell in &chunk.cells {
            let hex_id = WorldCell::id_of(cell.x, cell.y);
            let (center_x, center_y) = cell.world_pos(WorldGenConfig::TILE_SIZE);

            let dx = center_x - player_pos.x;
            let dy = center_y - player_pos.y;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq > render_radius_world * render_radius_world {
                continue;
            }

            // Any hex within the rendered area shows on the map immediately —
            // no small vision radius gate, so there's no pop-in delay after
            // walking onto a new hex.
            explored_hexes.explored.entry(hex_id).or_insert(ExploredCell {
                center_x,
                center_y,
                terrain: cell.terrain,
            });

        }
    }
}

/// Spec 023 helper: multiply-tint for the minimap hex of `hex_id` —
/// gold for my territory, violet for other players', red on the capital
/// hex while a wave targets it. `Color::WHITE` = no tint.
fn td_tile_tint(
    hex_id: u64,
    net_td: &crate::net::td::TdTerritory,
    td_view: &crate::net::td::TdView,
    address: Option<String>,
) -> Color {
    if td_view.incoming.is_some() {
        if let Some(cap) = td_view.capital.as_ref() {
            if cap.hex_id == hex_id {
                return Color::srgb(1.0, 0.4, 0.35);
            }
        }
    }
    let is_mine = address
        .as_deref()
        .map(|me| net_td.owners.get(&hex_id).map(|o| o == me).unwrap_or(false));
    match net_td.owners.get(&hex_id) {
        Some(_) if is_mine == Some(true) => Color::srgb(1.0, 0.85, 0.45),
        Some(_) => Color::srgb(0.7, 0.55, 1.0),
        None => Color::WHITE,
    }
}

/// Spec 023 §3.9: the territory border — an owned hex that touches an
/// unowned hex (or a rival's) draws an outline, dark mine-gold or dark
/// rival-violet. `None` = no border (interior hexes / unowned hexes).
fn td_tile_border(
    hex_id: u64,
    net_td: &crate::net::td::TdTerritory,
    address: Option<&str>,
) -> Option<Color> {
    let owner = net_td.owners.get(&hex_id)?;
    let is_mine = address == Some(owner.as_str());
    let (q, r) = {
        let c = TileCoord::from_id(hex_id);
        (c.x, c.y)
    };
    let is_border = TileCoord::new(q, r)
        .neighbors()
        .iter()
        .any(|&(nq, nr)| {
            net_td
                .owners
                .get(&TileCoord::new(nq, nr).to_id())
                .map(|other| other != owner)
                .unwrap_or(true)
        });
    is_border.then(|| {
        if is_mine {
            Color::srgb(0.55, 0.4, 0.0)
        } else {
            Color::srgb(0.35, 0.2, 0.5)
        }
    })
}

/// Create or update the fog-of-war overlay texture.

/// Rotate the player arrow marker to indicate facing direction.
pub fn update_player_marker(
    minimap_state: Res<MinimapState>,
    mut arrow_query: Query<&mut UiTransform, With<PlayerArrow>>,
) {
    let Ok(mut transform) = arrow_query.single_mut() else { return };

    let angle = minimap_state.rotation.marker_rotation(minimap_state.facing_angle);
    let want = Rot2::radians(angle);
    if transform.rotation != want {
        transform.rotation = want;
    }
}

// ============================================================================
// Waypoint Rendering
// ============================================================================

/// Spawn/update/despawn waypoint marker nodes.
pub fn render_waypoints(
    minimap_state: Res<MinimapState>,
    minimap_waypoints: Res<MinimapWaypoints>,
    mut commands: Commands,
    mut waypoint_entity_map: ResMut<WaypointEntityMap>,
    map_content_query: Query<Entity, With<MapContent>>,
    mut node_query: Query<&mut Node, With<WaypointNode>>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };

    let map_content_entity = match map_content_query.single() {
        Ok(e) => e,
        Err(_) => return,
    };

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let mm_size = minimap_state.mm_size();

    let mut active_ids: HashSet<u64> = HashSet::new();

    for wp in &minimap_waypoints.waypoints {
        let (screen_x, screen_y) = world_to_map_pixel(
            (wp.position.x, wp.position.y),
            (player_pos.x, player_pos.y),
            pixel_scale,
            mm_center,
            rotation,
        );

        if screen_x < -10.0 || screen_x > mm_size + 10.0
            || screen_y < -10.0 || screen_y > mm_size + 10.0
        {
            continue;
        }

        active_ids.insert(wp.id);

        if let Some(&entity) = waypoint_entity_map.entities.get(&wp.id) {
            if let Ok(mut node) = node_query.get_mut(entity) {
                node.left = Val::Px(screen_x - 6.0);
                node.top = Val::Px(screen_y - 6.0);
            }
        } else {
            let marker_size = 12.0;
            let entity = commands.spawn((
                Name::new("waypoint-marker"),
                WaypointNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(screen_x - marker_size / 2.0),
                    top: Val::Px(screen_y - marker_size / 2.0),
                    width: Val::Px(marker_size),
                    height: Val::Px(marker_size),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.9, 0.2, 0.9)),
            )).id();
            commands.entity(entity).insert(ChildOf(map_content_entity));
            waypoint_entity_map.entities.insert(wp.id, entity);
        }
    }

    let to_remove: Vec<u64> = waypoint_entity_map.entities.keys()
        .filter(|id| !active_ids.contains(id))
        .cloned()
        .collect();

    for id in to_remove {
        if let Some(entity) = waypoint_entity_map.entities.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

// ============================================================================
// Nav Marker Rendering
// ============================================================================

/// Render navigation POI markers on the minimap.
pub fn render_nav_markers(
    minimap_state: Res<MinimapState>,
    minimap_markers: Res<MinimapMarkers>,
    mut commands: Commands,
    map_content_query: Query<Entity, With<MapContent>>,
    marker_query: Query<Entity, With<NavMarkerNode>>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };

    let map_content_entity = match map_content_query.single() {
        Ok(e) => e,
        Err(_) => return,
    };

    for entity in marker_query.iter() {
        commands.entity(entity).despawn();
    }

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);

    for marker in &minimap_markers.markers {
        let (screen_x, screen_y) = world_to_map_pixel(
            (marker.position.x, marker.position.y),
            (player_pos.x, player_pos.y),
            pixel_scale,
            mm_center,
            rotation,
        );

        let marker_size = 12.0;
        let entity = commands.spawn((
            Name::new("nav-marker"),
            NavMarkerNode,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_x - marker_size / 2.0),
                top: Val::Px(screen_y - marker_size / 2.0),
                width: Val::Px(marker_size),
                height: Val::Px(marker_size),
                ..default()
            },
            BackgroundColor(marker.color),
        )).id();
        commands.entity(entity).insert(ChildOf(map_content_entity));
    }
}

// ============================================================================
// Capital Marker Rendering
// ============================================================================

/// Spec 023 markers on the map: a diamond per placed capital.
/// Colors — gold = my capital, violet = another player's; red flash while
/// any wave is open against it. Returns `(fill, border)`.
pub fn capital_marker_color(is_mine: bool, raided: bool) -> (Color, Color) {
    if raided {
        (
            Color::srgb(1.0, 0.35, 0.3),
            Color::srgb(0.6, 0.12, 0.08),
        )
    } else if is_mine {
        (
            Color::srgb(1.0, 0.78, 0.15),
            Color::srgb(0.55, 0.4, 0.0),
        )
    } else {
        (
            Color::srgb(0.8, 0.5, 1.0),
            Color::srgb(0.45, 0.28, 0.6),
        )
    }
}

/// Spawn/update/despawn capital markers (one diamond per live capital).
/// In-place diff keyed by capital hex id, mirroring `render_waypoints`.
pub fn render_capital_markers(
    mut commands: Commands,
    minimap_state: Res<MinimapState>,
    td: Res<crate::net::td::TdTerritory>,
    net: Res<crate::net::plugin::Net>,
    mut entity_map: ResMut<CapitalMarkerEntityMap>,    map_content_query: Query<Entity, With<MapContent>>,
    mut marker_query: Query<(
        &CapitalMarkerNode,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };
    let Ok(map_content_entity) = map_content_query.single() else { return };

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let mm_w = minimap_state.mm_size() + 64.0;

    // Live capitals visible on the map, with ownership + raid flags.
    let active: HashMap<u64, (Color, Color)> = td
        .capitals
        .iter()
        .map(|&hex_id| {
            let is_mine = net
                .address
                .as_deref()
                .map(|me| td.owners.get(&hex_id).map(|o| o == me).unwrap_or(false))
                .unwrap_or(false);
            let colors =
                capital_marker_color(is_mine, td.wave_targets.contains(&hex_id));
            (hex_id, colors)
        })
        .collect();

    for (&hex_id, &(fill, border_color)) in &active {
        let c = TileCoord::from_id(hex_id);
        let (wx, wy) = WorldGrid::tile_center(c.x, c.y, HEX_SIZE);
        let (x, y) = world_to_map_pixel(
            (wx, wy),
            (player_pos.x, player_pos.y),
            pixel_scale,
            mm_center,
            rotation,
        );
        if x < -64.0 || x > mm_w || y < -64.0 || y > mm_w {
            continue; // off-map: keep the spot in `entity_map` for re-entry
        }

        if let Some(&entity) = entity_map.entities.get(&hex_id) {
            if let Ok((_, mut node, mut bg, mut border)) = marker_query.get_mut(entity) {
                node.left = Val::Px(x - MARKER_SIZE / 2.0);
                node.top = Val::Px(y - MARKER_SIZE / 2.0);
                bg.0 = fill;
                *border = BorderColor::all(border_color);
            }
        } else {
            let entity = commands
                .spawn((
                    Name::new("capital-marker"),
                    CapitalMarkerNode,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - MARKER_SIZE / 2.0),
                        top: Val::Px(y - MARKER_SIZE / 2.0),
                        width: Val::Px(MARKER_SIZE),
                        height: Val::Px(MARKER_SIZE),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    // 45° tilt turns the node into a diamond.
                    UiTransform::from_rotation(Rot2::radians(std::f32::consts::FRAC_PI_4)),
                    BackgroundColor(fill),
                    BorderColor::all(border_color),
                    ChildOf(map_content_entity),
                ))
                .id();
            entity_map.entities.insert(hex_id, entity);
        }
    }

    // Destroyed / vacated capitals lose their marker.
    let removed: Vec<u64> = entity_map
        .entities
        .keys()
        .filter(|id| !active.contains_key(id))
        .cloned()
        .collect();
    for hex_id in removed {
        if let Some(entity) = entity_map.entities.remove(&hex_id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Approximate marker width; the diamond reads at every zoom level.
const MARKER_SIZE: f32 = 12.0;

// ============================================================================
// Resize / Visibility
// ============================================================================

/// Resize minimap container and reposition center-fixed elements when size changes.
pub fn resize_minimap_container(
    minimap_state: Res<MinimapState>,
    mut query: Query<
        (
            &mut Node,
            Option<&MinimapRoot>,
            Option<&MapContent>,
            Option<&PlayerArrow>,
        ),
        Or<(With<MinimapRoot>, With<MapContent>, With<PlayerArrow>)>,
    >,
) {
    if !minimap_state.is_changed() {
        return;
    }

    let mm_size = minimap_state.mm_size();
    let mm_center = mm_size / 2.0;

    for (mut node, root, map, arrow) in query.iter_mut() {
        if root.is_some() || map.is_some() {
            node.width = Val::Px(mm_size);
            node.height = Val::Px(mm_size);
        }
        if arrow.is_some() {
            node.left = Val::Px(mm_center - 8.0);
            node.top = Val::Px(mm_center - 8.0);
        }
    }
}

// ============================================================================
// Remote Player Dots
// ============================================================================

/// Spawn/despawn dots for other players on the minimap (Spec 009 T2.3/T4.3).
/// In-place diff: existing dots are repositioned, only new players spawn and
/// only gone players despawn (replaces the per-frame rebuild).
pub fn render_remote_players(
    minimap_state: Res<MinimapState>,
    net: Res<crate::net::plugin::Net>,
    minimap_assets: Res<MinimapAssets>,
    mut commands: Commands,
    map_content_query: Query<Entity, With<MapContent>>,
    mut dot_query: Query<(Entity, &RemotePlayerDot, &mut Node)>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };
    let Ok(map_content_entity) = map_content_query.single() else { return };

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let mm_w = minimap_state.mm_size() + 64.0;
    let dot_size = 8.0;

    let active: Vec<(String, Vec2)> = net
        .players
        .iter()
        .filter(|(_, snap)| snap.online)
        .filter_map(|(address, snap)| {
            let (x, y) = world_to_map_pixel(
                (snap.x, snap.y),
                (player_pos.x, player_pos.y),
                pixel_scale,
                mm_center,
                rotation,
            );
            if x < -64.0 || x > mm_w || y < -64.0 || y > mm_w {
                return None;
            }
            Some((address.clone(), Vec2::new(x, y)))
        })
        .collect();

    let mut kept: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (entity, dot, mut node) in dot_query.iter_mut() {
        match active.iter().find(|(a, _)| *a == dot.address) {
            Some((_, pos)) => {
                let left = Val::Px(pos.x - dot_size / 2.0);
                let top = Val::Px(pos.y - dot_size / 2.0);
                if node.left != left {
                    node.left = left;
                }
                if node.top != top {
                    node.top = top;
                }
                kept.insert(dot.address.clone());
            }
            None => {
                commands.entity(entity).despawn();
            }
        }
    }
    for (address, pos) in &active {
        if kept.contains(address) {
            continue;
        }
        commands.entity(map_content_entity).with_children(|parent| {
            parent.spawn((
                Name::new(format!("remote-dot-{address}")),
                RemotePlayerDot { address: address.clone() },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(dot_size),
                    height: Val::Px(dot_size),
                    left: Val::Px(pos.x - dot_size / 2.0),
                    top: Val::Px(pos.y - dot_size / 2.0),
                    ..default()
                },
                UiTransform::IDENTITY,
                ImageNode::new(minimap_assets.dot_texture.clone()),
            ));
        });
    }
}

// ============================================================================
// Teleport Selection Marker
// ============================================================================

/// Render a ring around the last left-clicked hex (Spec 009 T3.1).
/// Only respawns when the selection actually changed.
pub fn render_selection_marker(
    mut minimap_state: ResMut<MinimapState>,
    mut commands: Commands,
    map_content_query: Query<Entity, With<MapContent>>,
    marker_query: Query<Entity, With<SelectionMarker>>,
) {
    let Ok(map_content_entity) = map_content_query.single() else { return };
    if minimap_state.selected_px == minimap_state.last_selection_px {
        return;
    }
    let previous = minimap_state.last_selection_px;
    minimap_state.last_selection_px = minimap_state.selected_px;
    for entity in marker_query.iter() {
        commands.entity(entity).despawn();
    }
    let Some(px) = minimap_state.selected_px else {
        let _ = previous;
        return;
    };
    let size = 22.0;
    commands.entity(map_content_entity).with_children(|parent| {
        parent.spawn((
            Name::new("selection-marker"),
            SelectionMarker,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                left: Val::Px(px.0 - size / 2.0),
                top: Val::Px(px.1 - size / 2.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.3, 1.0, 0.5)),
            UiTransform::IDENTITY,
        ));
    });
}

#[cfg(test)]
mod tests_zoom_budget {
    use super::*;

    #[test]
    fn merged_rows_sample_top_down_in_one_direction() {
        // Row 0 samples the very TOP of the map box; the LAST row samples
        // just above the bottom. Every row increases downward — a flip
        // here mirrored the whole zoomed-out map N/S (fixed once).
        assert!(merged_row_y(0, 2.0) < merged_row_y(1, 2.0));
        assert!((merged_row_y(0, 2.0) - 1.0).abs() < 1e-4);
        let last = merged_row_y(99, 2.0);
        assert!(last <= 200.0 && last > 198.0);
    }

    #[test]
    fn merged_map_world_north_is_map_top() {
        // World N of the player (y greater) must land ABOVE the center of
        // the map: mm-space y (down-positive) shrinks below mm/2, and the
        // image row for that map y (merged_row_y) sits in the image's top
        // half. With the old bottom-first row order this was inverted.
        let player = (0.0_f32, 0.0);
        let scale: f32 = MinimapZoom::World.pixel_scale();
        let mm = 200.0_f32;
        let (map_x, map_y) =
            idlecore_core::minimap::world_to_map_pixel((0.0, 200.0), player, scale, mm / 2.0, 0.0);
        assert_eq!(map_x, mm / 2.0, "straight north stays on the centerline");
        assert!(map_y < mm / 2.0, "north paints above center: {map_y}");
        // The bitmap row that paints map_y: row = map_y / step - 0.5.
        let row = (map_y - 0.5) / 2.0;
        assert!(row < mm / 2.0 / 2.0, "north row sits in the image top half");
    }

    #[test]
    fn near_zooms_keep_tiles_far_zooms_go_bitmap() {
        // Local (0.15): ~0.9k hexes < budget → crisp per-hex nodes.
        assert!(
            estimated_tile_count(200.0, MinimapZoom::Local.pixel_scale())
                <= FAR_TILES_BUDGET
        );
        // Area/World far exceed the budget → merged bitmap.
        assert!(
            estimated_tile_count(200.0, MinimapZoom::Area.pixel_scale())
                > FAR_TILES_BUDGET
        );
        assert!(
            estimated_tile_count(200.0, MinimapZoom::World.pixel_scale())
                > FAR_TILES_BUDGET
        );
        // Zoom-in levels stay node-priced: Close ~1.0k, Max ~0.3k.
        assert!(
            estimated_tile_count(200.0, MinimapZoom::Close.pixel_scale())
                <= FAR_TILES_BUDGET
        );
        assert!(
            estimated_tile_count(200.0, MinimapZoom::Max.pixel_scale())
                <= FAR_TILES_BUDGET
        );
        // Expanded (M) doubles the box: compact-Local fits nodes, but the
        // expanded box at Local needs >4.6k hexes → merged bitmap there too.
        assert!(
            estimated_tile_count(400.0, MinimapZoom::Local.pixel_scale())
                > FAR_TILES_BUDGET
        );
        assert!(
            estimated_tile_count(400.0, MinimapZoom::Close.pixel_scale())
                <= FAR_TILES_BUDGET
        );
    }
}
