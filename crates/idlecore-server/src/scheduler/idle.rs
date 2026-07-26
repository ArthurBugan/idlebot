//! Sistema de Idle — Ganhos Offline

use idlebot_core::idle_config;
use idlebot_core::Player;
use std::time::{SystemTime, UNIX_EPOCH};

/// Calcular e distribuir idle gains periodicamente
/// Executado pelo SpacetimeDB scheduler a cada 5 minutos
pub fn process_idle_gains(players: &mut Vec<Player>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for player in players.iter_mut() {
        if player.is_online {
            continue;
        }

        let elapsed = now.saturating_sub(player.last_seen);
        let capped = elapsed.min(idle_config::MAX_IDLE_SECONDS);

        if capped == 0 {
            continue;
        }

        let gains = idle_config::gains_for_time(std::time::Duration::from_secs(capped));

        player.xp += gains.xp;
        player.gold += gains.gold;
        player.level = Player::calculate_level(player.xp);
    }
}

/// Notificação de idle gains (mostrada quando jogador logar)
pub fn check_idle_notification(player: &Player) -> Option<IdleNotification> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let elapsed = now.saturating_sub(player.last_seen);
    if elapsed < 3600 {
        return None; // Menos de 1 hora offline
    }

    let gains = idle_config::gains_for_time(std::time::Duration::from_secs(
        elapsed.min(idle_config::MAX_IDLE_SECONDS)
    ));

    Some(IdleNotification {
        xp_gained: gains.xp,
        gold_gained: gains.gold,
        hours_offline: elapsed / 3600,
    })
}

#[derive(Debug, Clone)]
pub struct IdleNotification {
    pub xp_gained: u64,
    pub gold_gained: u64,
    pub hours_offline: u64,
}
