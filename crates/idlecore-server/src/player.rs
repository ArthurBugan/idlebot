//! Player lifecycle: login (with identity binding + idle anti-cheat, Spec 013/
//! 014), logout, profile updates, hex-occupancy helpers.

use spacetimedb::{ReducerContext, Table};
use crate::types::{
    hex_center, hex_id_of, now_secs, player, Player, RAPID_LOGIN_BAN_SECS, RAPID_LOGIN_WINDOW_SECS,
    STARTING_GOLD,
};

/// Login: create the account on first login, otherwise restore it. Binds the
/// SpacetimeDB connection identity to the wallet address (server-authoritative
/// ownership — the address is only ever stored once, on first login).
/// Outcome of a login, separated from the DB write so it is testable.
#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub created: bool,
    pub player: Player,
    pub rapid: bool,
}

/// Pure part of login: create the account on first login, otherwise restore
/// it. Binds the connection identity to the wallet address (server-
/// authoritative ownership — the address is only ever stored once, on first
/// login) and applies the idle-gain anti-cheat rapid-login ban.
pub fn resolve_login(
    existing: Option<&Player>,
    address: &str,
    identity: &str,
    now: u64,
) -> Result<LoginOutcome, String> {
    let lower = address.to_lowercase();
    let Some(existing) = existing else {
        return Ok(LoginOutcome {
            created: true,
            player: new_player(&lower, identity, now),
            rapid: false,
        });
    };
    if !existing.identity.is_empty() && existing.identity != identity {
        return Err("Address already claimed by another identity".to_string());
    }
    let mut player = existing.clone();
    player.identity = identity.to_string();

    // Never let anyone log in standing on open water (e.g. legacy saves from
    // before the Earth replica, or coordinates outside the map): snap to the
    // resolved land spawn.
    if !idlecore_core::earth::is_land_at(player.position_x, player.position_y) {
        let (sq, sr) = idlecore_core::earth::resolve_spawn_hex();
        let (sx, sy) = hex_center(sq, sr);
        tracing::info!(
            "RELOCATE {lower}: was at sea ({:.0},{:.0}) -> spawn ({sq},{sr})",
            player.position_x,
            player.position_y
        );
        player.hex_q = sq;
        player.hex_r = sr;
        player.hex_id = hex_id_of(sq, sr);
        player.position_x = sx;
        player.position_y = sy;
    }

    // Idle-gain anti-cheat: rapid re-login within 5 min of the previous one
    // triggers a 90-day "new player" state (PROPOSAL 2.2).
    let rapid = player.last_login > 0
        && now.saturating_sub(player.last_login) < RAPID_LOGIN_WINDOW_SECS;
    player.rapid_login_count = if rapid {
        player.rapid_login_count.saturating_add(1)
    } else {
        0
    };
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
    Ok(LoginOutcome {
        created: false,
        player,
        rapid,
    })
}

/// Login: applies `resolve_login`, then persists the row (Spec 013/014).
pub fn login(
    ctx: &ReducerContext,
    address: &str,
    sender_identity: &str,
) -> Result<(bool, bool), String> {
    let now = now_secs(ctx);
    let lower = address.to_lowercase();
    let existing = ctx.db.player().address().find(lower.clone());
    let outcome = resolve_login(existing.as_ref(), &lower, sender_identity, now)?;
    if outcome.created {
        ctx.db.player().insert(outcome.player.clone());
    } else {
        ctx.db.player().address().update(outcome.player.clone());
    }
    Ok((outcome.created, outcome.rapid))
}

/// Create a brand-new player (Spec 014): starting gold 100, level 1, spawned
/// on solid land (resolved Earth-replica spawn, never mid-ocean).
fn new_player(address: &str, identity: &str, now: u64) -> Player {
    let (sq, sr) = idlecore_core::earth::resolve_spawn_hex();
    let (sx, sy) = hex_center(sq, sr);
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
        position_x: sx,
        position_y: sy,
        hex_q: sq,
        hex_r: sr,
        hex_id: hex_id_of(sq, sr),
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
    // Lazy world materialization: tiles come into existence as players
    // approach them (the planet is too large to pre-generate).
    crate::world::ensure_tiles_around(ctx, q, r);
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

#[cfg(test)]
mod login_tests {
    use super::*;

    fn sample(address: &str, identity: &str, last_login: u64, count: u32) -> Player {
        let mut p = new_player(address, identity, 1_000_000);
        p.last_login = last_login;
        p.rapid_login_count = count;
        p
    }

    #[test]
    fn creation_from_wallet_address_works() {
        // Spec 014 T5.1: first login creates the row from the wallet address.
        let outcome = resolve_login(None, "0xAbC123", "id-1", 1_000_000).unwrap();
        assert!(outcome.created);
        assert!(!outcome.rapid);
        let p = &outcome.player;
        assert_eq!(p.address, "0xabc123");
        assert_eq!(p.identity, "id-1");
        assert_eq!(p.level, 1);
        assert_eq!(p.gold, STARTING_GOLD);
        assert_eq!(p.status, "online");
        assert_eq!(p.avatar, "Tetrahedron");
    }

    #[test]
    fn claimed_address_rejected_for_other_identity() {
        let existing = sample("0xa", "id-1", 0, 0);
        let err = resolve_login(Some(&existing), "0xA", "id-2", 1_000_000).unwrap_err();
        assert!(err.contains("claimed"));
    }

    #[test]
    fn empty_identity_claim_binds() {
        let existing = sample("0xa", "", 0, 0);
        let outcome = resolve_login(Some(&existing), "0xa", "id-9", 1_000_000).unwrap();
        assert!(!outcome.created);
        assert_eq!(outcome.player.identity, "id-9");
    }

    #[test]
    fn regular_relogin_resets_rapid_count() {
        let existing = sample("0xa", "id-1", 1_000_000 - 400, 3);
        let outcome = resolve_login(Some(&existing), "0xa", "id-1", 1_000_000).unwrap();
        assert!(!outcome.rapid);
        assert_eq!(outcome.player.rapid_login_count, 0);
    }
}
