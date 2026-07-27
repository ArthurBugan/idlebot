//! Hex tile data structures — core types (no Bevy dependency here).
//!
//! `HexTileComponent` is a Bevy component for rendering.
//! `HexTileData` is the core data shared with the server.

use crate::terrain::TerrainType;

/// Core hex tile data (shared between core, server, client).
#[derive(Debug, Clone, Copy)]
pub struct HexTileData {
    /// Axial coordinate as a u64 id
    pub hex_id: u64,
    /// Axial coordinate (q, r)
    pub coord: (i32, i32),
    /// Center position in world coordinates
    pub center_x: f32,
    pub center_y: f32,
    /// Terrain type
    pub terrain: TerrainType,
    /// Eco rating (0-100)
    pub eco_rating: u32,
    /// Which player owns this tile (None = unowned)
    pub owned_by: Option<u64>,
    /// Whether a plant exists on this hex
    pub has_plant: bool,
    /// Whether this hex is polluted
    pub has_pollution: bool,
}

impl HexTileData {
    /// Create a new hex tile with the given terrain.
    pub fn new(hex_id: u64, coord: (i32, i32), terrain: TerrainType) -> Self {
        let (q, r) = coord;
        let s = -q - r;
        let hex_radius = 10.0f32;
        let sqrt3 = f32::sqrt(3.0);

        // Convert to pixel coordinates (flat-top hex)
        let x = hex_radius * sqrt3 * (q as f32 + r as f32 / 2.0);
        let y = hex_radius * 1.5 * r as f32;

        Self {
            hex_id,
            coord,
            center_x: x,
            center_y: y,
            terrain,
            eco_rating: crate::terrain::eco_rating(&terrain),
            owned_by: None,
            has_plant: false,
            has_pollution: false,
        }
    }

    /// Reset the tile to its default state.
    pub fn reset(&mut self) {
        self.owned_by = None;
        self.has_plant = false;
        self.has_pollution = false;
    }

    /// Set the plant owner.
    pub fn set_plant_owner(&mut self, player_id: u64) {
        self.has_plant = true;
        self.owned_by = Some(player_id);
    }

    /// Clear the plant.
    pub fn clear_plant(&mut self) {
        self.has_plant = false;
        self.owned_by = None;
    }

    /// Set pollution.
    pub fn set_polluted(&mut self) {
        self.has_pollution = true;
    }

    /// Clean pollution.
    pub fn clean_pollution(&mut self) {
        self.has_pollution = false;
    }
}

/// Bevy component attached to each hex entity in the grid.
#[derive(Debug, Clone, Copy)]
pub struct HexTileComponent {
    pub pos: [f32; 3],
    pub terrain: TerrainType,
    pub eco_rating: u32,
}

impl HexTileComponent {
    /// Create a Bevy component from a core HexTileData.
    pub fn from_data(data: &HexTileData) -> Self {
        Self {
            pos: [data.center_x, data.center_y, 0.0],
            terrain: data.terrain,
            eco_rating: data.eco_rating,
        }
    }
}
