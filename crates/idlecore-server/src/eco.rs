//! Eco system (Spec 020) — hex rating decay (−1/day), pollution re-spread
//! after 48 h neglect (PROPOSAL 3.5), weekly economy audit log.

use spacetimedb::{ReducerContext, Table};
use crate::types::{now_secs, hex_tile, player, POLLUTION_RESPAWN_SECS};

/// Hourly tick: apply −1/day eco decay to every non-water hex, and re-pollute
/// cleaned hexes neglected for 48 h.
pub fn hourly_eco_tick(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    let day_scale = 1.0 / 24.0; // -1 per day → ~0.042 per hour

    let hex_ids: Vec<u64> = ctx.db.hex_tile().iter().map(|h| h.hex_id).collect();
    let mut repolluted = 0u64;
    let mut decayed = 0u64;

    for id in hex_ids {
        let Some(mut hex) = ctx.db.hex_tile().hex_id().find(id) else {
            continue;
        };
        if hex.terrain == "Water" {
            continue;
        }

        let mut changed = false;

        // -1/day decay, floored at 0 (Spec 020 FR3/NFR3).
        if hex.eco_rating > 0 {
            let delta = ((now.saturating_sub(hex.last_interaction)) as f64 / 86_400.0).floor() as i32;
            if delta > 0 {
                hex.eco_rating = (hex.eco_rating - delta).max(0);
                changed = true;
                decayed += 1;
            }
        }
        let _ = day_scale;

        // Re-pollute cleaned hexes after 48 h with no interaction (PROPOSAL 3.5).
        if !hex.is_polluted
            && hex.cleaned_at.is_some()
            && now.saturating_sub(hex.last_interaction) >= POLLUTION_RESPAWN_SECS
            && hex.eco_rating < 50
        {
            hex.is_polluted = true;
            hex.eco_rating = 10;
            changed = true;
            repolluted += 1;
            tracing::info!("ECO-TICK: hex {id} re-polluted (48h neglect)");
        }

        if let Some(cleaned) = hex.cleaned_at {
            // Only re-pollute once per clean cycle.
            if cleaned + POLLUTION_RESPAWN_SECS <= now && !hex.is_polluted && changed {
                // covered above
                let _ = cleaned;
            }
        }

        if changed {
            ctx.db.hex_tile().hex_id().update(hex);
        }
    }

    tracing::info!("ECO-TICK: {decayed} hexes decayed, {repolluted} re-polluted");
}

/// Weekly economy audit hook (Ecosystem 4.2): log circulation summary.
pub fn weekly_audit(ctx: &ReducerContext) {
    let total_gold: u64 = ctx.db.player().iter().map(|p| p.gold).sum();
    let total_xp: u64 = ctx.db.player().iter().map(|p| p.total_xp).sum();
    tracing::info!(
        "AUDIT: {} players, {total_gold}G in circulation, {total_xp} total XP",
        ctx.db.player().iter().count()
    );
}