//! Action validation and execution.
//! Validates gold, plant state, pollution, and executes actions.

use crate::economy;
use crate::plant::{Plant, PlantActionResult, PlantType};
use crate::HexTileState;
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// ActionResult (refined for client/server wire)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    Success {
        message: String,
        xp_change: u64,
        gold_change: i64, // negative for cost, positive for reward, 0 for no change
    },
    Failed {
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ActionError {
    InsufficientGold { needed: u64 },
    HexOccupied,
    HexEmpty,
    NotPolluted,
    PlantNotMature,
    HexOutOfRange,
    OnCooldown,
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionError::InsufficientGold { needed } => {
                write!(f, "Insufficient gold (need {}G)", needed)
            }
            ActionError::HexOccupied => write!(f, "Hex already occupied"),
            ActionError::HexEmpty => write!(f, "Hex is empty"),
            ActionError::NotPolluted => write!(f, "Hex is not polluted"),
            ActionError::PlantNotMature => write!(f, "Plant not mature yet"),
            ActionError::HexOutOfRange => write!(f, "Hex out of interaction range"),
            ActionError::OnCooldown => write!(f, "Action on cooldown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PLANT_COST: u64 = 10;
const HARVEST_GOLD: u64 = 15;
const HARVEST_XP: u64 = 10;
const CLEAN_COST: u64 = 20;
const CLEAN_GOLD_REWARD: u64 = 20;
const CLEAN_XP_REWARD: u64 = 15;

/// Interaction range: 1 hex radius (10 meters)
pub const INTERACTION_RANGE_HEXES: u32 = 1;
pub const INTERACTION_RANGE_METERS: f32 = 10.0;

/// Cooldown in seconds (5 seconds for user actions)
pub const ACTION_COOLDOWN_SECS: u64 = 5;

/// Snapshot of state needed for action execution (avoids DB calls)
#[derive(Clone, Debug)]
pub struct ActionContext {
    pub player_gold: u64,
    pub player_xp: u64,
    pub current_hex_id: u64,
    pub player_position: (f32, f32),
    pub now: u64,
}

// ---------------------------------------------------------------------------
// Validation Functions
// ---------------------------------------------------------------------------

/// Validate plant action: player must have gold, hex must be empty.
/// Returns Ok(()) if valid, Err with reason otherwise.
pub fn validate_plant(player: &crate::Player, hex: &HexTileState) -> Result<(), ActionError> {
    if player.gold < PLANT_COST {
        return Err(ActionError::InsufficientGold { needed: PLANT_COST });
    }
    if !hex.is_empty() {
        return Err(ActionError::HexOccupied);
    }
    Ok(())
}

/// Validate harvest action: hex must have a mature plant.
pub fn validate_harvest(hex: &HexTileState, now: u64) -> Result<(), ActionError> {
    let plant = hex.plant.as_ref().ok_or(ActionError::HexEmpty)?;
    if !plant.is_mature(now) {
        return Err(ActionError::PlantNotMature);
    }
    Ok(())
}

/// Validate clean action: hex must be polluted, player must have gold.
pub fn validate_clean(player: &crate::Player, hex: &HexTileState) -> Result<(), ActionError> {
    if player.gold < CLEAN_COST {
        return Err(ActionError::InsufficientGold { needed: CLEAN_COST });
    }
    if !hex.is_polluted() {
        return Err(ActionError::NotPolluted);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Execution Functions
// ---------------------------------------------------------------------------

/// Execute plant action: spend 10G, give 5 XP, plant seed on hex.
pub fn execute_plant(player: &mut crate::Player, hex: &mut HexTileState, plant_type: PlantType, now: u64) -> ActionResult {
    // Deduct gold
    player.gold = player.gold.saturating_sub(PLANT_COST);
    player.xp += 5;

    // Check level up
    if player.xp >= player.xp_for_next_level() {
        player.level = player.calculate_level(player.xp);
    }

    // Plant the seed
    let display_name = plant_type.plant_type_name().to_string();
    hex.plant = Some(Plant::new(plant_type));
    hex.is_polluted = false;
    hex.eco_rating = (hex.eco_rating + 10).min(100);

    ActionResult::Success {
        message: format!("Planted {display_name}, +5 XP"),
        xp_change: 5,
        gold_change: -PLANT_COST as i64,
    }
}

/// Execute harvest action: collect gold + XP, remove plant.
pub fn execute_harvest(player: &mut crate::Player, hex: &HexTileState, now: u64) -> ActionResult {
    // The plant must be mature (validated beforehand)
    let plant = hex.plant.as_ref().unwrap(); // safe: validated by caller
    let config = plant.plant_type.config();

    // Add rewards
    player.gold += config.gold_reward;
    player.xp += config.xp_reward;

    // Check level up
    if player.xp >= player.xp_for_next_level() {
        player.level = player.calculate_level(player.xp);
    }

    // Remove plant
    hex.plant = None;

    ActionResult::Success {
        message: format!("Harvested {}! +{}G, +{} XP", plant.plant_type, config.gold_reward, config.xp_reward),
        xp_change: config.xp_reward,
        gold_change: config.gold_reward as i64,
    }
}

/// Execute clean action: spend 20G, give 20G + 15 XP, remove pollution.
pub fn execute_clean(player: &mut crate::Player, hex: &mut HexTileState) -> ActionResult {
    // Net gold change: -20G cost + 20G reward = 0
    player.gold = player.gold.saturating_sub(CLEAN_COST).saturating_add(CLEAN_GOLD_REWARD);
    player.xp += CLEAN_XP_REWARD;

    // Check level up
    if player.xp >= player.xp_for_next_level() {
        player.level = player.calculate_level(player.xp);
    }

    // Remove pollution
    hex.is_polluted = false;
    hex.eco_rating = (hex.eco_rating + 30).min(100);

    ActionResult::Success {
        message: "Pollution cleaned! +15 XP".to_string(),
        xp_change: CLEAN_XP_REWARD,
        gold_change: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Player;
    use crate::Vehicle;

    fn test_player() -> crate::Player {
        Player::new("0x1234".into(), crate::Position::new(0.0, 0.0))
    }

    #[test]
    fn test_validate_plant_insufficient_gold() {
        let player = test_player();
        player.gold = 5;
        let hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = validate_plant(&player, &hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::InsufficientGold { needed } => {
                assert_eq!(needed, 10);
            }
            _ => panic!("Expected InsufficientGold"),
        }
    }

    #[test]
    fn test_validate_plant_sufficient_gold() {
        let player = test_player(); // starts with 100G
        let hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = validate_plant(&player, &hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_plant_hex_occupied() {
        let player = test_player();
        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        hex.plant = Some(Plant::new(PlantType::Wheat));
        let result = validate_plant(&player, &hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::HexOccupied => {}
            _ => panic!("Expected HexOccupied"),
        }
    }

    #[test]
    fn test_validate_harvest_no_plant() {
        let hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = validate_harvest(&hex, 1000);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::HexEmpty => {}
            _ => panic!("Expected HexEmpty"),
        }
    }

    #[test]
    fn test_validate_harvest_not_mature() {
        let now = 1000;
        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        hex.plant = Some(Plant {
            plant_type: PlantType::Wheat,
            planted_at: now,
            growth_time_seconds: 3600,
        });
        let result = validate_harvest(&hex, now);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::PlantNotMature => {}
            _ => panic!("Expected PlantNotMature"),
        }
    }

    #[test]
    fn test_validate_harvest_mature() {
        let now = 1000;
        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        hex.plant = Some(Plant {
            plant_type: PlantType::Wheat,
            planted_at: now,
            growth_time_seconds: 3600,
        });
        // Wait until mature
        let result = validate_harvest(&hex, now + 3600);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_clean_no_pollution() {
        let player = test_player();
        let hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = validate_clean(&player, &hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::NotPolluted => {}
            _ => panic!("Expected NotPolluted"),
        }
    }

    #[test]
    fn test_validate_clean_insufficient_gold() {
        let mut player = test_player();
        player.gold = 10;
        let mut hex = HexTileState::new(0, "Polluted".into(), "0x1234");
        hex.is_polluted = true;
        let result = validate_clean(&player, &hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::InsufficientGold { needed } => {
                assert_eq!(needed, 20);
            }
            _ => panic!("Expected InsufficientGold"),
        }
    }

    #[test]
    fn test_validate_clean_sufficient() {
        let player = test_player(); // 100G
        let mut hex = HexTileState::new(0, "Polluted".into(), "0x1234");
        hex.is_polluted = true;
        let result = validate_clean(&player, &hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_plant_success() {
        let mut player = test_player();
        assert_eq!(player.gold, 100);

        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = execute_plant(&mut player, &mut hex, PlantType::Wheat, 1000);

        assert_eq!(player.gold, 90);
        assert_eq!(player.xp, 5);
        assert!(hex.plant.is_some());
        assert!(hex.plant.as_ref().unwrap().plant_type == PlantType::Wheat);
        match result {
            ActionResult::Success { message, xp_change, gold_change } => {
                assert!(message.contains("Planted"));
                assert_eq!(xp_change, 5);
                assert_eq!(gold_change, -10);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_execute_plant_insufficient_gold() {
        let mut player = test_player();
        player.gold = 5;

        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        let result = execute_plant(&mut player, &mut hex, PlantType::Wheat, 1000);

        match result {
            ActionResult::Failed { reason } => {
                assert!(reason.contains("Insufficient gold"));
            }
            _ => panic!("Expected failed action"),
        }
    }

    #[test]
    fn test_execute_harvest_success() {
        let mut player = test_player();
        let now = 1000;
        let mut hex = HexTileState::new(0, "Grass".into(), "0x1234");
        hex.plant = Some(Plant {
            plant_type: PlantType::Wheat,
            planted_at: now,
            growth_time_seconds: 3600,
        });
        // Make it mature
        let result = execute_harvest(&mut player, &hex, now + 3600);

        assert_eq!(player.gold, 115);
        assert_eq!(player.xp, 10);
        assert!(hex.plant.is_none());
        match result {
            ActionResult::Success { message, xp_change, gold_change } => {
                assert!(message.contains("Harvested"));
                assert_eq!(xp_change, 5);  // Wheat gives 5 XP
                assert_eq!(gold_change, 15);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_execute_clean_success() {
        let mut player = test_player();
        let mut hex = HexTileState::new(0, "Polluted".into(), "0x1234");
        hex.is_polluted = true;

        let result = execute_clean(&mut player, &mut hex);

        assert_eq!(player.gold, 100); // -20 + 20 = 100
        assert_eq!(player.xp, 15);
        assert!(!hex.is_polluted());
        hex.eco_rating = (50 + 30).min(100);
        assert_eq!(hex.eco_rating, 80);
        match result {
            ActionResult::Success { message, xp_change, gold_change } => {
                assert!(message.contains("Pollution cleaned"));
                assert_eq!(xp_change, 15);
                assert_eq!(gold_change, 0);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_execute_plant_tree() {
        let mut player = test_player();
        let mut hex = HexTileState::new(0, "Forest".into(), "0x1234");
        let result = execute_plant(&mut player, &mut hex, PlantType::Tree, 1000);

        assert_eq!(player.gold, 90);
        assert_eq!(player.xp, 5);
        assert!(hex.plant.as_ref().unwrap().plant_type == PlantType::Tree);
    }

    #[test]
    fn test_execute_clean_nets_zero_gold() {
        let mut player = test_player();
        let mut hex = HexTileState::new(0, "Polluted".into(), "0x1234");
        hex.is_polluted = true;

        let result = execute_clean(&mut player, &mut hex);
        match result {
            ActionResult::Success { gold_change, .. } => {
                assert_eq!(gold_change, 0);
            }
            _ => panic!("Expected success"),
        }
    }
}
