//! Sistema de Progressão — Níveis & Unlocks

use super::types::PlayerDbEntry;
use spacetimedb::ReducerContext;
use crate::types::player;

/// Level up system — quando XP acumulado >= xp_for_next_level(level), sobe de nível
pub fn check_level_up(ctx: &ReducerContext, player_address: &str) -> Option<u32> {
    let player = ctx.db.player().iter()
        .find(|p| p.address == player_address)?;

    let next_level_xp = player.level as u64 * 100 * player.level as u64; // 100 * L²

    if player.xp >= next_level_xp {
        let new_level = player.level + 1;
        let mut player = player;
        player.level = new_level;
        ctx.db.player().address().update(player);
        Some(new_level)
    } else {
        None
    }
}

/// Desbloqueios por nível
pub fn unlocks_for_level(level: u32) -> Vec<Unlock> {
    match level {
        1 => vec![Unlock::BasicFarming],
        2 => vec![Unlock::Bicycle],
        3 => vec![Unlock::Scooter],
        5 => vec![Unlock::Motorcycle],
        7 => vec![Unlock::Airplane],
        10 => vec![Unlock::PremiumMarket],
        _ => vec![],
    }
}

/// Verificar unlocks disponíveis para um jogador
pub fn available_unlocks(player: &PlayerDbEntry) -> Vec<Unlock> {
    let mut unlocks = Vec::new();

    for level in 1..=player.level {
        for unlock in unlocks_for_level(level) {
            unlocks.push(unlock);
        }
    }

    unlocks
}

/// Tipos de unlock
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Unlock {
    BasicFarming,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
    PremiumMarket,
}

impl Unlock {
    pub fn to_string(&self) -> String {
        match self {
            Unlock::BasicFarming => "basic_farming".to_string(),
            Unlock::Bicycle => "bicycle".to_string(),
            Unlock::Scooter => "scooter".to_string(),
            Unlock::Motorcycle => "motorcycle".to_string(),
            Unlock::Boat => "boat".to_string(),
            Unlock::Airplane => "airplane".to_string(),
            Unlock::PremiumMarket => "premium_market".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Unlock::BasicFarming => "Basic Farming",
            Unlock::Bicycle => "Electric Bicycle",
            Unlock::Scooter => "Electric Scooter",
            Unlock::Motorcycle => "Electric Motorcycle",
            Unlock::Boat => "Electric Boat",
            Unlock::Airplane => "Electric Airplane",
            Unlock::PremiumMarket => "Premium Market Access",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Unlock::BasicFarming => "Unlock ability to plant and harvest crops",
            Unlock::Bicycle => "Unlock electric bicycle (2x speed)",
            Unlock::Scooter => "Unlock electric scooter (3x speed)",
            Unlock::Motorcycle => "Unlock electric motorcycle (5x speed)",
            Unlock::Boat => "Unlock electric boat (4x speed on water)",
            Unlock::Airplane => "Unlock electric airplane (10x speed)",
            Unlock::PremiumMarket => "Access premium market (500 template slots)",
        }
    }
}
