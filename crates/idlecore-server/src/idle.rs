//! Idle gains (Spec 001) — server-side calculation, 24 h cap, anti-cheat,
//! decay, and manual claim (FR1-FR5). Timestamps come from the server clock
//! only; client input is never trusted.

use spacetimedb::{ReducerContext, Table};
use crate::economy::{add_gold, add_xp};
use crate::types::{idle_gain, now_secs, player, IdleGain};
use idlecore_core::idle_config::{gains_for_time, MAX_IDLE_SECONDS};
use std::time::Duration;

/// Recompute and bank a player's pending offline gains.
pub fn accrue(ctx: &ReducerContext, address: &str) {
    let now = now_secs(ctx);
    let Some(mut p) = crate::economy::find_player(ctx, &address.to_lowercase()) else {
        return;
    };

    // Track pending gains in the idle_gain row.
    let mut gain = match ctx.db.idle_gain().player().find(p.address.clone()) {
        Some(g) => g,
        None => IdleGain {
            player: p.address.clone(),
            pending_xp: 0,
            pending_gold: 0,
            last_calculated_at: now,
            claimed_at: None,
        },
    };

    // Server-clock elapsed (Spec 001 NFR1) — never client timestamps.
    let elapsed = now.saturating_sub(p.last_seen).min(MAX_IDLE_SECONDS);
    let raw = gains_for_time(Duration::from_secs(elapsed));

    // Anti-cheat (PROPOSAL 2.2): rapid-login ban and idle decay multiplier.
    let mut gained_xp = raw.xp;
    let mut gained_gold = raw.gold;
    if p.idle_gains_blocked(now) {
        tracing::warn!("IDLE-BAN: {} skipped gains (rapid login)", address);
        gained_xp = 0;
        gained_gold = 0;
    } else {
        let mult = p.idle_decay_multiplier(now) as f64;
        if mult < 1.0 {
            gained_xp = ((gained_xp as f64) * mult) as u64;
            gained_gold = ((gained_gold as f64) * mult) as u64;
        }
    }

    gain.pending_xp = gain.pending_xp.saturating_add(gained_xp);
    gain.pending_gold = gain.pending_gold.saturating_add(gained_gold);
    gain.last_calculated_at = now;
    gain.claimed_at = None;
    match ctx.db.idle_gain().player().find(gain.player.clone()) {
        Some(existing) => {
            ctx.db.idle_gain().player().update(gain.clone());
            let _ = existing;
        }
        None => {
            ctx.db.idle_gain().insert(gain.clone());
        }
    }

    if gained_xp > 0 || gained_gold > 0 {
        tracing::info!(
            "IDLE-GAIN: {} +{}XP +{}G (elapsed {}s)",
            address,
            gained_xp,
            gained_gold,
            elapsed
        );
    }
    let _ = &mut p;
}

/// Spec 001 FR4: manual claim applies the pending gains to the player.
/// Returns Ok((xp, gold)) claimed.
pub fn claim(ctx: &ReducerContext, address: &str) -> Result<(u64, u64), String> {
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    let mut gain = ctx
        .db
        .idle_gain()
        .player()
        .find(p.address.clone())
        .ok_or_else(|| "No pending idle gains".to_string())?;

    let xp = gain.pending_xp;
    let gold = gain.pending_gold;
    if xp == 0 && gold == 0 {
        return Ok((0, 0));
    }

    gain.pending_xp = 0;
    gain.pending_gold = 0;
    gain.claimed_at = Some(now_secs(ctx));
    ctx.db.idle_gain().player().update(gain);

    add_gold(ctx, &mut p, gold, "idle_gain");
    add_xp(ctx, &mut p, xp, "idle_gain");
    ctx.db.player().address().update(p);
    Ok((xp, gold))
}

/// Spec 001 FR3: expose the pending-gains state (for client display).
pub fn pending(ctx: &ReducerContext, address: &str) -> Option<(u64, u64)> {
    let p = crate::economy::find_player(ctx, &address.to_lowercase())?;
    ctx.db
        .idle_gain()
        .player()
        .find(p.address)
        .map(|g| (g.pending_xp, g.pending_gold))
}