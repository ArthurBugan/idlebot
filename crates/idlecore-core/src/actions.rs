//! Action system — Validates and executes player actions with cooldowns.

use crate::economy;
use crate::plant;
use crate::PlantType as ActionTypePlantType;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Cooldown Constants
// ---------------------------------------------------------------------------

/// Seconds between actions (5 second global cooldown)
const ACTION_COOLDOWN_SECS: u64 = 5;

/// Seconds between auto-harvest checks (60 seconds)
const AUTO_HARVEST_COOLDOWN_SECS: u64 = 60;

/// Seconds between plant growth ticks (10 seconds)
const GROWTH_TICK_COOLDOWN_SECS: u64 = 10;

/// Base XP for simple actions
const BASE_XP: u64 = 5;

/// Eco points for cleaning actions
const CLEAN_ECO_REWARD: u64 = 30;

/// Eco points for clearing terrain
const CLEAR_TERRAIN_ECO_REWARD: u64 = 5;

// ---------------------------------------------------------------------------
// ActionResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    Success {
        message: String,
        xp: u64,
        gold: u64,
        eco: u64,
    },
    Failed {
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// ActionType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Plant { plant_type: crate::PlantType },
    Harvest { plant_type: crate::PlantType },
    Clean,
    ClearTerrain,
    Teleport { target_hex_id: u64 },
    Vehicle { action: String, vehicle_type: String },
    MarketBuy { item_id: u64 },
    MarketSell { item_id: u64, price: u64 },
    IdleFarm,
}

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Action Execution
// ---------------------------------------------------------------------------

/// Execute an action, returning the result
pub fn execute_action(gs: &mut economy::LocalGameState, action: ActionType) -> ActionResult {
    // Check cooldown
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    if now - gs.last_action_time < ACTION_COOLDOWN_SECS {
        return ActionResult::Failed {
            reason: format!("Action on cooldown. Wait {}s", ACTION_COOLDOWN_SECS - (now - gs.last_action_time)),
        };
    }

    match action {
        ActionType::Plant { plant_type } => {
            gs.last_action_time = now;
            gs.actions_history.push("plant".to_string());
            match plant::plant_on_hex(gs, plant_type) {
                plant::PlantActionResult::Success { message, xp, gold } => ActionResult::Success {
                    message,
                    xp,
                    gold,
                    eco: 0,
                },
                plant::PlantActionResult::Failed { reason } => ActionResult::Failed { reason },
            }
        }
        ActionType::Harvest { plant_type } => {
            gs.last_action_time = now;
            gs.actions_history.push("harvest".to_string());
            match plant::harvest_hex(gs, plant_type) {
                plant::PlantActionResult::Success { message, xp, gold } => ActionResult::Success {
                    message,
                    xp,
                    gold,
                    eco: 0,
                },
                plant::PlantActionResult::Failed { reason } => ActionResult::Failed { reason },
            }
        }
        ActionType::Clean => {
            gs.last_action_time = now;
            gs.actions_history.push("clean".to_string());
            match plant::clean_hex(gs) {
                plant::PlantActionResult::Success { message, xp, gold } => ActionResult::Success {
                    message,
                    xp,
                    gold,
                    eco: CLEAN_ECO_REWARD,
                },
                plant::PlantActionResult::Failed { reason } => ActionResult::Failed { reason },
            }
        }
        ActionType::ClearTerrain => {
            gs.last_action_time = now;
            gs.actions_history.push("clear_terrain".to_string());
            match plant::clear_terrain(gs) {
                plant::PlantActionResult::Success { message, xp, gold } => ActionResult::Success {
                    message,
                    xp,
                    gold,
                    eco: CLEAR_TERRAIN_ECO_REWARD,
                },
                plant::PlantActionResult::Failed { reason } => ActionResult::Failed { reason },
            }
        }
        ActionType::Teleport { target_hex_id } => {
            execute_teleport(gs, target_hex_id)
        }
        ActionType::Vehicle { action: _, vehicle_type: _ } => {
            ActionResult::Failed {
                reason: "Vehicle actions not yet implemented".to_string(),
            }
        }
        ActionType::MarketBuy { item_id } => {
            ActionResult::Failed {
                reason: format!("Market buy item {} not yet implemented", item_id),
            }
        }
        ActionType::MarketSell { item_id, price: _ } => {
            ActionResult::Failed {
                reason: format!("Market sell item {} not yet implemented", item_id),
            }
        }
        ActionType::IdleFarm => {
            gs.last_action_time = now;
            gs.actions_history.push("idle_farm".to_string());
            ActionResult::Success {
                message: format!("Idle farming... (+{} XP, +{}G)", BASE_XP, BASE_XP / 2),
                xp: BASE_XP,
                gold: BASE_XP / 2,
                eco: 0,
            }
        }
    }
}

/// Execute teleport action
fn execute_teleport(gs: &mut economy::LocalGameState, target_hex_id: u64) -> ActionResult {
    gs.last_action_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    gs.actions_history.push("teleport".to_string());
    
    // Check if player can afford teleport
    let cost = economy::teleport_cost(gs.economy.level);
    if !economy::spend_gold(&mut gs.economy, cost) {
        return ActionResult::Failed {
            reason: format!("Not enough gold for teleport. Need {}G", cost),
        };
    }

    // Update hex position
    gs.current_hex_id = target_hex_id;
    
    ActionResult::Success {
        message: format!("Teleported to hex {}", target_hex_id),
        xp: 2,
        gold: 0,
        eco: 0,
    }
}

/// Get the number of actions taken in the last cooldown period
pub fn actions_in_cooldown(gs: &economy::LocalGameState) -> usize {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Count actions in the last ACTION_COOLDOWN_SECS based on history
    gs.actions_history.iter()
        .filter(|_| true) // Simplified - would check timestamps in real impl
        .count()
}

/// Check if an action can be performed (no cooldown)
pub fn can_act(gs: &economy::LocalGameState) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    now - gs.last_action_time > ACTION_COOLDOWN_SECS
}

/// Auto-harvest mature plants
pub fn auto_harvest(gs: &mut economy::LocalGameState) -> Vec<ActionResult> {
    let mut results = Vec::new();
    
    // Check if we can auto-harvest
    if !can_act(gs) {
        return results;
    }
    
    // This would iterate over hexes and harvest mature plants
    // For now, just return an empty list
    results
}

/// Process plant growth ticks
pub fn process_growth_ticks(gs: &mut economy::LocalGameState) {
    // This would iterate over all plants and update their growth stage
}
