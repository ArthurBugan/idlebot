//! Voice channels (Spec 005) — auto-created per hex, active at 2+ players,
//! destroyed after 5 minutes of emptiness (FR1-FR4).

use spacetimedb::{ReducerContext, Table};
use crate::types::{now_secs, voice_channel, VoiceChannel};

/// Spec 005 FR4: empty-channel destruction timeout.
pub const CHANNEL_EMPTY_TIMEOUT_SECS: u64 = 300;

/// Spec 005 FR2/FR3: join the channel for a hex (auto-creating it).
pub fn join(ctx: &ReducerContext, address: &str, hex_id: u64) -> Result<(), String> {
    let now = now_secs(ctx);
    let mut players: Vec<String> = {
        let mut ch = match ctx.db.voice_channel().hex_id().find(hex_id) {
            Some(ch) => ch,
            None => {
                ctx.db.voice_channel().insert(VoiceChannel {
                    hex_id,
                    players: serde_json::to_string(&vec![address.to_lowercase()]).unwrap(),
                    created_at: now,
                    last_activity: now,
                    is_active: false,
                });
                return Ok(());
            }
        };
        let list: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
        if list.iter().any(|p| p == &address.to_lowercase()) {
            return Ok(()); // already in channel
        }
        let mut list = list;
        list.push(address.to_lowercase());

        // Spec 005 FR2: activate at 2 players.
        if list.len() >= 2 && !ch.is_active {
            ch.is_active = true;
            tracing::info!("VOICE: channel {hex_id} activated ({} players)", list.len());
        }
        ch.players = serde_json::to_string(&list).unwrap();
        ch.last_activity = now;
        ctx.db.voice_channel().hex_id().update(ch);
        list
    };
    let _ = &mut players;
    Ok(())
}

/// Spec 005 FR3: leave a channel; the channel persists (empty) until the
/// cleanup scheduler destroys it after 5 minutes.
pub fn leave(ctx: &ReducerContext, address: &str, hex_id: u64) -> Result<(), String> {
    let Some(mut ch) = ctx.db.voice_channel().hex_id().find(hex_id) else {
        return Ok(());
    };
    let mut list: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
    list.retain(|p| p != &address.to_lowercase());
    ch.players = serde_json::to_string(&list).unwrap();
    ch.last_activity = now_secs(ctx);
    if list.is_empty() {
        ch.is_active = false;
    }
    ctx.db.voice_channel().hex_id().update(ch);
    Ok(())
}

/// Schedule cleanup (runs every minute): destroy channels empty ≥ 5 min.
pub fn cleanup(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    let mut removed = 0u64;
    let channel_ids: Vec<u64> = ctx.db.voice_channel().iter().map(|c| c.hex_id).collect();
    for hex_id in channel_ids {
        let Some(ch) = ctx.db.voice_channel().hex_id().find(hex_id) else {
            continue;
        };
        let players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
        if players.is_empty() && now.saturating_sub(ch.last_activity) >= CHANNEL_EMPTY_TIMEOUT_SECS {
            ctx.db.voice_channel().hex_id().delete(ch.hex_id);
            removed += 1;
        }
    }
    if removed > 0 {
        tracing::info!("VOICE-TICK: destroyed {removed} idle channels");
    }
}