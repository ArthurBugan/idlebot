//! Economy primitives — the only place gold/XP/eco balances change.
//!
//! Spec 010: server-authoritative, no negative balances, full ledger.

use spacetimedb::{ReducerContext, Table};
use crate::types::{
    now_secs, player, transaction, Player, Transaction,
};

/// Spend gold. Returns Err(reason) if the balance is insufficient.
pub fn spend_gold(
    ctx: &ReducerContext,
    p: &mut Player,
    amount: u64,
    action: &str,
) -> Result<(), String> {
    if p.gold < amount {
        return Err(format!("Insufficient gold (need {amount}, have {})", p.gold));
    }
    p.gold -= amount;
    p.lifetime_gold_spent = p.lifetime_gold_spent.saturating_add(amount);
    p.last_spend = now_secs(ctx);
    record(ctx, p, action, -(amount as i64), 0, 0);
    ctx.db.player().address().update(p.clone());
    Ok(())
}

/// Add gold.
pub fn add_gold(ctx: &ReducerContext, p: &mut Player, amount: u64, action: &str) {
    p.gold = p.gold.saturating_add(amount);
    p.lifetime_gold_earned = p.lifetime_gold_earned.saturating_add(amount);
    record(ctx, p, action, amount as i64, 0, 0);
    ctx.db.player().address().update(p.clone());
}

/// Add XP and advance the level (Spec 017). Returns true on level-up.
pub fn add_xp(ctx: &ReducerContext, p: &mut Player, amount: u64, action: &str) -> bool {
    p.total_xp = p.total_xp.saturating_add(amount);
    let new_level = idlecore_core::progression::calculate_level(p.total_xp);
    let leveled = new_level > p.level;
    if leveled {
        p.level = new_level;
        record(ctx, p, "level_up", 0, 0, 0);
        tracing::info!(
            "LEVEL_UP player={} new_level={} total_xp={}",
            p.address,
            new_level,
            p.total_xp
        );
    }
    record(ctx, p, action, 0, amount as i64, 0);
    ctx.db.player().address().update(p.clone());
    leveled
}

/// Add eco points (coerced to i32 range; never negative).
pub fn add_eco_points(ctx: &ReducerContext, p: &mut Player, amount: i64, action: &str) {
    let before = p.eco_points as i64;
    let after = (before + amount).clamp(0, i32::MAX as i64) as u32;
    p.eco_points = after;
    record(ctx, p, action, 0, 0, amount);
    ctx.db.player().address().update(p.clone());
}

pub fn spend_eco_points(ctx: &ReducerContext, p: &mut Player, amount: u32) -> Result<(), String> {
    if p.eco_points < amount {
        return Err("Insufficient eco points".to_string());
    }
    p.eco_points -= amount;
    record(ctx, p, "spend_eco", 0, 0, -(amount as i64));
    ctx.db.player().address().update(p.clone());
    Ok(())
}

/// Usdt helpers (6-decimal virtual balance; chain-backed in production).
pub fn spend_usdt(ctx: &ReducerContext, p: &mut Player, amount: u64, action: &str) -> Result<(), String> {
    if p.usdt < amount {
        return Err(format!("Insufficient USDT (need {amount}, have {})", p.usdt));
    }
    p.usdt -= amount;
    record(ctx, p, action, 0, 0, 0);
    ctx.db.player().address().update(p.clone());
    Ok(())
}

pub fn add_usdt(ctx: &ReducerContext, p: &mut Player, amount: u64, action: &str) {
    p.usdt = p.usdt.saturating_add(amount);
    record(ctx, p, action, 0, 0, 0);
    ctx.db.player().address().update(p.clone());
}

/// Append a ledger row (Spec 010 FR7). The balance reported is gold.
fn record(ctx: &ReducerContext, p: &Player, action: &str, gold: i64, xp: i64, eco: i64) {
    ctx.db.transaction().insert(Transaction {
        tx_id: 0, // auto-inc
        player: p.address.clone(),
        timestamp: now_secs(ctx),
        action: action.to_string(),
        gold_change: gold,
        xp_change: xp,
        eco_points_change: eco,
        balance_after: p.gold,
    });
}

/// Find a player row by address.
pub fn find_player(ctx: &ReducerContext, address: &str) -> Option<Player> {
    ctx.db.player().address().find(address.to_string())
}