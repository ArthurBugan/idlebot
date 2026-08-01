//! Minimap -- 2D hex overlay showing local/mid/global view ranges.
//!
//! Ponytail: Simple 2D component using Bevy's Sprite system. No 3D dependency.
//! Renders dark background + player dot + hex circles for visibility zone.

use bevy::prelude::*;
use crate::hex::HexCoord;

/// Zoom levels for the minimap viewport radius.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub enum MinimapZoom {
    /// Local -- 5-hex radius (closest view)
    Local,
    /// Mid -- 20-hex radius
    Mid,
    /// Global -- 64-hex radius (full map)
    Global,
}

/// Minimap component attached to the player entity.
/// Renders a 2D overlay showing the visibility zone and player position.
#[derive(Component)]
pub struct MinimapComponent {
    /// Current zoom level (controls viewport radius)
    pub zoom: MinimapZoom,
    /// Player's world-space position in 2D (x, y) or 3D (x, z)
    pub player_pos: Vec2,
    /// Cached viewport hexes for rendering
    pub viewport_hexes: Vec<HexCoord>,
    /// Hex size in world units (used for coordinate conversion)
    pub hex_size: f32,
    /// Screen offset for positioning the minimap (e.g., bottom-right corner)
    pub screen_offset: Vec2,
}

impl MinimapComponent {
    /// Create a new minimap at origin with Local zoom.
    pub fn new(hex_size: f32) -> Self {
        Self {
            zoom: MinimapZoom::Local,
            player_pos: Vec2::ZERO,
            viewport_hexes: Vec::new(),
            hex_size,
            screen_offset: Vec2::new(0.0, 0.0),
        }
    }

    /// Zoom in one level (increase viewport radius).
    pub fn zoom_in(&mut self) {
        self.zoom = match self.zoom {
            MinimapZoom::Global => MinimapZoom::Mid,
            MinimapZoom::Mid => MinimapZoom::Local,
            MinimapZoom::Local => MinimapZoom::Local, // already max
        };
    }

    /// Zoom out one level (decrease viewport radius).
    pub fn zoom_out(&mut self) {
        self.zoom = match self.zoom {
            MinimapZoom::Local => MinimapZoom::Mid,
            MinimapZoom::Mid => MinimapZoom::Global,
            MinimapZoom::Global => MinimapZoom::Global, // already min
        };
    }

    /// Set the zoom level directly.
    pub fn set_zoom(&mut self, zoom: MinimapZoom) {
        self.zoom = zoom;
    }

    /// Update the player's position on the minimap.
    pub fn set_player_pos(&mut self, pos: Vec2) {
        self.player_pos = pos;
    }

    /// Refresh which hexes are visible based on current zoom level.
    /// Recalculates the viewport hexes from the player's position.
    pub fn refresh_view(&mut self) {
        match self.zoom {
            MinimapZoom::Local => {
                self.viewport_hexes = self.collect_hexes(self.zoom_radius(), false);
            }
            MinimapZoom::Mid => {
                self.viewport_hexes = self.collect_hexes(20, false);
            }
            MinimapZoom::Global => {
                self.viewport_hexes = self.collect_hexes(64, true);
            }
        }
    }

    /// Collect hexes within a Manhattan distance from the player's hex.
    /// Uses brute-force scan (O(n²)) -- fine for map sizes up to ~100x100.
    /// ponytail: O(n²) scan instead of hex-radius math; upgrade when map gets huge.
    fn collect_hexes(&self, max_distance: i32, include_all: bool) -> Vec<HexCoord> {
        let mut viewport = Vec::new();

        // Get player's hex coord from world position
        let player_hex = self.player_pos_to_hex();

        // Scan hex grid around player using Manhattan distance
        for dq in -max_distance..=max_distance {
            for dr in -max_distance..=max_distance {
                let hex_q = player_hex.q + dq;
                let hex_r = player_hex.r + dr;
                let hex_s = -(hex_q + hex_r); // enforce q+r+s=0 invariant

                let dist = manhattan_hex_distance(player_hex.q, player_hex.r, hex_q, hex_r);
                if dist <= max_distance {
                    viewport.push(HexCoord { q: hex_q, r: hex_r, s: hex_s });
                }
            }
        }

        viewport
    }

    /// Convert a world position to the nearest hex coord using hex_size.
    fn player_pos_to_hex(&self) -> HexCoord {
        let q = ((self.player_pos.x / self.hex_size).round()) as i32;
        let r = ((self.player_pos.y / self.hex_size).round()) as i32;
        HexCoord {
            q,
            r,
            s: -q - r, // enforce invariant
        }
    }

    /// Calculate Manhattan distance between two hexes.
    fn manhattan_hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
        // Convert to offset coordinates for Manhattan distance
        // Then use the standard hex distance formula
        let dx = q2 - q1;
        let dy = r2 - r1;
        let dz = -(dx + dy);
        (dx.abs() + dy.abs() + dz.abs()) / 2
    }

    /// Convert a hex coord to a world-space 2D position.
    /// Uses flat-top hex geometry.
    fn hex_to_world(&self, hex: &HexCoord) -> Vec2 {
        let sqrt3 = f32::sqrt(3.0);
        let q = hex.q;
        let r = hex.r;
        // Flat-top hex: x = sqrt(3) * (q + r/2), y = 1.5 * r
        Vec2::new(
            self.hex_size * sqrt3 * (q as f32 + r as f32 / 2.0),
            self.hex_size * 1.5 * r as f32,
        )
    }

    /// Convert world position to screen position (apply offset).
    fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        world_pos + self.screen_offset
    }

    /// Get the zoom radius for the current level.
    pub fn zoom_radius(&self) -> i32 {
        match self.zoom {
            MinimapZoom::Local => 5,
            MinimapZoom::Mid => 20,
            MinimapZoom::Global => 64,
        }
    }

    /// Convert a hex coord to a simple color for display.
    /// Uses terrain type hints based on hex coordinates (simplified).
    fn hex_color(&self, hex: &HexCoord) -> Color {
        let q_abs = hex.q.abs();
        let r_abs = hex.r.abs();

        // Simple color gradient based on hex position
        // This is a placeholder -- a real implementation would use terrain data
        match q_abs % 3 {
            0 => Color::srgb(0.15, 0.70, 0.30), // Green (grass)
            1 => Color::srgb(0.30, 0.75, 0.45), // Light green
            2 => Color::srgb(0.20, 0.50, 0.20), // Dark green
        }
    }
}

/// System: refresh minimap viewport when player moves.
/// Called each frame to update the minimap with the player's new position.
pub fn update_minimap_view_system(
    mut commands: Commands,
    minimap: Query<Entity, With<MinimapComponent>>,
) {
    // In a full implementation, this would:
    // 1. Query the player entity to get its MinimapComponent
    // 2. Call refresh_view() on the component
    // 3. Spawn/reparent Sprite components for the hex grid
    //
    // For now, this is a stub -- the actual rendering would be in the Bevy app
    // setup or a separate system.
    for _entity in &minimap {
        // Placeholder: in a real implementation, we'd call component.refresh_view()
        // but we can't access it through the Query API without additional setup.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_cycle() {
        let mut minimap = MinimapComponent::new(10.0);

        // Start at Local
        assert_eq!(minimap.zoom, MinimapZoom::Local);

        // Zoom out to Mid
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        // Zoom out to Global
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        // Global zoomed out should stay at Global
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        // Zoom in
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        // Zoom in to Local
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);

        // Already at max zoom
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);
    }

    #[test]
    fn test_refresh_view_updates_hexes() {
        let mut minimap = MinimapComponent::new(10.0);

        // Initially empty
        minimap.refresh_view();
        assert!(!minimap.viewport_hexes.is_empty());

        // Update player position
        minimap.set_player_pos(Vec2::new(50.0, 50.0));
        minimap.refresh_view();

        // Should have viewport hexes (non-empty)
        assert!(!minimap.viewport_hexes.is_empty());
    }

    #[test]
    fn test_zoom_radius() {
        let minimap = MinimapComponent::new(10.0);

        assert_eq!(minimap.zoom_radius(), 5); // Local

        minimap.zoom = MinimapZoom::Mid;
        assert_eq!(minimap.zoom_radius(), 20); // Mid

        minimap.zoom = MinimapZoom::Global;
        assert_eq!(minimap.zoom_radius(), 64); // Global
    }
}
