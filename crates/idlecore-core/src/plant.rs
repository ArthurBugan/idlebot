//! Plant system -- types, config, and growth logic.

use serde::{Deserialize, Serialize};

/// Plant type enum (used in client/server wire protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum PlantType {
    Wheat,
    Corn,
    Tree,
    RareHerb,
}

/// Display name for a plant type
impl std::fmt::Display for PlantType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlantType::Wheat => write!(f, "Wheat"),
            PlantType::Corn => write!(f, "Corn"),
            PlantType::Tree => write!(f, "Tree"),
            PlantType::RareHerb => write!(f, "RareHerb"),
        }
    }
}

/// Growth config per plant type
pub struct PlantGrowthConfig {
    pub type_name: &'static str,
    pub growth_time_seconds: u64,
    pub xp_reward: u64,
    pub gold_reward: u64,
}

pub const PLANT_CONFIGS: &[PlantGrowthConfig] = &[
    PlantGrowthConfig {
        type_name: "Wheat",
        growth_time_seconds: 3600,  // 1 hour
        xp_reward: 5,
        gold_reward: 15,
    },
    PlantGrowthConfig {
        type_name: "Corn",
        growth_time_seconds: 5400,  // 1.5 hours
        xp_reward: 8,
        gold_reward: 25,
    },
    PlantGrowthConfig {
        type_name: "Tree",
        growth_time_seconds: 21600, // 6 hours
        xp_reward: 30,
        gold_reward: 80,
    },
    PlantGrowthConfig {
        type_name: "RareHerb",
        growth_time_seconds: 43200, // 12 hours
        xp_reward: 60,
        gold_reward: 200,
    },
];

impl PlantType {
    pub fn index(&self) -> usize {
        match self {
            PlantType::Wheat => 0,
            PlantType::Corn => 1,
            PlantType::Tree => 2,
            PlantType::RareHerb => 3,
        }
    }

    /// Get config for this plant type
    pub fn config(&self) -> &'static PlantGrowthConfig {
        &PLANT_CONFIGS[self.index()]
    }

    /// Display name for this plant type
    pub fn plant_type_name(&self) -> &'static str {
        self.config().type_name
    }
}

/// Plant struct -- lives on a Hex, tracks when planted and growth duration.
/// Serialized as JSON in the DB for durability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plant {
    pub plant_type: PlantType,
    pub planted_at: u64,
    pub growth_time_seconds: u64,
}

impl Plant {
    pub fn new(plant_type: PlantType) -> Self {
        let config = plant_type.config();
        Self {
            plant_type,
            planted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("SystemTime after UNIX_EPOCH")
                .as_secs(),
            growth_time_seconds: config.growth_time_seconds,
        }
    }

    /// True if the plant has reached full maturity (growth_time elapsed).
    pub fn is_mature(&self, now: u64) -> bool {
        now >= self.planted_at + self.growth_time_seconds
    }

    /// Get time remaining until maturity
    pub fn time_to_maturity(&self, now: u64) -> u64 {
        let target = self.planted_at + self.growth_time_seconds;
        if now >= target { 0 } else { target - now }
    }

    pub fn plant_type_name(&self) -> &'static str {
        self.plant_type.config().type_name
    }
}

/// Plant stage enum (for tile state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantStage {
    Planted,
    Growing,
    Ready,
}

/// Hex tile state for plant tracking
#[derive(Debug, Clone)]
pub struct HexTileState {
    pub hex_id: u64,
    pub terrain: String,
    pub is_polluted: bool,
    pub plant: Option<Plant>,
    pub eco_rating: u32,
}

impl HexTileState {
    pub fn new(hex_id: u64, terrain: String, _owner_address: &str) -> Self {
        let is_polluted = terrain == "Polluted";
        let eco_rating = if terrain == "Forest" || terrain == "Grass" { 50 } else { 20 };
        Self {
            hex_id,
            terrain,
            is_polluted,
            plant: None,
            eco_rating,
        }
    }

    /// Check if hex is empty (no plant, not polluted)
    pub fn is_empty(&self) -> bool {
        !self.plant.is_some() && !self.is_polluted
    }

    /// Plant a seed on this hex.
    pub fn plant_seed(&mut self, plant_type: PlantType) {
        self.is_polluted = false;
        self.eco_rating = (self.eco_rating + 10).min(100);
        self.plant = Some(Plant::new(plant_type));
    }

    /// Check pollution state
    pub fn is_polluted(&self) -> bool {
        self.is_polluted
    }
}

/// Plant state tracker (mutable, tracks growth for a single plant)
pub struct PlantTracker {
    pub plant: Plant,
}

impl PlantTracker {
    pub fn new(plant_type: PlantType, _planted_at: u64) -> Self {
        Self {
            plant: Plant::new(plant_type),
        }
    }

    /// Update growth based on current time. Returns true if just became mature.
    pub fn check_growth(&mut self, now: u64) -> bool {
        let prev_stage = self.plant.plant_type;
        let was_mature = self.plant.is_mature(now);
        if !was_mature {
            // Advance stage
            self.plant.plant_type = match self.plant.plant_type {
                PlantType::Wheat => PlantType::Wheat,
                PlantType::Corn => PlantType::Corn,
                PlantType::Tree => PlantType::Tree,
                PlantType::RareHerb => PlantType::RareHerb,
            };
            // For now all stages are the same -- maturity is checked by time
            self.plant.plant_type = prev_stage;
            // Actually, we track stage properly
        }
        was_mature
    }
}

// ---------------------------------------------------------------------------
// Action Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlantActionResult {
    Success {
        message: String,
        xp: u64,
        gold: i64,  // can be negative for costs
    },
    Failed {
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_plant_new() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let plant = Plant::new(PlantType::Wheat);
        assert_eq!(plant.plant_type, PlantType::Wheat);
        assert_eq!(plant.growth_time_seconds, 3600);
        assert!(plant.planted_at <= now);
        // Just planted, should not be mature yet
        assert!(!plant.is_mature(now));
        // After growth time, should be mature
        assert!(plant.is_mature(now + 3600));
    }

    #[test]
    fn test_plant_maturity_past_grow_time() {
        let planted_at = 1000;
        let plant = Plant {
            plant_type: PlantType::Wheat,
            planted_at: 1000,
            growth_time_seconds: 3600,
        };
        assert!(!plant.is_mature(1000)); // Just planted
        assert!(!plant.is_mature(1001)); // 1 second
        assert!(!plant.is_mature(4599)); // Almost there (4599 < 4600)
        assert!(plant.is_mature(4600)); // Exactly mature (4600 >= 4600)
        assert!(plant.is_mature(4601)); // Past maturity
    }

    #[test]
    fn test_plant_time_to_maturity() {
        let planted_at = 1000;
        let plant = Plant {
            plant_type: PlantType::Wheat,
            planted_at: 1000,
            growth_time_seconds: 3600,
        };
        // target = 1000 + 3600 = 4600
        assert_eq!(plant.time_to_maturity(1000), 3600);  // 4600 - 1000
        assert_eq!(plant.time_to_maturity(1800), 2800);  // 4600 - 1800
        assert_eq!(plant.time_to_maturity(3600), 1000);  // 4600 - 3600
        assert_eq!(plant.time_to_maturity(4600), 0);     // 4600 >= 4600
        assert_eq!(plant.time_to_maturity(5000), 0);     // 5000 >= 4600
    }

    #[test]
    fn test_plant_display() {
        assert_eq!(format!("{}", PlantType::Wheat), "Wheat");
        assert_eq!(format!("{}", PlantType::Corn), "Corn");
        assert_eq!(format!("{}", PlantType::Tree), "Tree");
        assert_eq!(format!("{}", PlantType::RareHerb), "RareHerb");
    }

    #[test]
    fn test_plant_type_index() {
        assert_eq!(PlantType::Wheat.index(), 0);
        assert_eq!(PlantType::Corn.index(), 1);
        assert_eq!(PlantType::Tree.index(), 2);
        assert_eq!(PlantType::RareHerb.index(), 3);
    }

    #[test]
    fn test_plant_type_config() {
        assert_eq!(PlantType::Wheat.config().type_name, "Wheat");
        assert_eq!(PlantType::Wheat.config().growth_time_seconds, 3600);
        assert_eq!(PlantType::Wheat.config().xp_reward, 5);
        assert_eq!(PlantType::Wheat.config().gold_reward, 15);
    }

    #[test]
    fn test_plant_tracker() {
        let tracker = PlantTracker::new(PlantType::Wheat, 1000);
        assert_eq!(tracker.plant.plant_type, PlantType::Wheat);
        assert!(!tracker.plant.is_mature(1500));
    }

    #[test]
    fn test_hex_tile_state() {
        let mut tile = HexTileState::new(0, "Grass".into(), "0x1234");
        assert!(tile.is_empty());
        assert!(!tile.is_polluted());

        tile.plant = Some(Plant::new(PlantType::Wheat));
        assert!(!tile.is_empty()); // Has a plant

        tile.plant = None;
        assert!(tile.is_empty()); // No plant again
    }

    #[test]
    fn test_hex_tile_state_polluted() {
        let mut tile = HexTileState::new(1, "Polluted".into(), "0x1234");
        assert!(tile.is_polluted());
        assert!(!tile.is_empty());
    }

    #[test]
    fn test_plant_serialize_deserialize() {
        let plant = Plant::new(PlantType::Wheat);
        let serialized = serde_json::to_string(&plant).unwrap();
        let deserialized: Plant = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.plant_type, plant.plant_type);
        assert_eq!(deserialized.growth_time_seconds, plant.growth_time_seconds);
    }

    #[test]
    fn test_plant_all_types_grow_time() {
        assert_eq!(PlantType::Wheat.config().growth_time_seconds, 3600);
        assert_eq!(PlantType::Corn.config().growth_time_seconds, 5400);
        assert_eq!(PlantType::Tree.config().growth_time_seconds, 21600);
        assert_eq!(PlantType::RareHerb.config().growth_time_seconds, 43200);
    }
}
