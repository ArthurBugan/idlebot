//! Persistence — world modifications DB + hierarchical save/load (§23-24, §42, HEX-023/024).
//!
//! Generated terrain is regenerated from the world seed + coordinate. Only
//! *modifications* need to be serialized. This keeps save files tiny even for
//! a world-scale map.

use crate::hex::HexCoord;
use crate::terrain::TerrainType;
use crate::world_gen::{WaterClass, WorldGenConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single persistent modification to the world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldModification {
    pub coord: HexCoord,
    pub kind: ModificationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModificationKind {
    /// Overwrite terrain type.
    SetTerrain(TerrainType),
    /// Overwrite water class.
    SetWater(WaterClass),
    /// Toggle a flag bit on the cell.
    SetFlags(u8),
    /// Mark a resource as depleted.
    RemoveResource,
    /// Construct a road in this hex.
    BuildRoad,
    /// Construct a settlement in this hex.
    BuildSettlement,
}

impl WorldModification {
    /// Apply the modification to a generated base cell, returning the modified cell.
    pub fn apply(&self, base: &crate::world_gen::HexCell) -> crate::world_gen::HexCell {
        let mut cell = *base;
        match self.kind {
            ModificationKind::SetTerrain(t) => cell.terrain = t,
            ModificationKind::SetWater(w) => cell.water = w,
            ModificationKind::SetFlags(f) => cell.flags = f,
            ModificationKind::RemoveResource => cell.flags &= !crate::world_gen::HexCell::FLAG_RESOURCE,
            ModificationKind::BuildSettlement => cell.flags |= crate::world_gen::HexCell::FLAG_SETTLEMENT,
            ModificationKind::BuildRoad => cell.flags |= crate::world_gen::HexCell::FLAG_ROAD,
        }
        cell
    }
}

/// Database of modifications keyed by hex id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldModDB {
    pub modifications: HashMap<u64, WorldModification>,
}

impl WorldModDB {
    pub fn new() -> Self {
        Self { modifications: HashMap::new() }
    }

    /// Record a modification for a coordinate.
    pub fn apply(&mut self, coord: HexCoord, kind: ModificationKind) {
        let id = crate::world_gen::HexCell::id_of(coord.q, coord.r);
        self.modifications.insert(id, WorldModification { coord, kind });
    }

    /// Get all modifications in a region (for chunk/save streaming).
    pub fn modifications_in(&self, min_q: i32, max_q: i32, min_r: i32, max_r: i32) -> Vec<&WorldModification> {
        self.modifications
            .values()
            .filter(|m| {
                m.coord.q >= min_q && m.coord.q <= max_q && m.coord.r >= min_r && m.coord.r <= max_r
            })
            .collect()
    }

    /// Number of modifications.
    pub fn len(&self) -> usize {
        self.modifications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modifications.is_empty()
    }

    /// Compute the final cell for a coordinate: base generation + mods.
    pub fn cell_for(&self, config: &WorldGenConfig, q: i32, r: i32) -> crate::world_gen::HexCell {
        let mut cell = config.generate_hex(q, r);
        let id = crate::world_gen::HexCell::id_of(q, r);
        if let Some(m) = self.modifications.get(&id) {
            cell = m.apply(&cell);
        }
        cell
    }
}

// ============================================================================
// Save File Format (§42)
// ============================================================================

/// The top-level world save file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSave {
    pub format_version: u32,
    /// The generation seed (regenerates terrain deterministically).
    pub world_seed: u64,
    /// Global world state (e.g., world radius).
    pub world_radius: i32,
    /// Segment-level metadata (compact).
    pub segments: Vec<SegmentSave>,
    /// Chunk modifications (serialized compactly).
    pub mod_db: WorldModificationDb,
    /// Player state.
    pub player: PlayerState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerState {
    pub position: (f32, f32),
    pub facing_angle: f32,
    pub controlled_chunks: Vec<(i32, i32)>,
}

/// Per-segment saved metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSave {
    pub id: u64,
    pub dominant_biome: u16,
    pub water_percentage: f32,
    pub mod_count: u32,
}

/// Named alias so WorldSave uses one DB type consistently.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldModificationDb {
    pub items: HashMap<u64, RawMod>,
}

/// Raw serializable form of a modification (engine-agnostic).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RawMod {
    pub q: i32,
    pub r: i32,
    pub kind: ModificationKind,
}

impl WorldModificationDb {
    pub fn from_db(db: &WorldModDB) -> Self {
        Self {
            items: db
                .modifications
                .iter()
                .map(|(id, m)| {
                    (*id, RawMod { q: m.coord.q, r: m.coord.r, kind: m.kind })
                })
                .collect(),
        }
    }

    pub fn into_db(&self) -> WorldModDB {
        let mut db = WorldModDB::new();
        for raw in self.items.values() {
            db.apply(HexCoord::new(raw.q, raw.r), raw.kind);
        }
        db
    }
}

impl WorldSave {
    /// Create a save from a world generator config + modification DB + player.
    pub fn from_world(
        config: &WorldGenConfig,
        db: &WorldModDB,
        player: PlayerState,
    ) -> Self {
        Self {
            format_version: 1,
            world_seed: config.seed,
            world_radius: config.world_radius,
            segments: Vec::new(),
            mod_db: WorldModificationDb::from_db(db),
            player,
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

impl Default for WorldSave {
    fn default() -> Self {
        Self {
            format_version: 1,
            world_seed: WorldGenConfig::default().seed,
            world_radius: WorldGenConfig::default().world_radius,
            segments: Vec::new(),
            mod_db: WorldModificationDb::default(),
            player: PlayerState::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_db_apply_and_retrieve() {
        let mut db = WorldModDB::new();
        db.apply(HexCoord::new(3, -2), ModificationKind::BuildRoad);
        assert_eq!(db.len(), 1);
        let in_region = db.modifications_in(-10, 10, -10, 10);
        assert_eq!(in_region.len(), 1);
    }

    #[test]
    fn mod_db_cell_contains_mod() {
        let config = WorldGenConfig::default();
        let mut db = WorldModDB::new();
        let base = config.generate_hex(3, -2);
        assert!(!base.has_road());
        db.apply(HexCoord::new(3, -2), ModificationKind::BuildRoad);
        let cell = db.cell_for(&config, 3, -2);
        assert!(cell.has_road());
    }

    #[test]
    fn save_roundtrip_json() {
        let config = WorldGenConfig { seed: 7, world_radius: 50, flat: false };
        let mut db = WorldModDB::new();
        db.apply(HexCoord::new(1, 1), ModificationKind::SetTerrain(TerrainType::Mountain));
        let save = WorldSave::from_world(&config, &db, PlayerState { position: (10.0, 20.0), ..Default::default() });
        let json = save.to_json();
        assert!(!json.is_empty());
        let loaded = WorldSave::from_json(&json);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.world_seed, 7);
        assert_eq!(loaded.world_radius, 50);
        assert_eq!(loaded.player.position, (10.0, 20.0));
        assert_eq!(loaded.mod_db.items.len(), 1);
        // Rebuild mod db and verify the mod applies
        let rebuilt = loaded.mod_db.into_db();
        let cell = rebuilt.cell_for(&config, 1, 1);
        assert_eq!(cell.terrain, TerrainType::Mountain);
    }
}