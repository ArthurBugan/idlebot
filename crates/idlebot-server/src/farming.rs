//! Sistema de Farming — Plantar/Colher (Stardew Valley style)

use super::types::*;
use idlebot_core::{PlantType, PlantStage};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

/// Plantar semente num hexágono
pub fn plant_seed(
    player_address: &str,
    hex_id: u64,
    plant_type: PlantType,
) -> crate::world::ActionResult {
    let hex: HexTileDbEntry = db::hex_tile::table()
        .get(hex_id)
        .ok_or_else(|| crate::world::ActionResult::Failed {
            reason: "Hex not found".to_string(),
        })?;

    if hex.plant.is_some() {
        return crate::world::ActionResult::Failed {
            reason: "Hex already has a plant".to_string(),
        };
    }

    let plant_json = format!(
        "{{\"type\":\"{}\",\"stage\":\"Planted\",\"planted_at\":{}}}",
        plant_type as u8,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let mut hex = hex;
    hex.plant = Some(plant_json);
    hex.is_polluted = false;
    hex.eco_rating = (hex.eco_rating + 10).min(100);
    db::hex_tile::table().update(hex);

    crate::world::deduct_gold(player_address, 10);

    crate::world::ActionResult::Success {
        xp_gained: 5,
        gold_gained: 0,
        message: format!("Planted {:?}", plant_type),
    }
}

/// Colher planta pronta
pub fn harvest(
    hex_id: u64,
    player_address: &str,
) -> crate::world::ActionResult {
    let hex: HexTileDbEntry = db::hex_tile::table()
        .get(hex_id)
        .ok_or_else(|| crate::world::ActionResult::Failed {
            reason: "Hex not found".to_string(),
        })?;

    if hex.plant.is_none() {
        return crate::world::ActionResult::Failed {
            reason: "No plant here".to_string(),
        };
    }

    // Determinar rewards baseado no tipo de planta
    let (xp, gold, eco) = match hex.terrain {
        "Grass" => (10, 15, 0),
        "Forest" => (20, 40, 5),
        "City" => (5, 10, 0),
        _ => (10, 15, 0),
    };

    let mut hex = hex;
    hex.plant = None;
    db::hex_tile::table().update(hex);

    crate::world::add_xp(player_address, xp);
    crate::world::add_gold(player_address, gold);
    crate::world::add_eco_points(player_address, eco);

    crate::world::ActionResult::Success {
        xp_gained: xp,
        gold_gained: gold,
        message: "Harvested successfully!".to_string(),
    }
}

/// Atualizar crescimento de plantas (server-side timer)
pub fn update_plant_growth() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Buscar todas as plantas e verificar se estão prontas
    let plants: Vec<HexTileDbEntry> = db::hex_tile::table().collect();

    for hex in plants {
        if let Some(plant_json) = &hex.plant {
            // Parsing simplificado — em produção usar serde_json
            let planted_at = extract_planted_at(plant_json);
            let grow_time = estimate_grow_time(hex.terrain.as_str());

            if now >= planted_at + grow_time {
                // Planta está pronta!
                let mut hex = hex;
                hex.plant = Some(format!("{{\"ready\":true}}"));
                db::hex_tile::table().update(hex);
            }
        }
    }
}

/// Extrair timestamp de plantio do JSON (simplificado)
fn extract_planted_at(json: &str) -> u64 {
    if let Some(start) = json.find("\"planted_at\":") {
        let end = json[start..].find(',').unwrap_or(json.len() - start);
        json[start + 13..start + end].parse::<u64>().unwrap_or(0)
    } else {
        0
    }
}

/// Estimativa de tempo de crescimento (segundos)
fn estimate_grow_time(terrain: &str) -> u64 {
    match terrain {
        "Grass" => 300,     // 5 min
        "Forest" => 900,    // 15 min
        "City" => 1800,     // 30 min
        _ => 600,           // 10 min default
    }
}
