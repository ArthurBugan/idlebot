//! Action system — Validates and executes player actions with cooldowns.

use crate::economy;
use crate::plant;

// ---------------------------------------------------------------------------
// Cooldown Constants
// ---------------------------------------------------------------------------

/// Seconds between actions (5 second global cooldown)
pub const ACTION_COOLDOWN_SECS: u64 = 5;

/// Server-side plant update interval (seconds)
pub const PLANT_CHECK_INTERVAL: u64 = 10;

// ---------------------------------------------------------------------------
// Action Request
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ActionType {
    Plant { plant_type: plant::PlantType },
    Harvest { plant_type: plant::PlantType },
    Clean,
    ClearTerrain,
    Teleport { target_hex_id: u64 },
}

// ---------------------------------------------------------------------------
// Action Result
// ---------------------------------------------------------------------------

#[derive(Debug)]
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
// Execute All Actions
// ---------------------------------------------------------------------------

/// Execute the requested action on the game state.
pub fn execute_action(
    gs: &mut economy::LocalGameState,
    action: ActionType,
) -> ActionResult {
    // Global cooldown check
    if !gs.can_act() {
        return ActionResult::Failed {
            reason: format!("Action on cooldown ({}s between actions)", ACTION_COOLDOWN_SECS),
        };
    }

    match action {
        ActionType::Plant { plant_type } => {
            gs.record_action_named("plant");
            plant::plant_on_hex(gs, &plant_type.to_string())
                .map(|r| ActionResult::Success {
                    message: r.message,
                    xp: r.xp,
                    gold: r.gold,
                    eco: 0,
                })
                .unwrap_or_else(|e| ActionResult::Failed { reason: e.reason })
        }
        ActionType::Harvest { plant_type } => {
            gs.record_action_named("harvest");
            plant::harvest_hex(gs, &plant_type.to_string())
                .map(|r| ActionResult::Success {
                    message: r.message,
                    xp: r.xp,
                    gold: r.gold,
                    eco: 0,
                })
                .unwrap_or_else(|e| ActionResult::Failed { reason: e.reason })
        }
        ActionType::Clean => {
            gs.record_action_named("clean");
            plant::clean_hex(gs)
                .map(|r| ActionResult::Success {
                    message: r.message,
                    xp: r.xp,
                    gold: r.gold,
                    eco: 30,
                })
                .unwrap_or_else(|e| ActionResult::Failed { reason: e.reason })
        }
        ActionType::ClearTerrain => {
            gs.record_action_named("clear_terrain");
            plant::clear_terrain(gs)
                .map(|r| ActionResult::Success {
                    message: r.message,
                    xp: r.xp,
                    gold: r.gold,
                    eco: 5,
                })
                .unwrap_or_else(|e| ActionResult::Failed { reason: e.reason })
        }
        ActionType::Teleport { target_hex_id } => {
            execute_teleport(gs, target_hex_id)
        }
    }
}

// ---------------------------------------------------------------------------
// Teleport Action
// ---------------------------------------------------------------------------

/// Execute a teleport action
pub fn execute_teleport(
    gs: &mut economy::LocalGameState,
    target_hex_id: u64,
) -> ActionResult {
    let econ = &gs.economy;
    let cost = economy::teleport_cost(econ.level);

    println!("[ACTIONS] Attempting teleport to hex {}", target_hex_id);

    if !gs.can_act() {
        return ActionResult::Failed {
            reason: format!("Action on cooldown ({}s)", ACTION_COOLDOWN_SECS),
        };
    }

    if !economy::spend_gold(&mut econ.economy, cost) {
        return ActionResult::Failed {
            reason: format!("Not enough gold. Need {}G for teleport", cost),
        };
    }

    // Store the new hex position (in a real version, would resolve hex_id to coords)
    gs.current_hex_id = target_hex_id;

    // Update nearby hexes for teleport UI
    gs.nearby_hexes = Vec::new();

    println!(
        "[ACTIONS] Teleported to hex {} (cost: {}G, level {})",
        target_hex_id, cost, econ.level
    );
    gs.record_action_named("teleport");

    ActionResult::Success {
        message: format!("Teleported to hex {} (cost: {}G)", target_hex_id, cost),
        xp: 0,
        gold: cost,
        eco: 0,
    }
}

// ---------------------------------------------------------------------------
// Update Game State (called each frame)
// ---------------------------------------------------------------------------

/// Run periodic updates: plant growth checks, idle gains, maintenance
pub fn tick(gs: &mut economy::LocalGameState, delta_secs: f32) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Check if it's time for plant growth (every PLANT_CHECK_INTERVAL seconds)
    let plant_tracker = gs.plant_tracker.as_ref();
    if let Some(pt) = plant_tracker {
        pt.check_and_harvest();
    }

    // Check if 5 seconds have passed since last action (for auto-retry)
    let last_action = gs.last_action_time;
    let elapsed = now.saturating_sub(last_action);
    if elapsed >= ACTION_COOLDOWN_SECS {
        gs.last_action_time = 0; // Reset (player can act again)
        println!("[ACTIONS] Cooldown expired. Ready for next action.");
    }

    // Add some gold periodically for local testing convenience
    if delta_secs > 120.0 && gs.gold < 500 {
        gs.gold = (gs.gold + 10).min(500);
    }

    // Update display from economy
    gs.refresh_display();
}

// ---------------------------------------------------------------------------
// Debug Commands
// ---------------------------------------------------------------------------

/// Give gold (debug command)
pub fn debug_add_gold(gs: &mut economy::LocalGameState, amount: u64) {
    println!("[DEBUG] Adding {}G to {} (total: {}G)", amount, gs.player_address, gs.gold);
    gs.gold += amount;
}

/// Give XP (debug command)
pub fn debug_add_xp(gs: &mut economy::LocalGameState, amount: u64) {
    println!("[DEBUG] Adding {} XP to {} (total: {} XP)", amount, gs.player_address, gs.xp);
    gs.xp += amount;
}

/// Add eco points (debug command)
pub fn debug_add_eco(gs: &mut economy::LocalGameState, amount: u64) {
    println!("[DEBUG] Adding {} Eco Points to {} (total: {})", amount, gs.player_address, gs.eco_points);
    gs.eco_points += amount;
}

/// Teleport to a specific hex (debug command)
pub fn debug_teleport(gs: &mut economy::LocalGameState, target_hex_id: u64) {
    let econ = &gs.economy;
    let cost = economy::teleport_cost(econ.level);
    println!("[DEBUG] Teleporting to hex {} (cost: {}G)", target_hex_id, cost);

    if !economy::spend_gold(&mut econ.economy, cost) {
        println!("[DEBUG] ERROR: Not enough gold for teleport!");
        return;
    }

    gs.current_hex_id = target_hex_id;
    gs.nearby_hexes.clear();
    gs.last_action_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    println!("[DEBUG] Successfully teleported to hex {}!", target_hex_id);
}

/// Reset cooldowns (debug command)
pub fn debug_reset_cooldown(gs: &mut economy::LocalGameState) {
    gs.last_action_time = 0;
    gs.actions_history.clear();
    println!("[DEBUG] Cooldowns reset. Ready for action.");
}

/// Spawn a fake "player" at a hex (debug command for voice chat testing)
pub fn debug_spawn_fake_player(gs: &mut economy::LocalGameState, hex_id: u64, name: &str) {
    println!("[DEBUG] Fake player '{}' spawned at hex {}", name, hex_id);
    gs.nearby_hexes.push((hex_id, name.to_string()));
}
