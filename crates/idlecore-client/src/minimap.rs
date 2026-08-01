//! Minimap -- 2D hex overlay showing local/mid/global view ranges.
//!
//! Ponytail: Simple 2D component using Bevy's Sprite system. No 3D dependency.
//! Renders dark background + player dot + hex circles for visibility zone.

use bevy::prelude::*;
use idlecore_core::hex::HexCoord;
use idlecore_core::terrain::TerrainType;

// ---------------------------------------------------------------------------
// Minimap Zoom
// ---------------------------------------------------------------------------

/// Zoom levels for the minimap viewport radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum MinimapZoom {
    /// Local -- 5-hex radius (closest view)
    Local,
    /// Mid -- 20-hex radius
    Mid,
    /// Global -- 64-hex radius (full map)
    Global,
}

// ---------------------------------------------------------------------------
// Minimap Data Model
// ---------------------------------------------------------------------------

/// Marker for an object on the minimap (plant, pollution, hex building).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectMarker {
    /// Hex coordinate of the object.
    pub hex: HexCoord,
    /// Type of object for color coding.
    pub object_type: ObjectType,
    /// Optional label for display.
    pub label: Option<String>,
}

/// Object types that can appear on the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    /// A plant or tree.
    Plant,
    /// Polluted hex.
    Pollution,
    /// Hex has a building/structure.
    Building,
    /// Water body.
    Water,
    /// Desert area.
    Desert,
    /// City/urban area.
    City,
}

/// Combined minimap state: player position, viewport hexes, objects, and markers.
#[derive(Debug, Clone)]
pub struct MinimapData {
    /// Player's current 2D position (world units).
    pub player_position: Vec2,
    /// Hexes currently visible in the viewport.
    pub viewport_hexes: Vec<HexCoord>,
    /// Other players visible within range.
    pub other_players: Vec<(Vec2, String)>,
    /// Object markers on the minimap.
    pub objects: Vec<ObjectMarker>,
    /// Terrain data for viewport hexes.
    pub terrain_map: std::collections::HashMap<HexCoord, TerrainType>,
}

// ---------------------------------------------------------------------------
// Minimap Component
// ---------------------------------------------------------------------------

/// Minimap component attached to the player entity.
/// Renders a 2D overlay showing the visibility zone and player position.
#[derive(Component)]
pub struct MinimapComponent {
    /// Current zoom level (controls viewport radius).
    pub zoom: MinimapZoom,
    /// Player's world-space position in 2D.
    pub player_pos: Vec2,
    /// Cached viewport hexes for rendering.
    pub viewport_hexes: Vec<HexCoord>,
    /// Hex size in world units (used for coordinate conversion).
    pub hex_size: f32,
    /// Screen offset for positioning the minimap (bottom-right corner).
    pub screen_offset: Vec2,
    /// Width of the minimap in pixels.
    pub width: f32,
    /// Height of the minimap in pixels.
    pub height: f32,
    /// Whether the global map is toggled on.
    pub global_map_visible: bool,
    /// Selected destination hex (for teleport UI integration).
    pub selected_hex: Option<HexCoord>,
}

impl MinimapComponent {
    /// Create a new minimap component with Local zoom.
    pub fn new(hex_size: f32, screen_width: f32, screen_height: f32) -> Self {
        let minimap_size = 180.0;
        let screen_offset = Vec2::new(
            screen_width - minimap_size - 10.0,
            10.0,
        );
        Self {
            zoom: MinimapZoom::Local,
            player_pos: Vec2::ZERO,
            viewport_hexes: Vec::new(),
            hex_size,
            screen_offset,
            width: minimap_size,
            height: minimap_size,
            global_map_visible: false,
            selected_hex: None,
        }
    }

    /// Zoom in one level (increase viewport radius).
    pub fn zoom_in(&mut self) {
        self.zoom = match self.zoom {
            MinimapZoom::Global => MinimapZoom::Mid,
            MinimapZoom::Mid => MinimapZoom::Local,
            MinimapZoom::Local => MinimapZoom::Local, // already max
        };
        self.refresh_view();
    }

    /// Zoom out one level (decrease viewport radius).
    pub fn zoom_out(&mut self) {
        self.zoom = match self.zoom {
            MinimapZoom::Local => MinimapZoom::Mid,
            MinimapZoom::Mid => MinimapZoom::Global,
            MinimapZoom::Global => MinimapZoom::Global, // already min
        };
        self.refresh_view();
    }

    /// Set the zoom level directly and refresh.
    pub fn set_zoom(&mut self, zoom: MinimapZoom) {
        self.zoom = zoom;
        self.refresh_view();
    }

    /// Update the player's position on the minimap.
    pub fn set_player_pos(&mut self, pos: Vec2) {
        self.player_pos = pos;
        // Refresh viewport when player moves (if zoom is local or mid)
        if self.zoom != MinimapZoom::Global || !self.global_map_visible {
            self.refresh_view();
        }
    }

    /// Toggle global map visibility.
    pub fn toggle_global_map(&mut self) {
        self.global_map_visible = !self.global_map_visible;
        self.refresh_view();
    }

    /// Set the global map visibility state.
    pub fn set_global_map_visible(&mut self, visible: bool) {
        if self.global_map_visible != visible {
            self.global_map_visible = visible;
            self.refresh_view();
        }
    }

    /// Select a hex on the minimap (for teleport destination).
    pub fn select_hex(&mut self, hex: HexCoord) {
        self.selected_hex = Some(hex);
    }

    /// Clear the selected hex.
    pub fn clear_selection(&mut self) {
        self.selected_hex = None;
    }

    /// Refresh which hexes are visible based on current zoom level.
    pub fn refresh_view(&mut self) {
        let radius = if self.global_map_visible { 64 } else { self.zoom_radius() };
        self.viewport_hexes = self.collect_hexes(radius);
    }

    /// Collect hexes within a hex distance from the player's hex.
    fn collect_hexes(&self, max_distance: i32) -> Vec<HexCoord> {
        let player_hex = self.player_pos_to_hex();
        let mut viewport = Vec::new();

        for dq in -max_distance..=max_distance {
            for dr in -max_distance..=max_distance {
                let hex_q = player_hex.q + dq;
                let hex_r = player_hex.r + dr;
                let hex_s = -(hex_q + hex_r);

                let dist = manhattan_hex_distance(player_hex.q, player_hex.r, hex_q, hex_r);
                if dist <= max_distance {
                    viewport.push(HexCoord::new(hex_q, hex_r));
                }
            }
        }

        viewport
    }

    /// Convert a world position to the nearest hex coord.
    fn player_pos_to_hex(&self) -> HexCoord {
        let q = ((self.player_pos.x / self.hex_size).round()) as i32;
        let r = ((self.player_pos.y / self.hex_size).round()) as i32;
        HexCoord::new(q, r)
    }

    /// Calculate hex distance between two hexes.
    fn manhattan_hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
        let dx = q2 - q1;
        let dy = r2 - r1;
        let dz = -(dx + dy);
        (dx.abs() + dy.abs() + dz.abs()) / 2
    }

    /// Convert a hex coord to a world-space 2D position.
    fn hex_to_world(&self, hex: &HexCoord) -> Vec2 {
        let sqrt3 = f32::sqrt(3.0);
        let q = hex.q;
        let r = hex.r;
        Vec2::new(
            self.hex_size * sqrt3 * (q as f32 + r as f32 / 2.0),
            self.hex_size * 1.5 * r as f32,
        )
    }

    /// Get the zoom radius for the current level.
    pub fn zoom_radius(&self) -> i32 {
        match self.zoom {
            MinimapZoom::Local => 5,
            MinimapZoom::Mid => 20,
            MinimapZoom::Global => 64,
        }
    }

    /// Get terrain color for a hex based on its coordinates.
    pub fn hex_terrain_color(&self, hex: &HexCoord) -> Color {
        let q_abs = hex.q.abs();
        let r_abs = hex.r.abs();
        let sum = q_abs + r_abs;

        // Simple deterministic color based on hex position
        if sum % 5 == 0 && sum > 0 {
            Color::srgb(0.20, 0.50, 0.80) // Water (blue)
        } else if q_abs > 6 && r_abs > 6 {
            Color::srgb(0.90, 0.75, 0.40) // Desert (sand)
        } else if q_abs % 4 == 0 || r_abs % 4 == 0 {
            Color::srgb(0.10, 0.55, 0.20) // Forest (dark green)
        } else if (q_abs + r_abs) % 3 == 0 {
            Color::srgb(0.25, 0.60, 0.30) // Grass (medium green)
        } else {
            Color::srgb(0.35, 0.70, 0.40) // Light grass
        }
    }

    /// Get object marker color.
    pub fn object_color(&self, obj_type: ObjectType) -> Color {
        match obj_type {
            ObjectType::Plant => Color::srgb(0.10, 0.60, 0.10),
            ObjectType::Pollution => Color::srgb(0.50, 0.20, 0.50),
            ObjectType::Building => Color::srgb(0.80, 0.80, 0.30),
            ObjectType::Water => Color::srgb(0.15, 0.45, 0.75),
            ObjectType::Desert => Color::srgb(0.90, 0.75, 0.40),
            ObjectType::City => Color::srgb(0.60, 0.60, 0.60),
        }
    }

    /// Convert world position to minimap screen position.
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        let center = Vec2::new(
            self.screen_offset.x + self.width / 2.0,
            self.screen_offset.y + self.height / 2.0,
        );
        let scale = if self.global_map_visible {
            self.width as f32 / 128.0 * self.hex_size
        } else {
            self.width as f32 / (self.zoom_radius() as f32 * 2.0 + 2.0) * self.hex_size
        };
        center - Vec2::new(world_pos.x / scale, world_pos.y / scale)
    }

    /// Get the selected hex for teleport integration.
    pub fn selected_destination(&self) -> Option<HexCoord> {
        self.selected_hex
    }
}

impl Default for MinimapComponent {
    fn default() -> Self {
        Self::new(10.0, 800.0, 600.0)
    }
}

// ---------------------------------------------------------------------------
// Minimap Rendering Systems
// ---------------------------------------------------------------------------

/// Hex sprite component for minimap rendering.
#[derive(Component)]
pub struct MinimapHexSprite {
    /// Hex coordinate this sprite represents.
    pub hex: HexCoord,
    /// Screen position of the hex center.
    pub screen_pos: Vec2,
    /// Color of the hex based on terrain.
    pub color: Color,
}

/// Player marker component on the minimap.
#[derive(Component)]
pub struct MinimapPlayerMarker {
    /// Player's screen position.
    pub screen_pos: Vec2,
}

/// Object marker component on the minimap.
#[derive(Component)]
pub struct MinimapObjectMarker {
    /// Object type for color coding.
    pub obj_type: ObjectType,
    /// Screen position.
    pub screen_pos: Vec2,
}

/// Selection highlight component for the selected hex.
#[derive(Component)]
pub struct SelectionHighlight {
    /// Screen position of the selected hex.
    pub screen_pos: Vec2,
}

/// Spawn minimap hex sprites based on viewport.
pub fn spawn_minimap_hexes(
    mut commands: Commands,
    minimap: Query<&MinimapComponent>,
) {
    let minimap = minimap.single();
    if minimap.viewport_hexes.is_empty() {
        return;
    }

    // Clear existing hex sprites
    commands.entity(minimap).despawn_descendants();

    for hex in &minimap.viewport_hexes {
        let world_pos = minimap.hex_to_world(hex);
        let screen_pos = minimap.world_to_screen(world_pos);
        let color = minimap.hex_terrain_color(hex);

        commands.spawn((
            Name::new(format!("minimap_hex_{}_{}", hex.q, hex.r)),
            Sprite {
                color,
                custom_size: Some(Vec2::splat(8.0)),
                ..default()
            },
            Transform::from_translation(Vec3::new(screen_pos.x, screen_pos.y, 100.0)),
            MinimapHexSprite {
                hex: *hex,
                screen_pos,
                color,
            },
        ));
    }
}

/// Spawn player marker on minimap.
pub fn spawn_minimap_player(
    mut commands: Commands,
    minimap: Query<(Entity, &MinimapComponent)>,
) {
    let (entity, minimap) = minimap.single();

    // Remove existing player marker
    if let Ok(marker) = minimap.commands().get::<MinimapPlayerMarker>(entity) {
        if let Ok(child) = minimap.commands().get_parent(entity) {
            minimap.commands().despawn(child);
        }
    }

    let player_screen = minimap.world_to_screen(minimap.player_pos);

    commands.spawn((
        Name::new("minimap_player"),
        Sprite {
            color: Color::srgb(0.20, 0.50, 1.00), // Blue
            custom_size: Some(Vec2::splat(10.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(player_screen.x, player_screen.y, 101.0)),
        MinimapPlayerMarker { screen_pos: player_screen },
    ));
}

/// Update minimap hex positions when player moves or zoom changes.
pub fn update_minimap_view(
    mut commands: Commands,
    minimap: Query<(Entity, &MinimapComponent)>,
) {
    let (entity, minimap) = minimap.single();
    let player_hex = minimap.player_pos_to_hex();

    for hex in &minimap.viewport_hexes {
        if hex.q == player_hex.q && hex.r == player_hex.r {
            // This is the player's hex, skip
            continue;
        }

        let world_pos = minimap.hex_to_world(hex);
        let screen_pos = minimap.world_to_screen(world_pos);

        // Update hex sprite position
        if let Ok(mut hex_sprite) = minimap.commands().get::<MinimapHexSprite>(entity) {
            if hex_sprite.hex == *hex {
                hex_sprite.screen_pos = screen_pos;
                let _ = minimap.commands().set_translation(Vec3::new(screen_pos.x, screen_pos.y, 100.0));
            }
        }
    }
}

/// Handle minimap mouse clicks for hex selection.
pub fn handle_minimap_click(
    mut events: EventReader<MouseButtonInput>,
    minimap: Query<&MinimapComponent>,
    mut selected_hex: Local<Option<HexCoord>>,
) {
    let minimap = minimap.single();

    for event in events.read() {
        if event.pressed() {
            // Convert mouse position to minimap space
            // TODO: Get mouse position from input system
            // For now, this is a placeholder
        }
    }

    *selected_hex = minimap.selected_hex;
}

/// System: update minimap viewport when player moves.
pub fn update_minimap_view_system(
    minimap: Query<&MinimapComponent>,
) {
    let minimap = minimap.single();
    // Viewport is already refreshed in set_player_pos
    // This system can be used for periodic updates if needed
    let _ = minimap;
}
