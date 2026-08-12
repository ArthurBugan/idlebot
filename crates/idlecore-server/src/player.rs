//! Player lifecycle: login (with identity binding + idle anti-cheat, Spec 013/
//! 014), logout, profile updates, hex-occupancy helpers.

use spacetimedb::{ReducerContext, Table};
use crate::types::{
    hex_id_of, now_secs, player, Player, RAPID_LOGIN_BAN_SECS, RAPID_LOGIN_WINDOW_SECS,
    STARTING_GOLD,
};

/// Login: create the account on first login, otherwise restore it. Binds the
/// SpacetimeDB connection identity to the wallet address (server-authoritative
/// ownership — the address is only ever stored once, on first login).
pub fn login(
    ctx: &ReducerContext,
    address: &str,
    sender_identity: &str,
) -> Result<(bool, bool), String> {
    let now = now_secs(ctx);
    let lower = address.to_lowercase();

    let mut player = match ctx.db.player().address().find(lower.clone()) {
        Some(p) => p,
        None => {
            let p = new_player(&lower, sender_identity, now);
            ctx.db.player().insert(p.clone());
            return Ok((true, false)); // created = true
        }
    };

    if !player.identity.is_empty() && player.identity != sender_identity {
        return Err("Address already claimed by another identity".to_string());
    }
    player.identity = sender_identity.to_string();

    // Idle-gain anti-cheat: rapid re-login within 5 min of the previous one
    // triggers a 90-day "new player" state (PROPOSAL 2.2).
    let rapid = player.last_login > 0 && now.saturating_sub(player.last_login) < RAPID_LOGIN_WINDOW_SECS;
    player.rapid_login_count = if rapid { player.rapid_login_count.saturating_add(1) } else { 0 };
    if rapid && player.rapid_login_count >= 2 {
        player.idle_gains_blocked_until = now.saturating_add(RAPID_LOGIN_BAN_SECS);
        tracing::warn!(
            "ANTI_CHEAT rapid logins x{} address={} idle gains blocked 90d",
            player.rapid_login_count,
            lower
        );
        player.rapid_login_count = 0;
    }

    player.status = "online".to_string();
    player.last_login = now;
    player.last_seen = now;

    ctx.db.player().address().update(player.clone());
    Ok((false, rapid))
}

/// Create a brand-new player (Spec 014): starting gold 100, level 1.
fn new_player(address: &str, identity: &str, now: u64) -> Player {
    Player {
        address: address.to_string(),
        identity: identity.to_string(),
        status: "online".to_string(),
        display_name: None,
        avatar: "Tetrahedron".to_string(),
        bio: None,
        level: 1,
        total_xp: 0,
        gold: STARTING_GOLD,
        usdt: 0,
        eco_points: 0,
        lifetime_gold_earned: 0,
        lifetime_gold_spent: 0,
        position_x: 0.0,
        position_y: 0.0,
        hex_q: 0,
        hex_r: 0,
        hex_id: 0,
        vehicle: "None".to_string(),
        cosmetics: "[]".to_string(),
        templates: "[]".to_string(),
        last_login: now,
        last_seen: now,
        last_action_at: 0,
        last_spend: now,
        created_at: now,
        rapid_login_count: 0,
        idle_gains_blocked_until: 0,
        total_play_time: 0,
        plants_planted: 0,
        plants_harvested: 0,
        pollution_cleaned: 0,
        templates_published: 0,
        templates_purchased: 0,
    }
}

impl Player {
    /// True while the player is excluded from idle gains (rapid-login ban).
    pub fn idle_gains_blocked(&self, now: u64) -> bool {
        now < self.idle_gains_blocked_until
    }

    /// Idle-gain decay multiplier (Ecosystem spec 2.2): after 7 days without
    /// any gold spending the multiplier drops 10%/day, 25%/day past 15, 50%
    /// past 30.
    pub fn idle_decay_multiplier(&self, now: u64) -> f32 {
        let days = now.saturating_sub(self.last_spend) / 86_400;
        if days > 30 {
            0.5
        } else if days > 15 {
            0.75
        } else if days > 7 {
            0.9
        } else {
            1.0
        }
    }
}

/// Logout / disconnect: mark offline (idle gains accrue from last_seen).
pub fn logout(ctx: &ReducerContext, address: &str) {
    let Some(mut p) = ctx.db.player().address().find(address.to_lowercase()) else {
        return;
    };
    let now = now_secs(ctx);
    p.status = "offline".to_string();
    p.total_play_time = p.total_play_time.saturating_add(now.saturating_sub(p.last_seen));
    p.last_seen = now;
    ctx.db.player().address().update(p);
    tracing::info!("Player logged out: {address}");
}

/// Spec 014 FR3/FR4: display name (≤20 alphanumerics) and avatar.
pub fn update_profile(
    ctx: &ReducerContext,
    address: &str,
    display_name: Option<String>,
    avatar: Option<String>,
    bio: Option<String>,
) -> Result<(), String> {
    let mut p = ctx
        .db
        .player()
        .address()
        .find(address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    if let Some(name) = display_name {
        if name.chars().count() > 20 {
            return Err("Display name too long (max 20 chars)".to_string());
        }
        if !name.chars().all(|c| c.is_alphanumeric()) {
            return Err("Display name must be alphanumeric".to_string());
        }
        if !name.is_empty() {
            p.display_name = Some(name);
        }
    }
    if let Some(avatar) = avatar {
        p.avatar = avatar;
    }
    if let Some(bio) = bio {
        if bio.chars().count() <= 200 {
            p.bio = Some(bio);
        }
    }
    ctx.db.player().address().update(p);
    Ok(())
}

/// Move the player's hex bookmark (position is validated in movement.rs).
pub fn update_hex(ctx: &ReducerContext, address: &str, q: i32, r: i32, x: f32, y: f32, now: u64) {
    let Some(mut p) = ctx.db.player().address().find(address.to_lowercase()) else {
        return;
    };
    p.hex_q = q;
    p.hex_r = r;
    p.hex_id = hex_id_of(q, r);
    p.position_x = x;
    p.position_y = y;
    p.last_seen = now;
    ctx.db.player().address().update(p);
}

/// Count online players currently in a hex (Spec 018 FR3 occupancy).
pub fn players_in_hex(ctx: &ReducerContext, hex_id: u64) -> usize {
    ctx.db
        .player()
        .iter()
        .filter(|p| p.hex_id == hex_id && p.status == "online")
        .count()
}
/// Resolve the wallet address bound to a SpacetimeDB identity.
pub fn address_of_identity(ctx: &ReducerContext, identity: &str) -> Option<String> {
    ctx.db
        .player()
        .iter()
        .find(|p| p.identity == identity)
        .map(|p| p.address.clone())
}

/// Log an interaction outcome to the server log.
pub fn log_outcome(_ctx: &ReducerContext, address: &str, action: &str, outcome: crate::interactions::Outcome) {
    match outcome {
        crate::interactions::Outcome::Ok(msg) => {
            tracing::info!("ACT-OK {address} {action}: {msg}");
        }
        crate::interactions::Outcome::Err(msg) => {
            tracing::warn!("ACT-REJECTED {address} {action}: {msg}");
        }
    }
}
