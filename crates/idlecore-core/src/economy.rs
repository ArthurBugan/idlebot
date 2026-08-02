//! Economy system -- Gold, XP, level progression, costs, rewards.
//!
//! Local single-player: tracks player gold/XP, validates action costs,
//! applies vehicle maintenance, and calculates teleport/level costs.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration as ChronoDuration};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Cost to plant a seed (Gold)
pub const PLANT_COST: u64 = 10;

/// Reward from harvesting (Gold)
pub const HARVEST_GOLD_REWARD: u64 = 15;

/// Reward from harvesting (XP)
pub const HARVEST_XP_REWARD: u64 = 10;

/// Cost to clean a polluted hex (Gold)
pub const CLEAN_COST: u64 = 20;

/// Reward from cleaning a polluted hex (Gold)
pub const CLEAN_GOLD_REWARD: u64 = 20;

/// Reward from cleaning a polluted hex (XP)
pub const CLEAN_XP_REWARD: u64 = 15;

/// Cost to clear terrain (Gold)
pub const CLEAR_COST: u64 = 15;

/// Clear terrain XP reward
pub const CLEAR_XP_REWARD: u64 = 5;

/// Vehicle maintenance cost per hour (Gold)
pub const VEHICLE_MAINTENANCE_RATE_PER_HOUR: u64 = 5;

/// Teleport base cost (Gold)
pub const TELEPORT_BASE_COST: u64 = 100;

/// Maximum hexes per hex (voice channel limit)
pub const MAX_HEX_PLAYERS: u32 = 8;

/// Max idle gains (from server)
pub const MAX_IDLE_XP: u64 = 150;
pub const MAX_IDLE_GOLD: u64 = 75;

// ---------------------------------------------------------------------------
// Vehicle costs from PROPOSAL
// ---------------------------------------------------------------------------

pub struct VehicleDef {
    pub cost: u64,
    pub speed_multiplier: f32,
}

pub const VEHICLE_DEFINITIONS: &[VehicleDef] = &[
    VehicleDef { cost: 0, speed_multiplier: 1.0 },     // None
    VehicleDef { cost: 500, speed_multiplier: 2.0 },   // Bicycle
    VehicleDef { cost: 1000, speed_multiplier: 3.0 },  // Scooter
    VehicleDef { cost: 2500, speed_multiplier: 5.0 },  // Motorcycle
    VehicleDef { cost: 2000, speed_multiplier: 4.0 },  // Boat
    VehicleDef { cost: 10000, speed_multiplier: 10.0 },// Airplane
];

// ---------------------------------------------------------------------------
// Player Economy State
// ---------------------------------------------------------------------------

/// Full economy state for a single player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEconomy {
    pub address: String,
    pub gold: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    pub last_login_time: u64,
    pub last_logout_time: u64,
    pub vehicle: String,
    pub last_daily_gold_check: u64,
    pub daily_gold_applied: u64,
    pub last_level_check_time: u64,
    pub next_level_xp_needed: u64,
}

impl Default for PlayerEconomy {
    fn default() -> Self {
        Self {
            address: String::new(),
            gold: 100,        // Starting gold
            xp: 0,
            level: 1,
            eco_points: 0,
            last_login_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_logout_time: 0,
            vehicle: String::new(),
            last_daily_gold_check: 0,
            daily_gold_applied: 0,
            last_level_check_time: 0,
            next_level_xp_needed: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Utility: Level Calculation
// ---------------------------------------------------------------------------

/// XP required to reach the next level from the current level
/// Formula from spec: `100 * level^2`
pub fn xp_for_next_level(level: u32) -> u64 {
    100 * (level as u64).pow(2)
}

/// Given total accumulated XP, calculate current level
pub fn calculate_level(total_xp: u64) -> u32 {
    let mut level = 1u32;
    let mut xp_needed = xp_for_next_level(1);
    let mut remaining = total_xp;
    while remaining >= xp_needed {
        remaining -= xp_needed;
        level += 1;
        xp_needed = xp_for_next_level(level);
    }
    level
}

/// Recalculate level from total XP
pub fn recalculate_level(econ: &mut PlayerEconomy) {
    econ.level = calculate_level(econ.xp);
    econ.next_level_xp_needed = xp_for_next_level(econ.level);
}

// ---------------------------------------------------------------------------
// Utility: Teleport Cost
// ---------------------------------------------------------------------------

/// Calculate teleport cost based on current level
/// Base: 100G, scales as 100 * sqrt(level)
pub fn teleport_cost(level: u32) -> u64 {
    // 100 * sqrt(level)
    (((TELEPORT_BASE_COST as f64) * (level as f64).sqrt()) as u64)
        .min((level as u64).pow(2))  // cap at level^2 to avoid absurd costs
}

// ---------------------------------------------------------------------------
// Mock Idle Gain Calculation (server-side logic replicated locally)
// ---------------------------------------------------------------------------

/// Calculate idle gains for a given offline duration in seconds.
/// Mirrors the server formula from PROPOSAL.md:
/// - < 1h: 10 XP, 5 Gold
/// - 1-6h: 60 XP, 30 Gold
/// - 6-12h: 100 XP, 50 Gold
/// - 12-24h: 150 XP, 75 Gold
/// - After 24h: 150 XP, 75 Gold (capped)
///
/// Returns (xp, gold)
pub fn calculate_idle_gains(seconds_idle: u64) -> (u64, u64) {
    let hours = seconds_idle / 3600;

    if hours == 0 || seconds_idle < 3600 {
        // < 1 hour
        return (10, 5);
    } else if hours < 6 {
        // 1-6 hours
        return (60, 30);
    } else if hours < 12 {
        // 6-12 hours
        return (100, 50);
    } else if hours < 24 {
        // 12-24 hours: decay function
        let hours_excess = hours - 12;
        let gain = 150u64.saturating_sub(hours_excess * 75);
        return (gain, 75);
    } else {
        // Max: 24 hours
        return (MAX_IDLE_XP, MAX_IDLE_GOLD);
    }
}

// ---------------------------------------------------------------------------
// Console Logging Macro
// ---------------------------------------------------------------------------

/// Helper macro for economy logging (works in both dev and release)
macro_rules! econ_log {
    ($($arg:tt)*) => {
        println!("[ECONOMY] {}", format!($($arg)*));
    };
}

// ---------------------------------------------------------------------------
// Core Economy Functions
// ---------------------------------------------------------------------------

/// Apply idle gains to the player economy
pub fn apply_idle_gains(
    econ: &mut PlayerEconomy,
    last_login_time: u64,
) -> (u64, u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let seconds_offline = now.saturating_sub(last_login_time);
    econ.last_login_time = now;

    let (xp, gold) = calculate_idle_gains(seconds_offline);

    econ.xp += xp;
    econ.gold += gold;
    econ.last_logout_time = last_login_time;

    recalculate_level(econ);

    econ_log!(
        "  Idle gains applied: {} XP, {} Gold (was offline ~{}s)",
        xp,
        gold,
        seconds_offline,
    );

    (xp, gold)
}

/// Apply vehicle maintenance (5G/hour).
/// Applied daily at midnight server time.
pub fn apply_vehicle_maintenance(
    econ: &mut PlayerEconomy,
) -> Option<(u64, u64, String)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Check if daily check needs to run (every 24 hours)
    let elapsed = now.saturating_sub(econ.last_daily_gold_check);
    let hours_since_check = if elapsed > 0 {
        elapsed / 86400u64
    } else {
        0
    };

    let hours_online = now.saturating_sub(econ.last_logout_time) / 3600;

    if econ.vehicle.is_empty() || hours_online < 1 {
        return None;
    }

    // Calculate total maintenance owed
    let maintenance_hours = hours_since_check + hours_online;
    let maintenance_cost = (maintenance_hours as u64)
        .saturating_mul(VEHICLE_MAINTENANCE_RATE_PER_HOUR);

    if maintenance_cost > 0 {
        econ.gold = econ.gold.saturating_sub(maintenance_cost);
        econ.last_daily_gold_check = now;

        let vehicle_type = if econ.vehicle == "none" {
            "None".to_string()
        } else {
            econ.vehicle.clone()
        };

        econ_log!(
            "  Vehicle maintenance applied: {}G for {} hour(s) ({} on {}) - {}G deducted",
            maintenance_cost,
            maintenance_hours,
            vehicle_type,
            econ.vehicle,
            maintenance_cost,
        );

        Some((maintenance_cost, econ.gold, vehicle_type))
    } else {
        None
    }
}

/// Spend gold (checking balance first)
pub fn spend_gold(econ: &mut PlayerEconomy, amount: u64) -> bool {
    if econ.gold >= amount {
        econ.gold -= amount;
        econ_log!("  Spent {}G ({}G remaining)", amount, econ.gold);
        true
    } else {
        econ_log!(
            "  Not enough gold! Need {}G, have {}G",
            amount,
            econ.gold
        );
        false
    }
}

/// Add gold to economy
pub fn add_gold(econ: &mut PlayerEconomy, amount: u64) {
    econ.gold += amount;
    econ_log!("  Gained {}G - Total: {}G", amount, econ.gold);
}

/// Add XP to economy
pub fn add_xp(econ: &mut PlayerEconomy, amount: u64) {
    econ.xp += amount;
    econ_log!("  Gained {} XP - Total: {} XP", amount, econ.xp);

    // Check if leveled up
    if econ.xp >= econ.next_level_xp_needed {
        econ_log!(
            "  Level up! Reached level {} ({} XP, needed {})",
            econ.level + 1,
            econ.xp,
            econ.next_level_xp_needed
        );
        econ.level += 1;
        econ.next_level_xp_needed = xp_for_next_level(econ.level);
    }
}

/// Add eco points
pub fn add_eco_points(econ: &mut PlayerEconomy, amount: u64) {
    econ.eco_points = econ.eco_points.saturating_add(amount);
    econ_log!("  Gained {} Eco Points - Total: {}", amount, econ.eco_points);
}

/// Spend eco points
pub fn spend_eco_points(econ: &mut PlayerEconomy, amount: u64) -> bool {
    if econ.eco_points >= amount {
        econ.eco_points = econ.eco_points.saturating_sub(amount);
        econ_log!("  Spent {} Eco Points ({} remaining)", amount, econ.eco_points);
        true
    } else {
        econ_log!("  Not enough Eco Points! Need {}, have {}", amount, econ.eco_points);
        false
    }
}

/// Purchase a vehicle (using eco points to convert gold)
pub fn purchase_vehicle(
    econ: &mut PlayerEconomy,
    vehicle_name: &str,
) -> Option<(u64, f32)> {
    // Find vehicle definition
    let vehicle_def = VEHICLE_DEFINITIONS.iter()
        .find(|v| v.cost > 0 && v.cost == 0)
        .or_else(|| VEHICLE_DEFINITIONS.iter()
            .find(|v| &v.cost.to_string() == vehicle_name))
        .or_else(|| VEHICLE_DEFINITIONS.iter()
            .find(|v| v.speed_multiplier > 1.0));

    if let Some(v) = vehicle_def {
        let cost = v.cost;
        if econ.eco_points >= cost {
            econ.eco_points -= cost;
            econ.gold = econ.gold.saturating_add(cost);
            econ.vehicle = vehicle_name.to_string();
            econ_log!(
                "  Purchased vehicle: {} for {} Eco Points (speed: {}x)",
                vehicle_name,
                cost,
                v.speed_multiplier,
            );
            Some((cost, v.speed_multiplier))
        } else {
            econ_log!(
                "  Cannot purchase {}: need {} eco points",
                vehicle_name,
                cost,
            );
            None
        }
    } else {
        econ_log!("  Unknown vehicle: {}", vehicle_name);
        None
    }
}

// ---------------------------------------------------------------------------
// Game State (wrapper used by main.rs)
// ---------------------------------------------------------------------------

/// Single-player local game state
#[derive(Debug, Default)]
pub struct LocalGameState {
    pub player_address: String,
    pub economy: PlayerEconomy,
    /// Cooldown tracking (action -> last_triggered_time)
    pub cooldowns: HashMap<String, u64>,
    /// Current hex the player occupies
    pub current_hex_id: u64,
    /// Nearby hexes for teleport
    pub nearby_hexes: Vec<(u64, String)>,
    /// Gold display amount
    pub gold: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    /// Cooldown for actions (5 seconds)
    pub last_action_time: u64,
    /// Actions taken recently
    pub actions_history: Vec<String>,
    /// Current hex's terrain type
    pub current_terrain: String,
    /// Current hex's pollution status
    pub is_polluted: bool,
    /// Teleport state (cooldown tracking + persistence)
    pub teleport_state: crate::teleport::TeleportState,
}

impl LocalGameState {
    pub fn new(address: &str) -> Self {
        Self {
            player_address: address.to_string(),
            economy: PlayerEconomy::default(),
            cooldowns: HashMap::new(),
            current_hex_id: 0,
            nearby_hexes: Vec::new(),
            gold: 0,
            xp: 0,
            level: 0,
            eco_points: 0,
            last_action_time: 0,
            actions_history: Vec::new(),
            current_terrain: "Grass".to_string(),
            is_polluted: false,
            teleport_state: crate::teleport::TeleportState::new(),
        }
    }

    /// Check if action is available (cooldown check)
    pub fn can_act(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let elapsed = now.saturating_sub(self.last_action_time);
        elapsed >= 5  // 5 second cooldown
    }

    /// Record an action timestamp
    pub fn record_action(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_action_time = now;
        self.actions_history.push(format!(
            "[{}] {}",
            self.player_address,
            self.last_action_time
        ));
    }

    /// Record an action (timestamp + name)
    pub fn record_action_named(&mut self, name: &str) {
        self.record_action();
        econ_log!("  ACTION: {} at {}", self.player_address, name);
    }

    /// Update display state from economy
    pub fn refresh_display(&mut self) {
        self.gold = self.economy.gold;
        self.xp = self.economy.xp;
        self.level = self.economy.level;
        self.eco_points = self.economy.eco_points;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_teleport_cost_scaling() {
        // teleport_cost(level) = min(100 * sqrt(level), level^2)
        assert_eq!(teleport_cost(1), 1);      // min(100, 1) = 1
        assert_eq!(teleport_cost(4), 16);     // min(200, 16) = 16
        assert_eq!(teleport_cost(9), 81);     // min(300, 81) = 81
        assert_eq!(teleport_cost(100), 1000); // min(1000, 10000) = 1000
        assert_eq!(teleport_cost(256), 1600);  // min(1600, 65536) = 1600
    }

    #[test]
    fn test_level_calculation() {
        // xp_for_next_level(level) = 100 * level^2
        assert_eq!(calculate_level(0), 1);     // No XP, level 1
        assert_eq!(calculate_level(100), 2);   // 100 XP = level 1→2
        assert_eq!(calculate_level(499), 2);   // Less than 400 XP for level 3
        assert_eq!(calculate_level(500), 3);   // 100 + 400 = 500 XP = level 3
        assert_eq!(calculate_level(1399), 3);  // Less than 900 XP for level 4
        assert_eq!(calculate_level(1400), 4);  // 100 + 400 + 900 = 1400 XP = level 4
    }

    #[test]
    fn test_idle_gains() {
        assert_eq!(calculate_idle_gains(0), (10, 5));
        assert_eq!(calculate_idle_gains(3600), (60, 30));
        assert_eq!(calculate_idle_gains(7200), (60, 30));
        assert_eq!(calculate_idle_gains(10800), (60, 30));
        assert_eq!(calculate_idle_gains(21600), (100, 50));
        assert_eq!(calculate_idle_gains(36000), (100, 50));
        assert_eq!(calculate_idle_gains(43200), (150, 75));
        assert_eq!(calculate_idle_gains(86400), (150, 75));
    }
}
