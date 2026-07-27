//! Interaction system — hex action commands for the client.
//!
//! When the player presses E or clicks, this creates an InteractionCommand
//! that the main system sends to the server.

use bevy::prelude::*;
use idlecore_core::actions::{
    execute_action, ActionResult,
};
use idlecore_core::plant::PlantType;

/// A command sent to the server when the player wants to interact with a hex.
#[derive(Debug, Clone)]
pub struct InteractionCommand {
    /// Action to perform: plant, harvest, or clean
    pub action: InteractionAction,
    /// Plant type for planting (None for harvest/clean)
    pub plant_type: Option<String>,
}

/// Supported interaction actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionAction {
    Plant,
    Harvest,
    Clean,
}

impl std::fmt::Display for InteractionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractionAction::Plant => write!(f, "plant"),
            InteractionAction::Harvest => write!(f, "harvest"),
            InteractionAction::Clean => write!(f, "clean"),
        }
    }
}

impl InteractionAction {
    pub fn to_json(&self) -> String {
        match self {
            InteractionAction::Plant => "\"plant\"".to_string(),
            InteractionAction::Harvest => "\"harvest\"".to_string(),
            InteractionAction::Clean => "\"clean\"".to_string(),
        }
    }
}

/// Result of sending an interaction to the server.
#[derive(Debug, Clone)]
pub struct InteractionResult {
    pub action_result: ActionResult,
    /// Time since the interaction was sent (for UI feedback)
    pub elapsed_ms: f64,
}

/// Execute an interaction command locally and return the result.
/// In a full multiplayer game, this would send to the server instead.
pub fn execute_interaction(
    player: &idlecore_core::Player,
    hex_id: u64,
    action: InteractionAction,
    plant_type: Option<String>,
    now: u64,
) -> InteractionResult {
    let start = std::time::Instant::now();

    let result = match action {
        InteractionAction::Plant => {
            if let Some(pt) = plant_type {
                match pt.as_str() {
                    "Wheat" => execute_action(
                        idlecore_core::PlayerType::Plant(PlantType::Wheat),
                        PlayerTypeContext::new(player, hex_id),
                        1000,
                    ),
                    "Corn" => execute_action(
                        idlecore_core::PlayerType::Plant(PlantType::Corn),
                        PlayerTypeContext::new(player, hex_id),
                        1000,
                    ),
                    "Tree" => execute_action(
                        idlecore_core::PlayerType::Plant(PlantType::Tree),
                        PlayerTypeContext::new(player, hex_id),
                        1000,
                    ),
                    "RareHerb" => execute_action(
                        idlecore_core::PlayerType::Plant(PlantType::RareHerb),
                        PlayerTypeContext::new(player, hex_id),
                        1000,
                    ),
                    _ => ActionResult::Failed {
                        reason: format!("Unknown plant type: {}", pt),
                    },
                }
            } else {
                ActionResult::Failed {
                    reason: "Plant type required for planting".to_string(),
                }
            }
        }
        InteractionAction::Harvest => execute_action(
            idlecore_core::PlayerType::Harvest,
            PlayerTypeContext::new(player, hex_id),
            now,
        ),
        InteractionAction::Clean => execute_action(
            idlecore_core::PlayerType::Clean,
            PlayerTypeContext::new(player, hex_id),
            now,
        ),
    };

    let elapsed = start.elapsed().as_secs_f64();
    InteractionResult {
        action_result: result,
        elapsed_ms: elapsed,
    }
}

/// Context for an action, simplified for local play.
/// In production, this would include the full player state and hex data.
#[derive(Debug, Clone)]
struct PlayerTypeContext {
    gold: u64,
    xp: u64,
    hex_id: u64,
    is_polluted: bool,
    has_plant: bool,
}

impl PlayerTypeContext {
    fn new(player: &idlecore_core::Player, hex_id: u64) -> Self {
        Self {
            gold: player.gold,
            xp: player.xp,
            hex_id,
            is_polluted: false,
            has_plant: false,
        }
    }
}

/// Execute a plant action: spend 10G, give 5 XP, plant seed.
fn execute_action(
    _action_type: idlecore_core::PlayerType,
    ctx: PlayerTypeContext,
    _timestamp: u64,
) -> ActionResult {
    match _action_type {
        idlecore_core::PlayerType::Plant(PlantType::Wheat) => {
            // Plant: cost 10G, gives 5 XP
            ActionResult::Success {
                message: "Planted Wheat, +5 XP".to_string(),
                xp_gained: 5,
                gold_gained: -10,
            }
        }
        idlecore_core::PlayerType::Plant(_pt) => {
            ActionResult::Success {
                message: "Planted plant, +5 XP".to_string(),
                xp_gained: 5,
                gold_gained: -10,
            }
        }
        idlecore_core::PlayerType::Harvest => {
            // Harvest: free, gives 15G + 10 XP (if plant is mature)
            ActionResult::Success {
                message: "Harvested plant! +15G, +10 XP".to_string(),
                xp_gained: 10,
                gold_gained: 15,
            }
        }
        idlecore_core::PlayerType::Clean => {
            // Clean: cost 20G, gives 20G + 15 XP
            ActionResult::Success {
                message: "Cleaned pollution! +15 XP".to_string(),
                xp_gained: 15,
                gold_gained: 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlecore_core::Player;

    #[test]
    fn test_interaction_action_to_string() {
        assert_eq!(InteractionAction::Plant.to_string(), "plant");
        assert_eq!(InteractionAction::Harvest.to_string(), "harvest");
        assert_eq!(InteractionAction::Clean.to_string(), "clean");
    }

    #[test]
    fn test_interaction_action_to_json() {
        assert_eq!(InteractionAction::Plant.to_json(), "\"plant\"");
        assert_eq!(InteractionAction::Harvest.to_json(), "\"harvest\"");
        assert_eq!(InteractionAction::Clean.to_json(), "\"clean\"");
    }

    #[test]
    fn test_interaction_command_plant() {
        let cmd = InteractionCommand {
            action: InteractionAction::Plant,
            plant_type: Some("Wheat".to_string()),
        };
        assert_eq!(cmd.action, InteractionAction::Plant);
    }

    #[test]
    fn test_interaction_result() {
        let result = InteractionResult {
            action_result: ActionResult::Success {
                message: "Success".to_string(),
                xp_gained: 5,
                gold_gained: -10,
            },
            elapsed_ms: 0.001,
        };
        match result.action_result {
            ActionResult::Success { message, xp_gained, gold_change } => {
                assert_eq!(message, "Success");
                assert_eq!(xp_gained, 5);
                assert_eq!(gold_change, -10);
            }
            _ => panic!("Expected Success"),
        }
    }
}
