//! Sistema de voice chat - canais por hexágono

use super::types::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Join channel de voz
pub fn join_channel(wallet_address: &str, hex_id: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Buscar ou criar channel
    let channel: Option<VoiceChannelDbEntry> = db::voice_channel::table()
        .filter(|ch: &VoiceChannelDbEntry| ch.hex_id == hex_id)
        .first();
    
    match channel {
        Some(mut ch) => {
            // Adicionar player ao channel
            let mut players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
            if !players.contains(&wallet_address.to_string()) {
                players.push(wallet_address.to_string());
                ch.players = serde_json::to_string(&players).unwrap();
                ch.last_activity = now;
                db::voice_channel::table().update(ch);
            }
        }
        None => {
            // Criar novo channel
            let players = vec![wallet_address.to_string()];
            let channel = VoiceChannelDbEntry {
                hex_id,
                players: serde_json::to_string(&players).unwrap(),
                created_at: now,
                last_activity: now,
            };
            db::voice_channel::table().insert(channel);
        }
    }
}

/// Leave channel de voz
pub fn leave_channel(wallet_address: &str, hex_id: u64) {
    let ch: Option<VoiceChannelDbEntry> = db::voice_channel::table()
        .filter(|ch: &VoiceChannelDbEntry| ch.hex_id == hex_id)
        .first();
    
    if let Some(mut ch) = ch {
        let mut players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
        players.retain(|p| p != wallet_address);
        
        if players.is_empty() {
            // Remover channel se vazio
            db::voice_channel::table().delete(hex_id);
        } else {
            ch.players = serde_json::to_string(&players).unwrap();
            ch.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            db::voice_channel::table().update(ch);
        }
    }
}

/// Cleanup channels inativos (maior que 5 minutos sem atividade)
pub fn cleanup_inactive_channels() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let timeout = 300; // 5 minutos
    
    // Buscar channels inativos
    let channels: Vec<VoiceChannelDbEntry> = db::voice_channel::table().collect();
    
    for ch in channels {
        let time_diff = now - ch.last_activity;
        if time_diff > timeout {
            // Remover channel inativo
            db::voice_channel::table().delete(ch.hex_id);
            tracing::debug!("Removed inactive voice channel: {}", ch.hex_id);
        }
    }
}
