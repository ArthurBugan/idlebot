//! Scheduled functions (Spec 015) — recurring reducers driven by SpacetimeDB
//! schedule tables. The `#[reducer]` wrappers live in `types.rs` next to the
//! tables (the macro requires them in scope); the bodies are here and run
//! server-side exclusively.

use spacetimedb::{ReducerContext, ScheduleAt, Table, TimeDuration};
use crate::types::{player, hex_tile, scheduled_idle_gains, scheduled_plant_growth, scheduled_voice_cleanup, scheduled_market_cleanup, scheduled_eco_maintenance, 
    now_secs, scheduled_log, ScheduledLog,
};
use std::time::Duration;

/// Audit helper (Spec 015 FR6): every scheduled run appends a row.
pub fn audit(ctx: &ReducerContext, function: &str, detail: impl AsRef<str>) {
    ctx.db.scheduled_log().insert(ScheduledLog {
        id: 0,
        function_name: function.to_string(),
        timestamp: now_secs(ctx),
        detail: detail.as_ref().to_string(),
    });
}

fn count_players(ctx: &ReducerContext) -> usize {
    ctx.db.player().iter().count()
}

/// Register the five recurring functions. Called once from `init`.
pub fn register_all(ctx: &ReducerContext) {
    ctx.db.scheduled_idle_gains().insert(crate::types::ScheduledIdleGains {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(300))),
        payload: 0,
    });
    ctx.db.scheduled_plant_growth().insert(crate::types::ScheduledPlantGrowth {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(10))),
        payload: 0,
    });
    ctx.db.scheduled_voice_cleanup().insert(crate::types::ScheduledVoiceCleanup {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(60))),
        payload: 0,
    });
    ctx.db.scheduled_market_cleanup().insert(crate::types::ScheduledMarketCleanup {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(3600))),
        payload: 0,
    });
    ctx.db.scheduled_eco_maintenance().insert(crate::types::ScheduledEcoMaintenance {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(3600))),
        payload: 0,
    });
    tracing::info!("Scheduled functions registered: idle(5m), plant(10s), voice(1m), market(1h), eco(1h)");
}

/// Spec 001 NFR2: every 5 minutes, accrue idle gains banked while offline.
pub fn idle_gains_tick_body(ctx: &ReducerContext) {
    let players: Vec<String> = ctx
        .db
        .player()
        .iter()
        .filter(|p| p.status == "offline")
        .map(|p| p.address.clone())
        .collect();
    for address in players {
        crate::idle::accrue(ctx, &address);
    }
    audit(ctx, "idle_gains", format!("{} players checked", count_players(ctx)));
}

/// Spec 015 FR2: every 10 s, sweep plants — maturity is computed on demand,
/// the tick keeps the audit trail warm and flags mature crops.
pub fn plant_growth_tick_body(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    let mature: usize = ctx
        .db
        .hex_tile()
        .iter()
        .filter(|h| {
            h.plant
                .as_deref()
                .and_then(crate::types::Plant::from_json)
                .map(|p| p.is_mature(now))
                .unwrap_or(false)
        })
        .count();
    audit(ctx, "plant_growth", format!("{mature} mature crops ready"));
}

/// Spec 015 FR3: every minute, destroy empty voice channels ≥ 5 min idle.
pub fn voice_cleanup_tick_body(ctx: &ReducerContext) {
    crate::voice::cleanup(ctx);
    audit(ctx, "voice_cleanup", "voice channels swept");
}

/// Spec 015 FR4: hourly, expire listings (30 d + 24 h grace) and release
/// matured escrows.
pub fn market_cleanup_tick_body(ctx: &ReducerContext) {
    crate::market::cleanup(ctx);
    audit(ctx, "market_cleanup", "listings swept");
}

/// Hourly: eco decay + pollution re-spread + daily vehicle maintenance.
pub fn eco_maintenance_tick_body(ctx: &ReducerContext) {
    crate::eco::hourly_eco_tick(ctx);
    let epoch_day = (now_secs(ctx) / 86_400) as u32;
    crate::vehicles::charge_daily_maintenance(ctx, epoch_day);
    audit(ctx, "eco_maintenance", "eco decay + maintenance applied");
}