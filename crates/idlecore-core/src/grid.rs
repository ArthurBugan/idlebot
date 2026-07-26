//! Hex grid system — generates and manages the hex tile world.
//! Bevy 0.19 — flat-top hexagons with terrain colors.

use bevy::prelude::*;
use std::collections::HashMap;
use rand::SeedableRng;

use crate::hex::Hex;
use crate::hex_tile::HexTile;
use crate::terrain::TerrainType;

/// All hexes stored by hex_id (u64).
#[derive(Component, Default)]
pub struct WorldGrid {
    pub hexes: HashMap<u64, HexTile>,
}

impl WorldGrid {
    /// Generate a new world grid
    pub fn generate(hex_radius: f32, map_radius: i32, seed: u64) -> Self {
        use rand::Rng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut grid = Self::default();

        for q in -map_radius..=map_radius {
            for r in -map_radius..=map_radius {
                let s = -q - r;
                if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                    let hex_id = (q as u64).wrapping_shl(32) | (r as u64 & 0xFFFFFFFF);
                    let hex = Hex::new(q, r);
                    let center = hex.center(hex_radius);

                    let terrain = TerrainType::from_random(&mut rng);
                    let tile = HexTile::new(center, terrain);

                    grid.hexes.insert(hex_id, tile);
                }
            }
        }

        grid
    }

    /// Get a hex tile by ID
    pub fn get(&self, hex_id: u64) -> Option<&HexTile> {
        self.hexes.get(&hex_id)
    }

    /// Get the number of hexes
    pub fn len(&self) -> usize {
        self.hexes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.hexes.is_empty()
    }
}

/// Spawns a full hex grid and bevy entities for each tile.
pub fn spawn_world_grid(
    hex_radius: f32,
    map_radius: i32,
    seed: u64,
    mut commands: Commands,
    mut grid: Local<WorldGrid>,
) {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);

    let mut hex_entities = Vec::new();

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let s = -q - r;
            if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                let hex_id = (q as u64).wrapping_shl(32) | (r as u64 & 0xFFFFFFFF);
                let hex = Hex::new(q, r);
                let center = hex.center(hex_radius);

                let terrain = TerrainType::from_random(&mut rng);
                let tile = HexTile::new(center, terrain);

                grid.hexes.insert(hex_id, tile.clone());

                let entity = commands
                    .spawn((
                        Name::new(format!("hex_{q}_{r}")),
                        tile,
                        Transform::from_xyz(center.x, center.y, 0.0),
                        Visibility::default(),
                    ))
                    .id();

                hex_entities.push(entity);
            }
        }
    }

    tracing::info!("Spawned {} hex entities", hex_entities.len());
}
