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
use bevy::input::mouse::MouseWheel;
use bevy::math::Rot2;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::ImageNode;
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::minimap::{map_pixel_to_world, world_to_map_pixel, RotationMode};
use idlecore_core::world_gen::{HexCell, WorldGenConfig};
use std::collections::{HashMap, HashSet};

use crate::plugins::world::StreamingWorldResource;
use crate::player::Player;
use crate::fps_counter::FpsText;

// ============================================================================
// Constants
// ============================================================================

const COMPACT_SIZE: f32 = 200.0;
const EXPANDED_SIZE: f32 = 400.0;
const MINIMAP_PADDING: f32 = 10.0;
const HEX_SIZE: f32 = WorldGenConfig::HEX_SIZE;

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
    pub fn pixel_scale(&self) -> f32 {
        match self {
            MinimapZoom::Local => 0.005,
            MinimapZoom::Area => 0.0025,
            MinimapZoom::World => 0.00125,
            MinimapZoom::Close => 0.01,
            MinimapZoom::Max => 0.02,
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
    pub last_tiles_key: Option<(f32, f32, f32, f32, u8, bool, usize, usize)>,
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

    /// Render radius (vision + soft border) in minimap pixels.
    pub fn render_radius_px(&self) -> f32 {
        (1500.0 + 300.0) * self.pixel_scale()
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
#[derive(Debug, Clone, Copy)]
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

/// Track waypoint entities for proper despawn lifecycle.
#[derive(Resource, Default)]
pub struct WaypointEntityMap {
    pub entities: HashMap<u64, Entity>,
}

/// Track which chunk was last loaded (avoid redundant chunk operations).
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
fn create_hexagon_image(width: u32, height: u32, color: Color) -> Image {
    let pad = 2u32;
    let cw = width + pad * 2;
    let ch = height + pad * 2;
    let mut pixels = vec![0u8; (cw * ch * 4) as usize];
    let srgba = color.to_srgba();
    let px = (srgba.red * 255.0) as u8;
    let py = (srgba.green * 255.0) as u8;
    let pz = (srgba.blue * 255.0) as u8;
    let alpha = (srgba.alpha * 255.0) as u8;

    let cx = cw as f32 / 2.0;
    let cy = ch as f32 / 2.0;
    let rpx = height as f32 / 2.0;
    let kx = width as f32 / (SQRT_3 * rpx);

    // Pointy-top hexagon corners, matching the world mesh corner angles.
    let mut corners = [(0.0, 0.0); 6];
    for i in 0..6 {
        let angle = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        corners[i] = (
            cx + rpx * angle.cos() * kx,
            cy + rpx * angle.sin(),
        );
    }

    for y in 0..ch {
        for x in 0..cw {
            let idx = ((y * cw + x) * 4) as usize;
            if point_in_polygon(x as f32, y as f32, &corners) {
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

/// Ray-cast point-in-polygon test for convex hexagon corners.
fn point_in_polygon(tx: f32, ty: f32, corners: &[(f32, f32); 6]) -> bool {
    let mut inside = false;
    let mut j = 5;
    for i in 0..6 {
        let (xi, yi) = corners[i];
        let (xj, yj) = corners[j];
        if (yi > ty) != (yj > ty)
            && tx < (xj - xi) * (ty - yi) / (yj - yi) + xi
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Get (or create) a hexagon tile texture matching the current zoom, so tile
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
    let handle = images.add(create_hexagon_image(width, height, terrain_color(&terrain)));
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
        .map(|transform| Vec2::new(transform.translation.x, transform.translation.z));
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

    let (q, r) = world_pos_to_hex(player_pos.x, player_pos.y, HEX_SIZE);
    let chunk_size = WorldGenConfig::CHUNK_SIZE;
    let current_chunk = (q / chunk_size, r / chunk_size);

    if Some(current_chunk) == chunk_state.last_chunk {
        return;
    }

    let render_radius_world = minimap_state.render_radius_px() / minimap_state.pixel_scale();
    let view_radius = ((render_radius_world / HEX_SIZE as f32) as i32 / chunk_size) + 5;
    let _ = view_radius.max(15);

    let config = streaming_world.config;
    streaming_world.chunks.stream_around(&config, q, r);

    chunk_state.last_chunk = Some(current_chunk);
}

// ============================================================================
// Input Handling
// ============================================================================

/// Handle minimap input: zoom (mouse wheel), rotation (N), expand (M), waypoints (right-click).
pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut minimap_state: ResMut<MinimapState>,
    mut minimap_waypoints: ResMut<MinimapWaypoints>,
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
    for event in scroll.read() {
        if !on_minimap {
            continue;
        }
        if event.y > 0.0 {
            if let Some(new_zoom) = minimap_state.zoom.zoom_in() {
                minimap_state.zoom = new_zoom;
            }
        } else if event.y < 0.0 {
            if let Some(new_zoom) = minimap_state.zoom.zoom_out() {
                minimap_state.zoom = new_zoom;
            }
        }
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
        if let (Some(player_pos), Some(mm_pos)) =
            (minimap_state.player_pos, get_minimap_mouse_pos(&windows, minimap_state.mm_size()))
        {
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
            let (q, r) = world_pos_to_hex(wx, wz, WorldGenConfig::HEX_SIZE);
            minimap_state.selected_hex = Some((q, r));
            minimap_state.selected_px = Some(mm_pos);
        } else {
            minimap_state.selected_hex = None;
            minimap_state.selected_px = None;
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
    mut tile_query: Query<(&mut Node, &mut ImageNode), With<MapTileNode>>,
) {
    let Some(player_pos) = minimap_state.player_pos else { return };

    let map_content_entity = match map_content_query.single() {
        Ok(e) => e,
        Err(_) => return,
    };

    let pixel_scale = minimap_state.pixel_scale();
    let mm_center = minimap_state.mm_size() / 2.0;
    let rotation = minimap_state.rotation.map_rotation(minimap_state.facing_angle);
    let render_radius_world = minimap_state.render_radius_px() / pixel_scale;

    // Pointy-top hexagon bounding box (corners at 30° + 60°·i):
    // width = √3·R, height = 2R in world units, scaled to pixels.
    let tile_w = (SQRT_3 * HEX_SIZE * pixel_scale).ceil().max(3.0);
    let tile_h = (2.0 * HEX_SIZE * pixel_scale).ceil().max(3.0);

    // Perf: skip the whole pass when the view is unchanged since last frame
    // (standing still → zero work; any movement/zoom/rotation/discovery
    // changes the key and re-renders everything correctly).
    let key = (
        player_pos.x,
        player_pos.y,
        pixel_scale,
        minimap_state.mm_size(),
        minimap_state.rotation as u8,
        minimap_state.expanded,
        explored_hexes.explored.len(),
        streaming_world.chunks.chunks.len(),
    );
    if minimap_state.last_tiles_key == Some(key) {
        return;
    }
    minimap_state.last_tiles_key = Some(key);

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
            if let Ok((mut node, mut image)) = tile_query.get_mut(entity) {
                node.left = Val::Px(tile_left);
                node.top = Val::Px(tile_top);
                node.width = Val::Px(tile_w);
                node.height = Val::Px(tile_h);
                image.image = handle.clone();
            }
        } else {
            let entity = commands.spawn((
                Name::new("minimap-tile"),
                MapTileNode,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(tile_left),
                    top: Val::Px(tile_top),
                    width: Val::Px(tile_w),
                    height: Val::Px(tile_h),
                    ..default()
                },
                ImageNode::new(handle),
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
            let hex_id = HexCell::id_of(cell.q, cell.r);
            let (center_x, center_y) = cell.world_pos(HEX_SIZE);

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

/// Create or update the fog-of-war overlay texture.

/// Rotate the player arrow marker to indicate facing direction.
pub fn update_player_marker(
    minimap_state: Res<MinimapState>,
    mut arrow_query: Query<&mut UiTransform, With<PlayerArrow>>,
) {
    let Ok(mut transform) = arrow_query.single_mut() else { return };

    let angle = minimap_state.rotation.marker_rotation(minimap_state.facing_angle);
    transform.rotation = Rot2::radians(angle);
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
