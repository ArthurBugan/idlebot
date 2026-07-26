//! Sistema de voice chat - canais por hexágono

use super::types::*;
use spacetimedb::{ReducerContext, Table};
use std::time::{SystemTime, UNIX_EPOCH};

/// Join channel de voz
pub fn join_channel(ctx: &ReducerContext, wallet_address: &str, hex_id: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Buscar ou criar channel
    let channel = ctx.db.voice_channel().iter()
        .find(|ch| ch.hex_id == hex_id);

    match channel {
        Some(mut ch) => {
            // Adicionar player ao channel
            let mut players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
            if !players.contains(&wallet_address.to_string()) {
                players.push(wallet_address.to_string());
                ch.players = serde_json::to_string(&players).unwrap();
                ch.last_activity = now;
                ctx.db.voice_channel().hex_id().update(ch);
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
            ctx.db.voice_channel().insert(channel);
        }
    }
}

/// Leave channel de voz
pub fn leave_channel(ctx: &ReducerContext, wallet_address: &str, hex_id: u64) {
    let ch = ctx.db.voice_channel().iter()
        .find(|ch| ch.hex_id == hex_id);

    if let Some(mut ch) = ch {
        let mut players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
        players.retain(|p| p != wallet_address);

        if players.is_empty() {
            // Remover channel se vazio
            let channel_to_delete = ch;
            ctx.db.voice_channel().delete(channel_to_delete);
        } else {
            ch.players = serde_json::to_string(&players).unwrap();
            ch.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            ctx.db.voice_channel().hex_id().update(ch);
        }
    }
}

/// Cleanup channels inativos (maior que 5 minutos sem atividade)
pub fn cleanup_inactive_channels(ctx: &ReducerContext) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let timeout = 300; // 5 minutos

    // Buscar channels inativos
    let channels: Vec<VoiceChannelDbEntry> = ctx.db.voice_channel().iter().collect();

    for ch in channels {
        let time_diff = now - ch.last_activity;
        if time_diff > timeout {
            let hex_id = ch.hex_id;
            // Remover channel inativo
            ctx.db.voice_channel().delete(ch);
            tracing::debug!("Removed inactive voice channel: {}", hex_id);
        }
    }
}
