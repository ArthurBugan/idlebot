//! Sistema de mundo - geração procedural e interações

use super::types::*;
use std::collections::HashMap;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

/// Gerar mundo inicial
pub fn generate_initial_world() {
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
                
                db::hex_tile::table().insert(hex);
            }
        }
    }
    
    tracing::info!("World generated with ~{} hexes", (map_radius * 2) ^ 2);
}

/// Determinar terreno baseado em coordenadas
fn determine_terrain(q: i32, r: i32, rng: &mut impl Rng) -> &'static str {
    let seed = (q as u64) ^ ((r as u64) << 32);
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

/// Mover jogador
pub fn move_player(wallet_address: &str, target_x: f32, target_y: f32) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player must exist to move");
    
    let mut player = player;
    player.position_x = target_x;
    player.position_y = target_y;
    player.hex_id = calculate_hex_id(target_x, target_y);
    
    db::player::table().update(player);
    
    hex_changed::publish(());
}

/// Teleportar jogador
pub fn teleport_player(wallet_address: &str, target_hex_id: u64, cost: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player must exist to teleport");
    
    if player.gold < cost {
        tracing::warn!("Not enough gold for teleport");
        return;
    }
    
    let mut player = player;
    player.gold -= cost;
    player.hex_id = target_hex_id;
    
    // Converter hex_id pra posição
    let q = (target_hex_id >> 32) as i32;
    let r = (target_hex_id & 0xFFFFFFFF) as i32;
    let hex_radius = 10.0;
    player.position_x = hex_radius * 3.0_f32.sqrt() * (q as f32 + r as f32 / 2.0);
    player.position_y = hex_radius * 1.5 * r as f32;
    
    db::player::table().update(player);
    
    hex_changed::publish(());
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
    wallet_address: &str,
    hex_id: u64,
    action: &str,
    plant_type: Option<String>,
) -> Result<ActionResult, String> {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .map_err(|_| "Player not found")?;
    
    let hex: HexTileDbEntry = db::hex_tile::table()
        .get(hex_id)
        .ok_or("Hex not found")?;
    
    match action {
        "plant" => {
            if let Some(plant_type) = plant_type {
                if hex.plant.is_some() {
                    return Err("Hex already has a plant");
                }
                
                let plant_json = format!("{{\"type\":\"{}\",\"stage\":\"Planted\",\"planted_at\":{}}}", 
                    plant_type, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
                
                let mut hex = hex;
                hex.plant = Some(plant_json);
                hex.is_polluted = false;
                hex.eco_rating = (hex.eco_rating + 10).min(100);
                db::hex_tile::table().update(hex);
                
                deduct_gold(wallet_address, 10);
                
                Ok(ActionResult::Success {
                    xp_gained: 5,
                    gold_gained: 0,
                    message: format!("Planted {}", plant_type),
                })
            } else {
                Err("Plant type required")
            }
        }
        "harvest" => {
            if let Some(plant_json) = &hex.plant {
                // Simplificação - em produção faria parsing JSON
                add_xp(wallet_address, 10);
                add_gold(wallet_address, 15);
                
                let mut hex = hex;
                hex.plant = None;
                db::hex_tile::table().update(hex);
                
                Ok(ActionResult::Success {
                    xp_gained: 10,
                    gold_gained: 15,
                    message: "Harvested successfully!".to_string(),
                })
            } else {
                Err("No plant here")
            }
        }
        "clean" => {
            if !hex.is_polluted {
                return Err("Not polluted");
            }
            
            deduct_gold(wallet_address, 20);
            
            let mut hex = hex;
            hex.is_polluted = false;
            hex.eco_rating = (hex.eco_rating + 30).min(100);
            db::hex_tile::table().update(hex);
            
            add_xp(wallet_address, 15);
            
            Ok(ActionResult::Success {
                xp_gained: 15,
                gold_gained: 20,
                message: "Pollution cleaned!".to_string(),
            })
        }
        "clear" => {
            deduct_gold(wallet_address, 15);
            
            let mut hex = hex;
            hex.eco_rating = (hex.eco_rating + 5).min(100);
            db::hex_tile::table().update(hex);
            
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
pub fn buy_item(wallet_address: &str, item_type: &str, item_name: &str, cost: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    if player.gold < cost {
        tracing::warn!("Not enough gold for {}", item_name);
        return;
    }
    
    let mut player = player;
    player.gold -= cost;
    
    // Adicionar item ao inventário (simplificação)
    let new_cosmetic = format!("{{\"id\":1,\"name\":\"{}\",\"type\":\"{}\"}}", item_name, item_type);
    player.cosmetics = if player.cosmetics.is_empty() {
        new_cosmetic
    } else {
        format!("{},{}", player.cosmetics, new_cosmetic)
    };
    
    db::player::table().update(player);
}

/// Atualizar crescimento de plantas
pub fn update_plants() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Aqui seria o loop de verificação de plantas prontas
    // Simplificado para o exemplo
    tracing::trace!("Updated plant growth");
}

/// Deduzir gold
fn deduct_gold(wallet_address: &str, amount: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    let mut player = player;
    player.gold = player.gold.saturating_sub(amount);
    db::player::table().update(player);
}

/// Adicionar XP
fn add_xp(wallet_address: &str, amount: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    let mut player = player;
    player.xp += amount;
    db::player::table().update(player);
}

/// Adicionar gold
fn add_gold(wallet_address: &str, amount: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    let mut player = player;
    player.gold += amount;
    db::player::table().update(player);
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
