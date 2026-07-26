//! Plant growth system — stages, types, growth timers.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::fmt;
use crate::economy;
use crate::PlantType;

// ---------------------------------------------------------------------------
// Plant Types (by index into PLANT_CONFIGS)
// ---------------------------------------------------------------------------

impl PlantType {
    /// Get the index of this plant type
    pub fn index(&self) -> usize {
        match self {
            PlantType::Wheat => 0,
            PlantType::Corn => 1,
            PlantType::Tree => 2,
            PlantType::RareHerb => 3,
        }
    }
}

/// Growth data for each plant type. Indexed by PlantType variant number.
pub struct PlantGrowthConfig {
    pub type_name: &'static str,
    pub growth_time_seconds: u64,
    pub xp_reward: u64,
    pub gold_reward: u64,
    pub eco_reward: u64,
}

pub const PLANT_CONFIGS: &[PlantGrowthConfig] = &[
    PlantGrowthConfig {
        type_name: "Wheat",
        growth_time_seconds: 3600,  // 1 hour
        xp_reward: 5,
        gold_reward: 15,
        eco_reward: 0,
    },
    PlantGrowthConfig {
        type_name: "Corn",
        growth_time_seconds: 5400,  // 1.5 hours
        xp_reward: 8,
        gold_reward: 25,
        eco_reward: 0,
    },
    PlantGrowthConfig {
        type_name: "Tree",
        growth_time_seconds: 21600, // 6 hours
        xp_reward: 30,
        gold_reward: 80,
        eco_reward: 10,
    },
    PlantGrowthConfig {
        type_name: "RareHerb",
        growth_time_seconds: 43200, // 12 hours
        xp_reward: 60,
        gold_reward: 200,
        eco_reward: 20,
    },
];

/// Get config for a plant type by index
pub fn get_plant_config(index: usize) -> &'static PlantGrowthConfig {
    assert!(index < PLANT_CONFIGS.len(), "Unknown plant type index: {}", index);
    &PLANT_CONFIGS[index]
}

// ---------------------------------------------------------------------------
// Plant State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantStage {
    Planted,
    Growing,
    Ready,
}

#[derive(Debug, Clone)]
pub struct PlantState {
    pub plant_type_index: usize,
    pub planted_at: u64,
    pub stage: PlantStage,
}

impl PlantState {
    pub fn new(plant_type_index: usize, planted_at: u64) -> Self {
        Self {
            plant_type_index,
            planted_at,
            stage: PlantStage::Planted,
        }
    }

    pub fn plant_type_name(&self) -> &'static str {
        PLANT_CONFIGS[self.plant_type_index].type_name
    }

    pub fn check_growth(&mut self, current_time: u64) -> bool {
        let config = get_plant_config(self.plant_type_index);
        let elapsed = current_time - self.planted_at;

        match self.stage {
            PlantStage::Planted => {
                if elapsed >= config.growth_time_seconds {
                    self.stage = PlantStage::Growing;
                }
            }
            PlantStage::Growing => {
                // After growing phase (first quarter), plant is ready
                let grow_time = config.growth_time_seconds;
                if elapsed >= grow_time + (grow_time / 4) {
                    self.stage = PlantStage::Ready;
                    return true;
                }
            }
            PlantStage::Ready => {}
        }
        false
    }

    pub fn harvest_if_ready(&self, _current_time: u64) -> Option<(u64, u64, u64)> {
        if self.stage == PlantStage::Ready {
            let config = get_plant_config(self.plant_type_index);
            Some((config.xp_reward, config.gold_reward, config.eco_reward))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tile State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HexTileState {
    pub hex_id: u64,
    pub terrain: String,
    pub is_polluted: bool,
    pub plant: Option<PlantState>,
    pub eco_rating: u32,
    pub owner_address: String,
    pub last_interacted: u64,
}

impl HexTileState {
    pub fn new(hex_id: u64, terrain: String, owner_address: &str) -> Self {
        let terrain_clone = terrain.clone();
        let is_polluted = terrain_clone == "Polluted";
        let eco_rating = if terrain_clone == "Forest" || terrain_clone == "Grass" { 50 } else { 20 };
        Self {
            hex_id,
            terrain,
            is_polluted,
            plant: None,
            eco_rating,
            owner_address: owner_address.to_string(),
            last_interacted: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn plant_seed(&mut self, plant_type: PlantType, now: u64) {
        self.is_polluted = false;
        self.eco_rating = (self.eco_rating + 10).min(100);
        self.plant = Some(PlantState::new(plant_type as usize, now));
        self.last_interacted = now;
    }

    pub fn clean_pollution(&mut self) -> bool {
        if self.is_polluted {
            self.is_polluted = false;
            self.eco_rating = (self.eco_rating + 30).min(100);
            true
        } else {
            false
        }
    }

    pub fn harvest(&mut self) -> Option<(PlantState, (u64, u64, u64))> {
        if let Some(plant) = self.plant.take() {
            if plant.stage == PlantStage::Ready {
                let rewards = plant.harvest_if_ready(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                );
                Some((plant, rewards.unwrap()))
            } else {
                None
            }
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Action Results
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum PlantActionResult {
    Success { message: String, xp: u64, gold: u64 },
    Failed { reason: String },
}

// ---------------------------------------------------------------------------
// Plant Action Functions
// ---------------------------------------------------------------------------

/// Plant a seed: cost 10G, gives 5 XP
pub fn plant_on_hex(gs: &mut economy::LocalGameState, plant_type: PlantType) -> PlantActionResult {
    let cost = economy::PLANT_COST;

    println!("[PLANT] Attempting to plant {:?}...", plant_type);

    if !gs.can_act() {
        return PlantActionResult::Failed {
            reason: "Action on cooldown (5s)".to_string(),
        };
    }

    if !economy::spend_gold(&mut gs.economy, cost) {
        return PlantActionResult::Failed {
            reason: format!("Not enough gold. Need {}G", cost),
        };
    }

    println!(
        "[PLANT] Planted {:?}. Hex: {}, Cost: {}G, XP: 5",
        plant_type, gs.current_hex_id, cost
    );
    gs.last_action_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    gs.actions_history.push("plant".to_string());
    economy::add_xp(&mut gs.economy, 5);

    let growth_label = match plant_type {
        PlantType::Wheat => "Wheat (1h)",
        PlantType::Corn => "Corn (1.5h)",
        PlantType::Tree => "Tree (6h)",
        PlantType::RareHerb => "RareHerb (12h)",
    };
    println!(
        "[PLANT] Player {} now owns hex with {} (growth {})",
        gs.player_address,
        PLANT_CONFIGS[plant_type as usize].type_name,
        growth_label,
    );

    PlantActionResult::Success {
        message: format!("Planted {:?}, 5 XP gained", plant_type),
        xp: 5,
        gold: 0,
    }
}

/// Harvest a plant: gives gold + XP + eco based on type
pub fn harvest_hex(gs: &mut economy::LocalGameState, plant_type: PlantType) -> PlantActionResult {

    println!("[PLANT] Harvesting {:?} from hex {}", plant_type, gs.current_hex_id);

    if !gs.can_act() {
        return PlantActionResult::Failed {
            reason: "Action on cooldown (5s)".to_string(),
        };
    }

    let config = &PLANT_CONFIGS[plant_type.index()];
    let gold_reward = config.gold_reward;
    let xp = config.xp_reward;
    let eco = config.eco_reward;

    println!(
        "[PLANT] Harvesting mature {:?} - {}G gold, {} XP, {} Eco",
        plant_type, gold_reward, xp, eco
    );
    gs.last_action_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    gs.actions_history.push("harvest".to_string());

    economy::add_gold(&mut gs.economy, gold_reward);
    economy::add_xp(&mut gs.economy, xp);
    economy::add_eco_points(&mut gs.economy, eco);

    PlantActionResult::Success {
        message: format!(
            "Harvested {:?}! +{}G, +{} XP, +{} Eco",
            plant_type, gold_reward, xp, eco
        ),
        xp,
        gold: gold_reward,
    }
}

/// Clean polluted hex: cost 20G, give 20G and 15 XP
pub fn clean_hex(gs: &mut economy::LocalGameState) -> PlantActionResult {

    println!("[PLANT] Attempting to clean hex {}", gs.current_hex_id);

    if !gs.can_act() {
        return PlantActionResult::Failed {
            reason: "Action on cooldown (5s)".to_string(),
        };
    }

    if !economy::spend_gold(&mut gs.economy, economy::CLEAN_COST) {
        return PlantActionResult::Failed {
            reason: format!("Not enough gold. Need {}G", economy::CLEAN_COST),
        };
    }

    println!(
        "[PLANT] Cleaned hex {}! +{}G returned, +{} XP",
        gs.current_hex_id,
        economy::CLEAN_GOLD_REWARD,
        economy::CLEAN_XP_REWARD
    );
    gs.last_action_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    gs.actions_history.push("clean".to_string());

    economy::add_gold(&mut gs.economy, economy::CLEAN_GOLD_REWARD);
    economy::add_xp(&mut gs.economy, economy::CLEAN_XP_REWARD);
    economy::add_eco_points(&mut gs.economy, 30);

    PlantActionResult::Success {
        message: format!(
            "Cleaned pollution on hex {}. +20G, +15 XP, +30 Eco Points",
            gs.current_hex_id
        ),
        xp: economy::CLEAN_XP_REWARD,
        gold: economy::CLEAN_GOLD_REWARD,
    }
}

/// Clear terrain: cost 15G, give 5 XP
pub fn clear_terrain(gs: &mut economy::LocalGameState) -> PlantActionResult {

    println!("[PLANT] Attempting to clear terrain on hex {}", gs.current_hex_id);

    if !gs.can_act() {
        return PlantActionResult::Failed {
            reason: "Action on cooldown (5s)".to_string(),
        };
    }

    if !economy::spend_gold(&mut gs.economy, economy::CLEAR_COST) {
        return PlantActionResult::Failed {
            reason: format!("Not enough gold. Need {}G", economy::CLEAR_COST),
        };
    }

    println!(
        "[PLANT] Cleared terrain on hex {}! +{} XP",
        gs.current_hex_id, economy::CLEAR_XP_REWARD
    );
    gs.last_action_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    gs.actions_history.push("clear_terrain".to_string());

    economy::add_xp(&mut gs.economy, economy::CLEAR_XP_REWARD);
    economy::add_eco_points(&mut gs.economy, 5);

    PlantActionResult::Success {
        message: format!(
            "Cleared terrain on hex {}. +{} XP, +{} Eco Points",
            gs.current_hex_id, economy::CLEAR_XP_REWARD, 5
        ),
        xp: economy::CLEAR_XP_REWARD,
        gold: 0,
    }
}

// ---------------------------------------------------------------------------
// Local-only Plant Tracker
// ---------------------------------------------------------------------------

pub struct LocalPlantTracker {
    pub plants: Vec<(u64, PlantState)>,
    pub last_check_time: u64,
    pub check_interval: u64,
}

impl LocalPlantTracker {
    pub fn new() -> Self {
        Self {
            plants: Vec::new(),
            last_check_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            check_interval: 10,
        }
    }

    pub fn add_plant(&mut self, hex_id: u64, plant_type: PlantType) {
        self.plants.push((
            hex_id,
            PlantState::new(plant_type as usize,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()),
        ));
    }

    pub fn check_and_harvest(&mut self) -> Vec<(u64, PlantState, (u64, u64, u64))> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - self.last_check_time < self.check_interval {
            return Vec::new();
        }

        self.last_check_time = now;
        let mut harvested = Vec::new();

        for (hex_id, plant) in &mut self.plants {
            plant.check_growth(now);
            if plant.stage == PlantStage::Ready {
                if let Some(rewards) = plant.harvest_if_ready(now) {
                    harvested.push((*hex_id, plant.clone(), rewards));
                    println!(
                        "[PLANT] Tick: Plant {:?} on hex {} ready! +{} XP, +{} Gold, +{} Eco",
                        PLANT_CONFIGS[plant.plant_type_index].type_name,
                        hex_id, rewards.0, rewards.1, rewards.2
                    );
                }
            }
        }

        harvested
    }
}

impl Default for LocalPlantTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plant_config() {
        assert_eq!(PLANT_CONFIGS[0].type_name, "Wheat");
        assert_eq!(PLANT_CONFIGS[0].growth_time_seconds, 3600);
        assert_eq!(PLANT_CONFIGS[2].growth_time_seconds, 21600);
        assert_eq!(PLANT_CONFIGS[3].growth_time_seconds, 43200);
    }

    #[test]
    fn test_plant_growth_stages() {
        let mut plant = PlantState::new(0, 1000);
        assert_eq!(plant.stage, PlantStage::Planted);

        let now = 1000 + 3600 + 1;
        let ready = plant.check_growth(now);
        assert!(ready);
        assert_eq!(plant.stage, PlantStage::Ready);
    }
}
