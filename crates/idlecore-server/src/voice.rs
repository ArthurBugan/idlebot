/// Join channel de voz
pub fn join_channel(ctx: &ReducerContext, wallet_address: &str, hex_id: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Buscar ou criar channel
    let channel_entry = ctx.db.voice_channel().iter()
        .find(|ch| ch.hex_id == hex_id);

    match channel_entry {
        Some(mut ch) => {
            // Player already present or trying to rejoin. Update activity.
            let players: Vec<String> = serde_json::from_str(&ch.players).unwrap_or_default();
            if !players.contains(&wallet_address.to_string()) {
                // New player joining existing channel
                players.push(wallet_address.to_string());
                ch.players = serde_json::to_string(&players).unwrap();
                ch.last_activity = now;

                // Transition to active state if the second player joins
                if players.len() >= 2 && !ch.is_active {
                    ch.is_active = true;
                }
                ctx.db.voice_channel().hex_id().update(ch);
            }
        }
        None => {
            // First player joins: create PENDING channel.
            let players = vec![wallet_address.to_string()];
            let channel = VoiceChannelDbEntry {
                hex_id,
                players: serde_json::to_string(&players).unwrap(),
                created_at: now,
                last_activity: now,
                is_active: false, // <-- NEW: Starts inactive/pending
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
            // If the last player leaves, destroy channel.
            ctx.db.voice_channel().delete(ch);
        } else {
            // Players remain, update activity. is_active state persists.
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
            // Deleting based purely on timeout, consistent with original code structure.
            ctx.db.voice_channel().delete(ch);
            tracing::debug!("Removed inactive voice channel: {}", ch.hex_id);
        }
    }
}