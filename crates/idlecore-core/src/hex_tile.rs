//! HexTile component for Bevy entities.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::terrain::TerrainType;

/// ID for players (simplified - in production this would come from SpacetimeDB)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);

/// Component attached to each hex entity in the grid.
#[derive(Component, Clone)]
pub struct HexTile {
    /// Position of the hex center
    pub pos: Vec3,
    /// Terrain type
    pub terrain: TerrainType,
    /// Eco rating (0-100)
    pub eco_rating: u32,
    /// Which player owns this tile (None = unowned)
    pub owned_by: Option<PlayerId>,
    /// Whether a plant exists on this hex
    pub has_plant: bool,
    /// Whether this hex is polluted
    pub has_pollution: bool,
}

impl HexTile {
    /// Create a new hex tile with default values
    pub fn new(pos: Vec3, terrain: TerrainType) -> Self {
        Self {
            pos,
            terrain,
            eco_rating: crate::terrain::eco_rating(&terrain) as u32,
            owned_by: None,
            has_plant: false,
            has_pollution: false,
        }
    }

    /// Reset the tile to its default state
    pub fn reset(&mut self) {
        self.owned_by = None;
        self.has_plant = false;
        self.has_pollution = false;
    }

    /// Set the plant owner
    pub fn set_plant_owner(&mut self, player_id: PlayerId) {
        self.has_plant = true;
        self.owned_by = Some(player_id);
    }

    /// Clear the plant
    pub fn clear_plant(&mut self) {
        self.has_plant = false;
        self.owned_by = None;
    }

    /// Set pollution
    pub fn set_polluted(&mut self) {
        self.has_pollution = true;
    }

    /// Clean pollution
    pub fn clean_pollution(&mut self) {
        self.has_pollution = false;
    }
}
