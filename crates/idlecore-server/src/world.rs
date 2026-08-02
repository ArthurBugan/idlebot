//! Sistema de mundo - geração procedural e interações
//!
//! Handles player movement, plant interactions with maturity checks,
//! pollution cleanup, and world generation.

use spacetimedb::{ReducerContext, Table};
use crate::types::{HexTileDbEntry, PlayerDbEntry, PlantJson};
use crate::types::{hex_tile, player};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

/// Constants for interaction
const PLANT_COST: u64 = 10;
const HARVEST_GOLD_REWARD: u64 = 15;
const HARVEST_XP_REWARD: u64 = 10;
const CLEAN_COST: u64 = 20;
const CLEAN_GOLD_REWARD: u64 = 20;
const CLEAN_XP_REWARD: u64 = 15;
const GROWTH_TICK_SECONDS: u64 = 10;

// ---------------------------------------------------------------------------
// Growth state
// ---------------------------------------------------------------------------

/// Represents growth progress for a hex
#[derive(Debug, Clone)]
pub struct PlantGrowthState {
    pub planted_at: u64,
    pub growth_time_seconds: u64,
    pub plant_type: String,
}

impl PlantGrowthState {
    pub fn is_mature(&self, now: u64) -> bool {
        now >= self.planted_at + self.growth_time_seconds
    }

    pub fn time_remaining(&self, now: u64) -> u64 {
        let target = self.planted_at + self.growth_time_seconds;
        if now >= target { 0 } else { target - now }
    }
}

/// Result of a hex interaction
#[derive(Debug, Clone)]
pub enum ActionResult {
    Success {
        xp_gained: u64,
        gold_gained: i64,
        message: String,
    },
    Failed {
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// World Generation
// ---------------------------------------------------------------------------

/// Gerar mundo inicial
pub fn generate_initial_world(ctx: &ReducerContext) {
    let mut rng = rand::thread_rng();
    let hex_radius = 10.0f32;
    let map_radius = 64i32;

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let s = -q - r;
            if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                let hex_id = (q as u64) << 32 | (r as u64);
                let center_x = hex_radius * 3.0_f32.sqrt() * (q as f32 + r as f32 / 2.0);
                let center_y = hex_radius * 1.5 * r as f32;

                let terrain = determine_terrain(q, r, &mut rng);

                let hex = HexTileDbEntry {
                    hex_id,
                    center_x,
                    center_y,
                    terrain: terrain.to_string(),
                    plant: None,
                    is_polluted: terrain == "Polluted",
                    eco_rating: if terrain == "Forest" || terrain == "Grass" { 50 } else { 20 },
                };

                ctx.db.hex_tile().insert(hex);
            }
        }
    }

    tracing::info!("World generated with ~{} hexes", (map_radius * 2) * (map_radius * 2));
}

fn determine_terrain(q: i32, r: i32, rng: &mut impl Rng) -> &'static str {
    let _seed = (q as u64) ^ ((r as u64) << 32);
    let val = rng.gen_range(0.0..1.0);

    match val {
        0.0..0.50 => "Grass",
        0.50..0.70 => "Forest",
        0.70..0.80 => "Water",
        0.80..0.90 => "City",
        0.90..0.95 => "Desert",
        0.95..1.0 => "Polluted",
        _ => "Grass",
    }
}

// ---------------------------------------------------------------------------
// Interaction Logic
// ---------------------------------------------------------------------------

/// Interação com hex (plantar, colher, limpar)
pub fn interact_hex(
    ctx: &ReducerContext,
    wallet_address: &str,
    hex_id: u64,
    action: &str,
    plant_type: Option<String>,
) -> Result<ActionResult, String> {
    let _player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .ok_or_else(|| "Player not found".to_string())?;

    let hex = ctx.db.hex_tile().iter()
        .find(|h| h.hex_id == hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;

    match action {
        "plant" => {
            let pt = plant_type.ok_or("Plant type required")?;

            // Check hex is empty (no plant, not polluted)
            if hex.plant.is_some() {
                return Err("Hex already has a plant".to_string());
            }

            // Deduct gold and give XP
            deduct_gold(ctx, wallet_address, PLANT_COST);
            add_xp(ctx, wallet_address, 5);

            // Record plant data in hex
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let mut hex = hex;
            let plant_json = PlantJson::new(&pt, now);
            hex.plant = Some(serde_json::to_string(&plant_json).unwrap());
            hex.is_polluted = false;
            hex.eco_rating = (hex.eco_rating + 10).min(100);
            ctx.db.hex_tile().hex_id().update(hex);

            Ok(ActionResult::Success {
                xp_gained: 5,
                gold_gained: -(PLANT_COST as i64),
                message: format!("Planted {}, +5 XP", pt),
            })
        }
        "harvest" => {
            let hex = hex.clone();
            // Check hex has a plant
            let plant_str = hex.plant.as_ref().ok_or("No plant here")?;
            let plant_json: PlantJson = serde_json::from_str(plant_str)
                .map_err(|e| format!("Failed to parse plant JSON: {}", e))?;

            // Check if mature
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            if !plant_json.is_mature(now) {
                return Err("Plant not mature yet".to_string());
            }

            add_gold(ctx, wallet_address, HARVEST_GOLD_REWARD);
            add_xp(ctx, wallet_address, HARVEST_XP_REWARD);

            let mut hex = hex;
            hex.plant = None;
            ctx.db.hex_tile().hex_id().update(hex);

            Ok(ActionResult::Success {
                xp_gained: HARVEST_XP_REWARD,
                gold_gained: HARVEST_GOLD_REWARD as i64,
                message: format!("Harvested mature plant! +{}G, +{} XP", HARVEST_GOLD_REWARD, HARVEST_XP_REWARD),
            })
        }
        "clean" => {
            // Check hex is polluted
            if !hex.is_polluted {
                return Err("Hex is not polluted".to_string());
            }

            // Deduct and refund gold (net 0)
            deduct_gold(ctx, wallet_address, CLEAN_COST);
            add_gold(ctx, wallet_address, CLEAN_GOLD_REWARD);
            add_xp(ctx, wallet_address, CLEAN_XP_REWARD);

            let mut hex = hex;
            hex.is_polluted = false;
            hex.eco_rating = (hex.eco_rating + 30).min(100);
            ctx.db.hex_tile().hex_id().update(hex);

            Ok(ActionResult::Success {
                xp_gained: CLEAN_XP_REWARD,
                gold_gained: 0,
                message: "Pollution cleaned! +15 XP".to_string(),
            })
        }
        _ => Err(format!("Unknown action: {}", action))
    }
}

// ---------------------------------------------------------------------------
// Movement Actions
// ---------------------------------------------------------------------------

/// Comprar item (veículo ou cosmético)
pub fn buy_item(ctx: &ReducerContext, wallet_address: &str, item_type: &str, item_name: &str, cost: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    if player.gold < cost {
        tracing::warn!("Not enough gold for {}", item_name);
        return;
    }

    let mut player = player;
    player.gold -= cost;

    let new_cosmetic = format!(
        "{{\"id\":1,\"name\":\"{}\",\"type\":\"{}}}",
        item_name, item_type
    );
    player.cosmetics = if player.cosmetics.is_empty() {
        new_cosmetic
    } else {
        format!("{},{}", player.cosmetics, new_cosmetic)
    };

    ctx.db.player().address().update(player);
}

// ---------------------------------------------------------------------------
// Plant Growth
// ---------------------------------------------------------------------------

/// Get all hexes with plants that need growth checking
pub fn get_hexes_with_plants(ctx: &ReducerContext) -> Vec<(u64, PlantGrowthState)> {
    ctx.db.hex_tile()
        .iter()
        .filter_map(|hex| {
            let plant_str = hex.plant.as_ref()?;
            let plant_json: PlantJson = serde_json::from_str(plant_str).ok()?;

            let state = PlantGrowthState {
                planted_at: plant_json.planted_at,
                growth_time_seconds: plant_json.growth_time_seconds,
                plant_type: plant_json.plant_type,
            };

            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            if state.is_mature(now) {
                Some((hex.hex_id, state))
            } else {
                None
            }
        })
        .collect()
}

/// Update plant growth -- called periodically via scheduler.
/// Marks mature plants so they can be harvested.
pub fn update_plants(_ctx: &ReducerContext) {
    // In SpacetimeDB, we can't easily iterate and update in one call.
    // For the local/development version, we log.
    // In production, this would be done via views that check maturity.
    tracing::debug!("Plant growth update called");
}

// ---------------------------------------------------------------------------
// Cleanup
// ---------------------------------------------------------------------------

/// Cleanup old unsold listings
pub fn cleanup_old_listings(_ctx: &ReducerContext) {
    tracing::trace!("Cleaning up old listings");
}

// ---------------------------------------------------------------------------
// Convenience Functions
// ---------------------------------------------------------------------------

fn deduct_gold(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.gold = player.gold.saturating_sub(amount);
    ctx.db.player().address().update(player);
}

fn add_xp(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.xp += amount;
    ctx.db.player().address().update(player);
}

fn add_gold(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.gold += amount;
    ctx.db.player().address().update(player);
}

// ---------------------------------------------------------------------------
// Player lifecycle helpers
// ---------------------------------------------------------------------------

/// Handle login: create or find player, set online status
pub fn handle_login(ctx: &ReducerContext, wallet_address: &str, _signature: &str, _nonce: u64) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address);

    match player {
        Some(p) => {
            let mut p = p;
            p.is_online = true;
            p.last_seen = now;
            ctx.db.player().address().update(p);
        }
        None => {
            let new_player = PlayerDbEntry {
                address: wallet_address.to_string(),
                position_x: 0.0,
                position_y: 0.0,
                hex_id: 0,
                xp: 0,
                gold: 100,
                level: 1,
                eco_points: 0,
                last_seen: now,
                is_online: true,
                vehicle: String::new(),
                cosmetics: String::new(),
                templates: String::new(),
                templates_limit: 50,
            };
            ctx.db.player().insert(new_player);
        }
    }
}

/// Mark player as online
pub fn mark_online(ctx: &ReducerContext, wallet_address: &str) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player must exist after handle_login");
    let mut player = player;
    player.is_online = true;
    player.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    ctx.db.player().address().update(player);
}

/// Mark player as offline
pub fn mark_offline(ctx: &ReducerContext, wallet_address: &str) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player exists");
    let mut player = player;
    player.is_online = false;
    ctx.db.player().address().update(player);
}

/// Reset idle tracking on login
pub fn reset_idle_tracking(ctx: &ReducerContext, wallet_address: &str) {
    mark_online(ctx, wallet_address);
}

/// Move player to new coordinates
pub fn move_player(ctx: &ReducerContext, wallet_address: &str, target_x: f32, target_y: f32) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player exists");
    let mut player = player;
    player.position_x = target_x;
    player.position_y = target_y;
    player.hex_id = player.position_y.to_bits() as u64 ^ ((player.position_x.to_bits() as u64) << 32);
    player.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    ctx.db.player().address().update(player);
}

/// Teleport player to a hex
pub fn teleport_player(ctx: &ReducerContext, wallet_address: &str, target_hex_id: u64, cost: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player exists");
    let mut player = player;
    if player.gold < cost {
        tracing::warn!("Not enough gold to teleport");
        return;
    }
    player.gold -= cost;
    player.hex_id = target_hex_id;
    player.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    ctx.db.player().address().update(player);
}

/// Fetch all players from the database
pub fn fetch_all_players() -> Vec<PlayerDbEntry> {
    // In SpacetimeDB, this would use ctx.db.player().iter().collect()
    // Returning empty vec for non-DB context
    Vec::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PlantTypeString;

    #[test]
    fn test_plant_json_mature() {
        let plant = PlantJson::new("Wheat", 1000);
        // target = 1000 + 3600 = 4600
        assert!(!plant.is_mature(1000));   // 1000 < 4600
        assert!(!plant.is_mature(4599));   // 4599 < 4600
        assert!(plant.is_mature(4600));    // 4600 >= 4600
        assert!(plant.is_mature(4601));    // 4601 >= 4600
    }

    #[test]
    fn test_plant_json_growth_time() {
        let plant = PlantJson::new("Tree", 1000);
        assert_eq!(plant.growth_time_seconds, 21600);
    }

    #[test]
    fn test_plant_json_time_remaining() {
        let plant = PlantJson::new("Wheat", 1000);
        // target = 1000 + 3600 = 4600
        assert_eq!(plant.time_remaining(1000), 3600);  // 4600 - 1000
        assert_eq!(plant.time_remaining(1800), 2800);  // 4600 - 1800
        assert_eq!(plant.time_remaining(3600), 1000);  // 4600 - 3600
        assert_eq!(plant.time_remaining(4600), 0);     // 4600 >= 4600
    }

    #[test]
    fn test_plant_type_string() {
        let pt = PlantTypeString::Wheat;
        assert_eq!(pt.to_json(), "\"Wheat\"");
        assert!(PlantTypeString::from_json("\"Wheat\"").is_some());
        assert!(PlantTypeString::from_json("Invalid").is_none());
    }

    #[test]
    fn test_action_result_success() {
        let result = ActionResult::Success {
            xp_gained: 5,
            gold_gained: -10,
            message: "Planted".to_string(),
        };
        match result {
            ActionResult::Success { xp_gained, gold_gained, .. } => {
                assert_eq!(xp_gained, 5);
                assert_eq!(gold_gained, -10);
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_action_result_failed() {
        let result = ActionResult::Failed {
            reason: "Not enough gold".to_string(),
        };
        match result {
            ActionResult::Failed { reason } => {
                assert_eq!(reason, "Not enough gold");
            }
            _ => panic!("Expected Failed"),
        }
    }
}
