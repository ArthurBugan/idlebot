//! Sistema de mundo - geração procedural e interações

use super::types::*;
use spacetimedb::{ReducerContext, Table};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Determinar terreno baseado em coordenadas
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

/// Handle login - create player if new
pub fn handle_login(
    ctx: &ReducerContext,
    wallet_address: &str,
    _signature: &str,
    _nonce: u64,
) {
    let exists = ctx.db.player().iter().any(|p| p.address == wallet_address);
    if !exists {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let player = PlayerDbEntry {
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
            templates_limit: 10,
        };
        ctx.db.player().insert(player);
    }
}

/// Mark player offline
pub fn mark_offline(ctx: &ReducerContext, wallet_address: &str) {
    for mut player in ctx.db.player().iter() {
        if player.address == wallet_address {
            player.is_online = false;
            player.last_seen = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            ctx.db.player().address().update(player);
            break;
        }
    }
}

/// Mover jogador
pub fn move_player(ctx: &ReducerContext, wallet_address: &str, target_x: f32, target_y: f32) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player must exist to move");

    let mut player = player;
    player.position_x = target_x;
    player.position_y = target_y;
    player.hex_id = calculate_hex_id(target_x, target_y);
    player.last_seen = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    ctx.db.player().address().update(player);
}

/// Teleportar jogador
pub fn teleport_player(ctx: &ReducerContext, wallet_address: &str, target_hex_id: u64, cost: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player must exist to teleport");

    if player.gold < cost {
        tracing::warn!("Not enough gold for teleport");
        return;
    }

    let mut player = player;
    player.gold -= cost;
    player.hex_id = target_hex_id;

    let q = (target_hex_id >> 32) as i32;
    let r = (target_hex_id & 0xFFFFFFFF) as i32;
    let hex_radius = 10.0;
    player.position_x = hex_radius * 3.0_f32.sqrt() * (q as f32 + r as f32 / 2.0);
    player.position_y = hex_radius * 1.5 * r as f32;

    ctx.db.player().address().update(player);
}

/// Calcular hex_id baseado em posição
fn calculate_hex_id(x: f32, y: f32) -> u64 {
    let hex_radius = 10.0;
    let q = (x * 2.0 / (3.0_f32.sqrt() * hex_radius)) as i64;
    let r = (y * 2.0 / (3.0 * hex_radius * 0.75)) as i64;
    (q as u64) << 32 | (r as u64)
}

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
            if let Some(plant_type) = plant_type {
                if hex.plant.is_some() {
                    return Err("Hex already has a plant".to_string());
                }

                let plant_json = format!(
                    "{{\"type\":\"{}\",\"stage\":\"Planted\",\"planted_at\":{}}}",
                    plant_type,
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                );

                let mut hex = hex;
                hex.plant = Some(plant_json);
                hex.is_polluted = false;
                hex.eco_rating = (hex.eco_rating + 10).min(100);
                ctx.db.hex_tile().hex_id().update(hex);

                deduct_gold(ctx, wallet_address, 10);

                Ok(ActionResult::Success {
                    xp_gained: 5,
                    gold_gained: 0,
                    message: format!("Planted {}", plant_type),
                })
            } else {
                Err("Plant type required".to_string())
            }
        }
        "harvest" => {
            let hex = hex; // take ownership for match
            if let Some(_plant_json) = &hex.plant {
                add_xp(ctx, wallet_address, 10);
                add_gold(ctx, wallet_address, 15);

                let mut hex = hex;
                hex.plant = None;
                ctx.db.hex_tile().hex_id().update(hex);

                Ok(ActionResult::Success {
                    xp_gained: 10,
                    gold_gained: 15,
                    message: "Harvested successfully!".to_string(),
                })
            } else {
                Err("No plant here".to_string())
            }
        }
        "clean" => {
            let hex = hex;
            if !hex.is_polluted {
                return Err("Not polluted".to_string());
            }

            deduct_gold(ctx, wallet_address, 20);

            let mut hex = hex;
            hex.is_polluted = false;
            hex.eco_rating = (hex.eco_rating + 30).min(100);
            ctx.db.hex_tile().hex_id().update(hex);

            add_xp(ctx, wallet_address, 15);

            Ok(ActionResult::Success {
                xp_gained: 15,
                gold_gained: 20,
                message: "Pollution cleaned!".to_string(),
            })
        }
        "clear" => {
            deduct_gold(ctx, wallet_address, 15);

            let mut hex = hex;
            hex.eco_rating = (hex.eco_rating + 5).min(100);
            ctx.db.hex_tile().hex_id().update(hex);

            Ok(ActionResult::Success {
                xp_gained: 5,
                gold_gained: 0,
                message: "Terrain cleared!".to_string(),
            })
        }
        "teleport" => {
            Ok(ActionResult::Success {
                xp_gained: 0,
                gold_gained: 0,
                message: "Teleported!".to_string(),
            })
        }
        _ => Err(format!("Unknown action: {}", action))
    }
}

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
        "{{\"id\":1,\"name\":\"{}\",\"type\":\"{}\"}}",
        item_name, item_type
    );
    player.cosmetics = if player.cosmetics.is_empty() {
        new_cosmetic
    } else {
        format!("{},{}", player.cosmetics, new_cosmetic)
    };

    ctx.db.player().address().update(player);
}

/// Atualizar crescimento de plantas
pub fn update_plants(_ctx: &ReducerContext) {
    tracing::trace!("Updated plant growth");
}

/// Calcular idle gains
pub fn calculate_idle_gains(_ctx: &ReducerContext) {
    tracing::trace!("Calculating idle gains");
}

/// Deduzir gold
fn deduct_gold(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.gold = player.gold.saturating_sub(amount);
    ctx.db.player().address().update(player);
}

/// Adicionar XP
fn add_xp(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.xp += amount;
    ctx.db.player().address().update(player);
}

/// Adicionar gold
fn add_gold(ctx: &ReducerContext, wallet_address: &str, amount: u64) {
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    let mut player = player;
    player.gold += amount;
    ctx.db.player().address().update(player);
}

/// Resultado de uma ação
pub enum ActionResult {
    Success {
        xp_gained: u64,
        gold_gained: u64,
        message: String,
    },
    Failed {
        reason: String,
    },
}
