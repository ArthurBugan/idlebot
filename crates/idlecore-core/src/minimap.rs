//! Minimap system — small overview map of the game world.
//!
//! Shows player position, nearby hexes, and objects. Supports zoom levels.

use crate::hex::HexCoord;
use crate::terrain::TerrainType;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Zoom Levels
// ---------------------------------------------------------------------------

/// Zoom level for the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoomLevel {
    /// Local view: ~5 hex radius
    Local,
    /// Mid view: ~20 hex radius
    Mid,
    /// Global view: ~64 hex radius
    Global,
}

impl ZoomLevel {
    /// Hex radius for this zoom level.
    pub fn radius(&self) -> i32 {
        match self {
            ZoomLevel::Local => 5,
            ZoomLevel::Mid => 20,
            ZoomLevel::Global => 64,
        }
    }

    /// Pixel scale (hex size in pixels) for this zoom level.
    pub fn scale(&self) -> f32 {
        match self {
            ZoomLevel::Local => 1.0,
            ZoomLevel::Mid => 0.5,
            ZoomLevel::Global => 0.25,
        }
    }

    /// Zoom in one level.
    pub fn zoom_in(&self) -> Option<ZoomLevel> {
        match self {
            ZoomLevel::Local => Some(ZoomLevel::Mid),
            ZoomLevel::Mid => Some(ZoomLevel::Global),
            ZoomLevel::Global => None, // already max
        }
    }

    /// Zoom out one level.
    pub fn zoom_out(&self) -> Option<ZoomLevel> {
        match self {
            ZoomLevel::Local => None, // already min
            ZoomLevel::Mid => Some(ZoomLevel::Local),
            ZoomLevel::Global => Some(ZoomLevel::Mid),
        }
    }
}

// ---------------------------------------------------------------------------
// Chunk System
// ---------------------------------------------------------------------------

/// A chunk of the world (group of hex tiles) for the minimap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapChunk {
    /// Chunk center coordinates.
    pub center: HexCoord,
    /// Generated texture data (if available).
    pub texture: Option<MinimapTexture>,
    /// Whether the chunk has been generated.
    pub generated: bool,
    /// Whether the chunk is dirty (needs regeneration).
    pub dirty: bool,
    /// The tiles in this chunk (used for texture generation).
    pub tiles: Vec<crate::world::WorldTile>,
}

/// A cached texture for a chunk's minimap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapTexture {
    /// Width of the texture in pixels.
    pub width: u32,
    /// Height of the texture in pixels.
    pub height: u32,
    /// Pixel data (RGBA).
    pub data: Vec<u8>,
}

impl MinimapChunk {
    /// Create a new empty chunk.
    pub fn new(center: HexCoord) -> Self {
        Self {
            center,
            texture: None,
            generated: false,
            dirty: false,
            tiles: Vec::new(),
        }
    }

    /// Create a new chunk with tiles.
    pub fn new_with_tiles(center: HexCoord, tiles: Vec<crate::world::WorldTile>) -> Self {
        Self {
            center,
            texture: None,
            generated: false,
            dirty: false,
            tiles,
        }
    }

    /// Mark the chunk as dirty (needs regeneration).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Mark the chunk as clean (fully regenerated).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
        self.generated = true;
    }

    /// Generate a new texture for this chunk from the tiles.
    pub fn generate_texture(&mut self) {
        if self.tiles.is_empty() {
            return;
        }

        // Calculate bounds for the texture
        let min_x = self.tiles.iter().map(|t| t.center_x).fold(f32::INFINITY, f32::min);
        let max_x = self.tiles.iter().map(|t| t.center_x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = self.tiles.iter().map(|t| t.center_y).fold(f32::INFINITY, f32::min);
        let max_y = self.tiles.iter().map(|t| t.center_y).fold(f32::NEG_INFINITY, f32::max);

        let width = (max_x - min_x) as u32 + 1;
        let height = (max_y - min_y) as u32 + 1;

        let size = (width * height * 4) as usize;
        let mut data = vec![0u8; size];

        // Fill texture with terrain colors
        for tile in &self.tiles {
            let color = tile.terrain.minimap_color();
            let x = (tile.center_x - min_x) as usize;
            let y = (tile.center_y - min_y) as usize;

            if x < width as usize && y < height as usize {
                let idx = (y * width as usize + x) * 4;
                data[idx] = (color[0] * 255.0) as u8;     // R
                data[idx + 1] = (color[1] * 255.0) as u8; // G
                data[idx + 2] = (color[2] * 255.0) as u8; // B
                data[idx + 3] = 255;                       // A
            }
        }

        self.texture = Some(MinimapTexture { width, height, data });
        self.generated = true;
        self.dirty = false;
    }
}

// ---------------------------------------------------------------------------
// Minimap Data Structures
// ---------------------------------------------------------------------------

/// Marker for an object on the minimap (player, NPC, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMarker {
    /// Hex coordinates of the object.
    pub hex: HexCoord,
    /// Type of object for rendering.
    pub object_type: ObjectType,
    /// Display label (optional).
    pub label: Option<String>,
}

/// Types of objects that can appear on the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectType {
    Player,
    OtherPlayer,
    Vehicle,
    Building,
    Resource,
}

impl ObjectType {
    /// Color for this object type (RGB).
    pub fn color(&self) -> (f32, f32, f32) {
        match self {
            ObjectType::Player => (0.0, 0.0, 1.0), // Blue
            ObjectType::OtherPlayer => (0.0, 1.0, 0.0), // Green
            ObjectType::Vehicle => (1.0, 1.0, 0.0), // Yellow
            ObjectType::Building => (0.5, 0.5, 0.5), // Gray
            ObjectType::Resource => (1.0, 0.0, 1.0), // Magenta
        }
    }
}

/// Terrain color for minimap rendering.
pub fn terrain_color(terrain: &TerrainType) -> (f32, f32, f32) {
    let [r, g, b] = terrain.minimap_color();
    (r, g, b)
}

/// The minimap itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Minimap {
    /// Current zoom level.
    pub zoom: ZoomLevel,
    /// Center hex (usually player position).
    pub center: HexCoord,
    /// Viewport hexes (within zoom radius).
    pub viewport_hexes: Vec<HexCoord>,
    /// Object markers on the minimap.
    pub objects: Vec<ObjectMarker>,
}

impl Minimap {
    /// Create a new minimap centered on the given hex.
    pub fn new(center: HexCoord) -> Self {
        let viewport = Self::generate_viewport(&center, ZoomLevel::Local.radius());
        Self {
            zoom: ZoomLevel::Local,
            center,
            viewport_hexes: viewport,
            objects: Vec::new(),
        }
    }

    /// Generate hexes within the viewport for the given center and radius.
    fn generate_viewport(center: &HexCoord, radius: i32) -> Vec<HexCoord> {
        let mut hexes = Vec::new();
        for dq in -radius..=radius {
            for dr in -radius..=radius {
                let hex = HexCoord::new(center.q + dq, center.r + dr);
                if hex.distance(center) <= radius {
                    hexes.push(hex);
                }
            }
        }
        hexes
    }

    /// Zoom in one level.
    pub fn zoom_in(&mut self) {
        if let Some(new_zoom) = self.zoom.zoom_in() {
            self.zoom = new_zoom;
            self.viewport_hexes = Self::generate_viewport(&self.center, self.zoom.radius());
        }
    }

    /// Zoom out one level.
    pub fn zoom_out(&mut self) {
        if let Some(new_zoom) = self.zoom.zoom_out() {
            self.zoom = new_zoom;
            self.viewport_hexes = Self::generate_viewport(&self.center, self.zoom.radius());
        }
    }

    /// Add an object marker to the minimap.
    pub fn add_object(&mut self, marker: ObjectMarker) {
        self.objects.push(marker);
    }

    /// Remove an object marker by hex.
    pub fn remove_object(&mut self, hex: &HexCoord) -> bool {
        let len_before = self.objects.len();
        self.objects.retain(|m| &m.hex != hex);
        self.objects.len() < len_before
    }

    /// Get objects within the current viewport.
    pub fn viewport_objects(&self) -> Vec<&ObjectMarker> {
        self.objects
            .iter()
            .filter(|m| self.viewport_hexes.contains(&m.hex))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Rendering Helpers
// ---------------------------------------------------------------------------

/// Convert hex coordinates to pixel position for minimap rendering.
pub fn hex_to_minimap_pixel(hex: &HexCoord, scale: f32, offset_x: f32, offset_y: f32) -> (f32, f32) {
    let q = hex.q as f32;
    let r = hex.r as f32;
    let sqrt3 = 1.7320508075688772_f32; // sqrt(3)
    let x = scale * (q * 1.5) + offset_x;
    let y = scale * (r * sqrt3 * 1.5) + offset_y;
    (x, y)
}

/// Draw a hex outline on a canvas (simplified — returns the hex center for rendering).
pub fn draw_hex_outline(hex: &HexCoord, scale: f32, offset_x: f32, offset_y: f32) -> (f32, f32) {
    hex_to_minimap_pixel(hex, scale, offset_x, offset_y)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_level_radii() {
        assert_eq!(ZoomLevel::Local.radius(), 5);
        assert_eq!(ZoomLevel::Mid.radius(), 20);
        assert_eq!(ZoomLevel::Global.radius(), 64);
    }

    #[test]
    fn test_zoom_in_cycle() {
        assert_eq!(ZoomLevel::Local.zoom_in(), Some(ZoomLevel::Mid));
        assert_eq!(ZoomLevel::Mid.zoom_in(), Some(ZoomLevel::Global));
        assert_eq!(ZoomLevel::Global.zoom_in(), None);
    }

    #[test]
    fn test_zoom_out_cycle() {
        assert_eq!(ZoomLevel::Global.zoom_out(), Some(ZoomLevel::Mid));
        assert_eq!(ZoomLevel::Mid.zoom_out(), Some(ZoomLevel::Local));
        assert_eq!(ZoomLevel::Local.zoom_out(), None);
    }

    #[test]
    fn test_minimap_new() {
        let center = HexCoord::new(0, 0);
        let minimap = Minimap::new(center);
        assert_eq!(minimap.zoom, ZoomLevel::Local);
        assert_eq!(minimap.center, center);
        assert!(!minimap.viewport_hexes.is_empty());
    }

    #[test]
    fn test_minimap_zoom_in() {
        let mut minimap = Minimap::new(HexCoord::new(0, 0));
        minimap.zoom_in();
        assert_eq!(minimap.zoom, ZoomLevel::Mid);
    }

    #[test]
    fn test_minimap_zoom_out() {
        let mut minimap = Minimap::new(HexCoord::new(0, 0));
        minimap.zoom_in(); // Go to Mid
        minimap.zoom_out(); // Back to Local
        assert_eq!(minimap.zoom, ZoomLevel::Local);
    }

    #[test]
    fn test_object_marker_colors() {
        assert_eq!(ObjectType::Player.color(), (0.0, 0.0, 1.0));
        assert_eq!(ObjectType::OtherPlayer.color(), (0.0, 1.0, 0.0));
        assert_eq!(ObjectType::Vehicle.color(), (1.0, 1.0, 0.0));
    }

    #[test]
    fn test_terrain_colors() {
        assert_eq!(terrain_color(&TerrainType::Grass), (0.496, 0.792, 0.322));
        assert_eq!(terrain_color(&TerrainType::Water), (0.255, 0.404, 0.882));
    }

    #[test]
    fn test_hex_to_minimap_pixel() {
        let hex = HexCoord::new(1, 0);
        let (x, y) = hex_to_minimap_pixel(&hex, 1.0, 0.0, 0.0);
        // x = 1.5, y = 0 (r=0)
        assert!((x - 1.5).abs() < 0.001);
        assert!((y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_minimap_add_remove_object() {
        let mut minimap = Minimap::new(HexCoord::new(0, 0));
        let marker = ObjectMarker {
            hex: HexCoord::new(1, 0),
            object_type: ObjectType::Player,
            label: None,
        };
        minimap.add_object(marker);
        assert_eq!(minimap.objects.len(), 1);
        minimap.remove_object(&HexCoord::new(1, 0));
        assert_eq!(minimap.objects.len(), 0);
    }

    #[test]
    fn test_minimap_viewport_objects() {
        let mut minimap = Minimap::new(HexCoord::new(0, 0));
        minimap.add_object(ObjectMarker { hex: HexCoord::new(1, 0), object_type: ObjectType::Player, label: None });
        minimap.add_object(ObjectMarker { hex: HexCoord::new(100, 100), object_type: ObjectType::Player, label: None }); // Out of viewport
        let viewport_objs = minimap.viewport_objects();
        assert_eq!(viewport_objs.len(), 1);
    }

    #[test]
    fn test_minimap_chunk_generation() {
        use crate::hex::HexCoord as HC;
        use crate::terrain::TerrainType;
        let center = HexCoord::new(0, 0);
        let mut chunk = MinimapChunk::new(center);
        assert!(!chunk.generated);
        assert!(chunk.texture.is_none());

        // Add some tiles to the chunk
        let tile = crate::world::WorldTile::new(
            HC::new(0, 0),
            0u64,
            TerrainType::Grass,
            0.5,
            crate::world::Vegetation::None,
        );
        chunk.tiles.push(tile);
        chunk.generate_texture();
        assert!(chunk.generated);
        assert!(chunk.texture.is_some());
        let texture = chunk.texture.unwrap();
        assert!(texture.width > 0);
        assert!(texture.height > 0);
        assert_eq!(texture.data.len(), (texture.width * texture.height * 4) as usize);
    }
}

// ============================================================================
// Fog-of-War Vision System
// ============================================================================

/// Configuration for vision-based fog of war.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    /// Radius of player vision in world units.
    pub vision_radius: f32,
    /// Width of the soft gradient edge around the vision circle (world units).
    pub border_softness: f32,
    /// Fog color and opacity as RGBA bytes.
    pub fog_color: [u8; 4],
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            vision_radius: 1500.0,
            border_softness: 300.0,
            fog_color: [10, 12, 16, 220],
        }
    }
}

impl VisionConfig {
    /// Total radius within which tiles are rendered (vision + soft border).
    pub fn render_radius(&self) -> f32 {
        self.vision_radius + self.border_softness
    }

    /// Check if a world position is within the hard vision boundary.
    pub fn is_visible(&self, world_pos: (f32, f32), player_pos: (f32, f32)) -> bool {
        let dx = world_pos.0 - player_pos.0;
        let dy = world_pos.1 - player_pos.1;
        dx * dx + dy * dy <= self.vision_radius * self.vision_radius
    }

    /// Check if a world position is within the render radius (visible + border).
    pub fn is_renderable(&self, world_pos: (f32, f32), player_pos: (f32, f32)) -> bool {
        let dx = world_pos.0 - player_pos.0;
        let dy = world_pos.1 - player_pos.1;
        dx * dx + dy * dy <= self.render_radius() * self.render_radius()
    }

    /// Compute the dimming alpha for a position at the given distance from the player.
    /// Returns 1.0 for fully visible, <1.0 for partially fogged, 0.0 for not rendered.
    pub fn distance_to_alpha(&self, dist_sq: f32) -> f32 {
        let vision_sq = self.vision_radius * self.vision_radius;
        let render_sq = self.render_radius() * self.render_radius();

        if dist_sq <= vision_sq {
            1.0
        } else if dist_sq <= render_sq {
            let t = (dist_sq - vision_sq) / (render_sq - vision_sq);
            1.0 - t * 0.6
        } else {
            0.0
        }
    }
}

// ============================================================================
// Rotation Mode
// ============================================================================

/// Rotation mode for the minimap display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RotationMode {
    /// North is always at the top of the minimap.
    /// The player marker rotates to indicate facing direction.
    #[default]
    NorthUp,
    /// The player always faces toward the top of the minimap.
    /// The map rotates around the player.
    PlayerUp,
}

impl RotationMode {
    /// Toggle between rotation modes.
    pub fn toggle(self) -> Self {
        match self {
            Self::NorthUp => Self::PlayerUp,
            Self::PlayerUp => Self::NorthUp,
        }
    }

    /// Returns the rotation angle (in radians) to apply to the map content.
    /// For `NorthUp`, the map does not rotate (angle = 0).
    /// For `PlayerUp`, the map rotates so the player's facing direction points up.
    /// `facing_angle` is the player's world-facing angle in radians (0 = +x axis).
    pub fn map_rotation(self, facing_angle: f32) -> f32 {
        match self {
            Self::NorthUp => 0.0,
            // Map rotates opposite to marker: -(facing + π/2) so the player's
            // facing direction maps to "up" (north) on screen.
            Self::PlayerUp => -(facing_angle + std::f32::consts::FRAC_PI_2),
        }
    }

    /// Returns the rotation angle for the player marker.
    /// For `NorthUp`, the marker rotates to show facing.
    /// For `PlayerUp`, the marker always points up (angle = 0).
    pub fn marker_rotation(self, facing_angle: f32) -> f32 {
        match self {
            Self::NorthUp => facing_angle + std::f32::consts::FRAC_PI_2,
            Self::PlayerUp => 0.0,
        }
    }
}

// ============================================================================
// Waypoint
// ============================================================================

/// A waypoint placed by the player on the minimap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapWaypoint {
    /// Hex coordinate of the waypoint.
    pub hex: HexCoord,
    /// World position (x, y) of the waypoint.
    pub position: (f32, f32),
    /// Optional label for the waypoint.
    pub label: Option<String>,
}

impl MinimapWaypoint {
    /// Create a new waypoint at the given world position.
    pub fn new(world_pos: (f32, f32)) -> Self {
        use crate::hex::world_pos_to_hex;
        use crate::world::HEX_SIZE;
        let (q, r) = world_pos_to_hex(world_pos.0, world_pos.1, HEX_SIZE);
        let hex = HexCoord::new(q, r);
        Self {
            hex,
            position: world_pos,
            label: None,
        }
    }
}

// ============================================================================
// Coordinate Conversion (Player-Centered)
// ============================================================================

/// Convert a world position to a minimap screen pixel position (player-centered).
///
/// - `world_pos`: the world (x, y) position of the tile
/// - `player_pos`: the world (x, y) position of the player
/// - `pixel_scale`: pixels per world unit (includes zoom)
/// - `mm_center`: the center of the minimap in pixels (usually size/2)
/// - `rotation`: rotation angle in radians to apply to the map (for PlayerUp mode)
pub fn world_to_map_pixel(
    world_pos: (f32, f32),
    player_pos: (f32, f32),
    pixel_scale: f32,
    mm_center: f32,
    rotation: f32,
) -> (f32, f32) {
    let dx = (world_pos.0 - player_pos.0) * pixel_scale;
    let dy = (world_pos.1 - player_pos.1) * pixel_scale;

    if rotation.abs() > 1e-6 {
        let cos = rotation.cos();
        let sin = rotation.sin();
        (
            mm_center + dx * cos - dy * sin,
            mm_center + dx * sin + dy * cos,
        )
    } else {
        (mm_center + dx, mm_center + dy)
    }
}

/// Convert a minimap screen pixel to a world position (player-centered).
/// Inverse of [`world_to_map_pixel`].
pub fn map_pixel_to_world(
    pixel: (f32, f32),
    player_pos: (f32, f32),
    pixel_scale: f32,
    mm_center: f32,
    rotation: f32,
) -> (f32, f32) {
    let px = pixel.0 - mm_center;
    let py = pixel.1 - mm_center;

    if rotation.abs() > 1e-6 {
        let cos = rotation.cos();
        let sin = rotation.sin();
        // Inverse rotation (transpose)
        let wx = px * cos + py * sin;
        let wy = -px * sin + py * cos;
        (player_pos.0 + wx / pixel_scale, player_pos.1 + wy / pixel_scale)
    } else {
        (
            player_pos.0 + px / pixel_scale,
            player_pos.1 + py / pixel_scale,
        )
    }
}

// ============================================================================
// Tests for Fog-of-War System
// ============================================================================

#[cfg(test)]
mod fov_tests {
    use super::*;

    #[test]
    fn test_vision_config_defaults() {
        let cfg = VisionConfig::default();
        assert_eq!(cfg.vision_radius, 1500.0);
        assert_eq!(cfg.border_softness, 300.0);
        assert_eq!(cfg.render_radius(), 1800.0);
    }

    #[test]
    fn test_is_visible_within_radius() {
        let cfg = VisionConfig::default();
        // Player at origin, tile at (0, 0)
        assert!(cfg.is_visible((0.0, 0.0), (0.0, 0.0)));
        // Tile 1000 units away (within 1500 vision)
        assert!(cfg.is_visible((1000.0, 0.0), (0.0, 0.0)));
        // Tile exactly at vision boundary
        assert!(cfg.is_visible((1500.0, 0.0), (0.0, 0.0)));
    }

    #[test]
    fn test_is_visible_outside_radius() {
        let cfg = VisionConfig::default();
        // Tile 2000 units away (outside 1500 vision)
        assert!(!cfg.is_visible((2000.0, 0.0), (0.0, 0.0)));
        // Tile at render radius — still not "visible" (fogged)
        assert!(!cfg.is_visible((1800.0, 0.0), (0.0, 0.0)));
    }

    #[test]
    fn test_is_renderable_includes_border() {
        let cfg = VisionConfig::default();
        // Tile within vision — renderable
        assert!(cfg.is_renderable((1000.0, 0.0), (0.0, 0.0)));
        // Tile in border region — renderable (dimmed)
        assert!(cfg.is_renderable((1600.0, 0.0), (0.0, 0.0)));
        // Tile beyond render radius — not renderable
        assert!(!cfg.is_renderable((2000.0, 0.0), (0.0, 0.0)));
    }

    #[test]
    fn test_distance_to_alpha_full() {
        let cfg = VisionConfig::default();
        // Well within vision
        let alpha = cfg.distance_to_alpha(500_000.0); // 500 units from center
        assert!((alpha - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_distance_to_alpha_diminished() {
        let cfg = VisionConfig::default();
        // In the border region — should be between 0.4 and 1.0
        let vision_sq = 1500.0 * 1500.0;
        let render_sq = 1800.0 * 1800.0;
        let dist_sq = vision_sq + (render_sq - vision_sq) * 0.5; // halfway through border
        let alpha = cfg.distance_to_alpha(dist_sq);
        assert!(alpha > 0.4 && alpha < 1.0);
    }

    #[test]
    fn test_distance_to_alpha_zero_outside_render() {
        let cfg = VisionConfig::default();
        // Beyond render radius
        let alpha = cfg.distance_to_alpha(2000.0 * 2000.0);
        assert!((alpha - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_rotation_mode_toggle() {
        let mode = RotationMode::NorthUp;
        assert_eq!(mode.toggle(), RotationMode::PlayerUp);
        assert_eq!(mode.toggle().toggle(), RotationMode::NorthUp);
    }

    #[test]
    fn test_rotation_mode_map_rotation_north_up() {
        let mode = RotationMode::NorthUp;
        let facing = 0.0; // facing east
        assert!((mode.map_rotation(facing) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_mode_map_rotation_player_up() {
        let mode = RotationMode::PlayerUp;
        // Player faces east (0) → map should rotate to bring east to top
        // map_rotation = -(0 + π/2) = -π/2
        let facing = 0.0;
        let expected = -(std::f32::consts::FRAC_PI_2);
        let result = mode.map_rotation(facing);
        assert!((result - expected).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_mode_marker_north_up() {
        let mode = RotationMode::NorthUp;
        // Player faces east (0) → marker rotates to point right
        // marker_rotation = 0 + π/2 = π/2
        let facing = 0.0;
        let result = mode.marker_rotation(facing);
        assert!((result - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_mode_marker_player_up() {
        let mode = RotationMode::PlayerUp;
        // In player-up mode, marker always points up (no rotation)
        let facing = 1.5;
        let result = mode.marker_rotation(facing);
        assert!(result.abs() < 1e-6);
    }

    #[test]
    fn test_rotation_mode_map_rotation_north_up_zero() {
        let mode = RotationMode::NorthUp;
        // NorthUp always has zero map rotation regardless of facing
        assert!((mode.map_rotation(0.0) - 0.0).abs() < 1e-6);
        assert!((mode.map_rotation(1.5) - 0.0).abs() < 1e-6);
        assert!((mode.map_rotation(-2.3) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_world_to_map_pixel_no_rotation() {
        // Player at (100, 200), tile at (100, 200) → center of 200px minimap
        let (px, py) = world_to_map_pixel((100.0, 200.0), (100.0, 200.0), 0.1, 100.0, 0.0);
        assert!((px - 100.0).abs() < 0.01);
        assert!((py - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_world_to_map_pixel_offset() {
        // Player at (0, 0), tile at (1000, 0) → shifted right
        let (px, py) = world_to_map_pixel((1000.0, 0.0), (0.0, 0.0), 0.1, 100.0, 0.0);
        assert!((px - 200.0).abs() < 0.01); // 100 + 1000*0.1 = 200
        assert!((py - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_map_pixel_to_world_roundtrip() {
        let player_pos = (500.0, 300.0);
        let world_pos = (1000.0, 700.0);
        
        let (px, py) = world_to_map_pixel(world_pos, player_pos, 0.1, 100.0, 0.0);
        let (wx, wy) = map_pixel_to_world((px, py), player_pos, 0.1, 100.0, 0.0);
        
        assert!((wx - world_pos.0).abs() < 0.01);
        assert!((wy - world_pos.1).abs() < 0.01);
    }

    #[test]
    fn test_map_pixel_to_world_roundtrip_with_rotation() {
        let player_pos = (0.0, 0.0);
        let world_pos = (500.0, 200.0);
        let rotation = 0.5; // ~28.6 degrees
        
        let (px, py) = world_to_map_pixel(world_pos, player_pos, 0.1, 100.0, rotation);
        let (wx, wy) = map_pixel_to_world((px, py), player_pos, 0.1, 100.0, rotation);
        
        assert!((wx - world_pos.0).abs() < 0.01, "x: {} vs {}", wx, world_pos.0);
        assert!((wy - world_pos.1).abs() < 0.01, "y: {} vs {}", wy, world_pos.1);
    }

    #[test]
    fn test_waypoint_creation() {
        let wp = MinimapWaypoint::new((150.0, 300.0));
        assert_eq!(wp.position, (150.0, 300.0));
        assert!(wp.label.is_none());
    }
}

    #[test]
    fn map_pixel_world_roundtrip() {
        let world = (123.5, -45.25);
        let panel = (320.0, 240.0);
        for rot in [0.0, 0.7, -1.2, 3.14159] {
            for zoom in [1.0, 2.5, 10.0] {
                let px = world_to_map_pixel(world, panel, zoom, 200.0, rot);
                let back = map_pixel_to_world(px, panel, zoom, 200.0, rot);
                assert!(
                    (back.0 - world.0).abs() < 1e-2 && (back.1 - world.1).abs() < 1e-2,
                    "rot={rot} zoom={zoom} gave {back:?}"
                );
            }
        }
    }
