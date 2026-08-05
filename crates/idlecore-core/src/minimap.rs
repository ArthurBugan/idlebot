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
        let center = HexCoord::new(0, 0);
        let mut chunk = MinimapChunk::new(center);
        assert!(!chunk.generated);
        assert!(chunk.texture.is_none());
        chunk.generate_texture(100, 100);
        assert!(chunk.generated);
        assert!(chunk.texture.is_some());
        let texture = chunk.texture.unwrap();
        assert_eq!(texture.width, 100);
        assert_eq!(texture.height, 100);
        assert_eq!(texture.data.len(), 100 * 100 * 4);
    }
}
