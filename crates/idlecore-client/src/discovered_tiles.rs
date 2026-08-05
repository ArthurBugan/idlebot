//! Discovered tiles — fog of war / explored areas.
//!
//! Uses chunk-based spatial indexing so discovery queries are O(chunks_seen)
//! instead of O(world_size). Each discovered chunk stores the set of tile IDs
//! it contains, so querying by world position only checks a few chunks.

use bevy::prelude::*;
use std::collections::HashMap;

/// Chunk size for spatial indexing (in world units).
const CHUNK_SIZE: f32 = 1500.0;

/// Discovered tiles with chunk-based spatial indexing.
#[derive(Resource, Debug, Clone)]
pub struct DiscoveredTiles {
    /// Set of discovered tile hex IDs.
    pub tiles: std::collections::HashSet<u64>,
    /// Map of chunk coordinate → discovered tile hex IDs in that chunk.
    pub chunks: HashMap<(i32, i32), std::collections::HashSet<u64>>,
    /// Max discovery radius around player (world units).
    pub discovery_radius: f32,
}

impl DiscoveredTiles {
    /// Convert a world position to a chunk coordinate.
    fn world_to_chunk(world_x: f32, world_y: f32) -> (i32, i32) {
        let cx = (world_x / CHUNK_SIZE).floor() as i32;
        let cy = (world_y / CHUNK_SIZE).floor() as i32;
        (cx, cy)
    }

    /// Mark a tile as discovered, indexing it into its chunk.
    pub fn discover_tile(&mut self, tile_id: u64, tile_x: f32, tile_y: f32) {
        let chunk = Self::world_to_chunk(tile_x, tile_y);
        self.chunks.entry(chunk).or_default().insert(tile_id);
        self.tiles.insert(tile_id);
    }

    /// Check if a tile is discovered.
    pub fn is_discovered(&self, tile_id: u64) -> bool {
        self.tiles.contains(&tile_id)
    }

    /// Get the number of discovered tiles.
    pub fn count(&self) -> usize {
        self.tiles.len()
    }

    /// Clear all discovered tiles.
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.chunks.clear();
    }

    /// Get nearby discovered tiles for a given center position and radius.
    /// This is O(1) because we only check a few chunks around the center.
    pub fn nearby_discovered(&self, center_x: f32, center_y: f32, radius: f32) -> Vec<u64> {
        let mut result = Vec::new();
        let min_cx = Self::world_to_chunk(center_x - radius, center_y - radius).0;
        let max_cx = Self::world_to_chunk(center_x + radius, center_y + radius).0;
        let min_cy = Self::world_to_chunk(center_x - radius, center_y - radius).1;
        let max_cy = Self::world_to_chunk(center_x + radius, center_y + radius).1;

        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(tiles) = self.chunks.get(&(cx, cy)) {
                    result.extend(tiles.iter().cloned());
                }
            }
        }

        result
    }
}

impl Default for DiscoveredTiles {
    fn default() -> Self {
        Self {
            tiles: std::collections::HashSet::new(),
            chunks: HashMap::new(),
            discovery_radius: 200.0,
        }
    }
}
