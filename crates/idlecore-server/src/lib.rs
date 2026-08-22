//! IdleBot SpacetimeDB module — server-authoritative game backend.
//!
//! Spec coverage: 001 (idle gains), 003/018/021 (movement, multiplayer state,
//! anti-cheat), 004 (plant/harvest/clean), 005 (voice), 006 (vehicles),
//! 007 (cosmetics), 008 (teleport), 010 (economy ledger), 011/012
//! (marketplace + escrow + disputes), 013/014 (wallet auth), 015 (schedulers),
//! 017 (progression), 019 (schema), 020 (eco).



pub mod types;
pub mod world;
pub mod economy;
pub mod player;
pub mod movement;
pub mod interactions;
pub mod objects;
pub mod teleport;
pub mod vehicles;
pub mod cosmetics;
pub mod market;
pub mod voice;
pub mod idle;
pub mod eco;
pub mod scheduler;

use spacetimedb::{reducer, ReducerContext, ScheduleAt, TimeDuration, Table};
use crate::types::{scheduled_idle_gains, scheduled_plant_growth, scheduled_voice_cleanup, scheduled_market_cleanup, scheduled_eco_maintenance};
use std::time::Duration;

/// Identity → address helper used by every reducre.
fn address_of(ctx: &ReducerContext) -> Option<String> {
    crate::player::address_of_identity(ctx, &ctx.sender().to_string())
}

/// One-time module init: seed the spawn area and register the five recurring
/// schedulers (Spec 015 FR1-FR5).
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    tracing::info!("IdleBot module initializing");

    // The planet is far too large to pre-generate; tiles materialize lazily
    // around players (see world::ensure_tiles_around). Seed the spawn area.
    let (sq, sr) = idlecore_core::earth::resolve_spawn_hex();
    let count = crate::world::ensure_tiles_around(ctx, sq, sr);
    tracing::info!("Spawn area seeded at ({sq},{sr}): {count} hexes");

    if ctx.db.scheduled_idle_gains().iter().next().is_none() {
        ctx.db.scheduled_idle_gains().insert(crate::types::ScheduledIdleGains {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(300))),
            payload: 0,
        });
    }
    if ctx.db.scheduled_plant_growth().iter().next().is_none() {
        ctx.db.scheduled_plant_growth().insert(crate::types::ScheduledPlantGrowth {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(10))),
            payload: 0,
        });
    }
    if ctx.db.scheduled_voice_cleanup().iter().next().is_none() {
        ctx.db.scheduled_voice_cleanup().insert(crate::types::ScheduledVoiceCleanup {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(60))),
            payload: 0,
        });
    }
    if ctx.db.scheduled_market_cleanup().iter().next().is_none() {
        ctx.db.scheduled_market_cleanup().insert(crate::types::ScheduledMarketCleanup {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(3600))),
            payload: 0,
        });
    }
    if ctx.db.scheduled_eco_maintenance().iter().next().is_none() {
        ctx.db.scheduled_eco_maintenance().insert(crate::types::ScheduledEcoMaintenance {
            scheduled_id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_duration(Duration::from_secs(3600))),
            payload: 0,
        });
    }
    tracing::info!("IdleBot schedulers registered");
}

// ---------------------------------------------------------------------------
// Auth & lifecycle (Spec 013/014)
// ---------------------------------------------------------------------------

/// Login: binds the caller's SpacetimeDB identity to a wallet address
/// (Spec 013 FR1). Signature verification is deferred to the chain SDK;
/// the module enforces rate limits (Spec 014 FR2/FR3).
#[reducer]
pub fn login(ctx: &ReducerContext, wallet_address: String) {
    let address = wallet_address.to_lowercase();
    match crate::player::login(ctx, &address, &ctx.sender().to_string()) {
        Ok(_) => {
            tracing::info!("LOGIN: {address} (identity {})", ctx.sender());
            // Materialize the neighborhood of wherever this player lives.
            if let Some(p) = crate::economy::find_player(ctx, &address) {
                let count = crate::world::ensure_tiles_around(ctx, p.hex_q, p.hex_r);
                tracing::info!("LOGIN ensured {count} tiles around ({},{})", p.hex_q, p.hex_r);
            }
        }
        Err(e) => tracing::warn!("LOGIN-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn logout(ctx: &ReducerContext, wallet_address: String) {
    crate::player::logout(ctx, &wallet_address.to_lowercase());
}

#[reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    // The client announces its wallet via `login` after connect.
    tracing::debug!("CLIENT-CONNECTED: {}", ctx.sender());
}

/// Spec 018 FR5: mark the player offline on disconnect.
#[reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    let sender = ctx.sender().to_string();
    if let Some(address) = crate::player::address_of_identity(ctx, &sender) {
        crate::player::logout(ctx, &address);
        tracing::info!("DISCONNECT: {address}");
    }
}

// ---------------------------------------------------------------------------
// Profile (Spec 014 FR4)
// ---------------------------------------------------------------------------

#[reducer]
pub fn update_profile(
    ctx: &ReducerContext,
    display_name: Option<String>,
    avatar: Option<String>,
    bio: Option<String>,
) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::player::update_profile(ctx, &address, display_name, avatar, bio);
}

// ---------------------------------------------------------------------------
// Movement (Spec 003/018/021)
// ---------------------------------------------------------------------------

/// Server-authoritative movement with speed validation, hex locking and
/// occupancy enforcement.
#[reducer]
pub fn move_player(
    ctx: &ReducerContext,
    dir_x: f32,
    dir_y: f32,
    intended_speed: f32,
    dt: f32,
    to_x: f32,
    to_y: f32,
) {
    let Some(address) = address_of(ctx) else { return };
    if let Err(e) =
        crate::movement::move_player(ctx, &address, dir_x, dir_y, intended_speed, dt, to_x, to_y)
    {
        tracing::warn!("MOVE-REJECTED {address}: {e}");
    }
}

/// Heartbeat: keeps the player marked online and refreshes the hex lock.
#[reducer]
pub fn heartbeat(ctx: &ReducerContext) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::movement::heartbeat(ctx, &address);
}

// ---------------------------------------------------------------------------
// Interactions (Spec 004)
// ---------------------------------------------------------------------------

#[reducer]
pub fn plant(ctx: &ReducerContext, hex_id: u64, plant_type: String) -> Result<(), String> {
    let Some(address) = address_of(ctx) else {
        return Err("Not logged in — connect first".to_string());
    };
    let result = crate::interactions::plant_at(ctx, &address, hex_id, &plant_type);
    crate::player::log_outcome(ctx, &address, "plant", result.clone());
    result.into_result()
}

#[reducer]
pub fn harvest(ctx: &ReducerContext, hex_id: u64) -> Result<(), String> {
    let Some(address) = address_of(ctx) else {
        return Err("Not logged in — connect first".to_string());
    };
    let result = crate::interactions::harvest(ctx, &address, hex_id);
    crate::player::log_outcome(ctx, &address, "harvest", result.clone());
    result.into_result()
}

#[reducer]
pub fn clean(ctx: &ReducerContext, hex_id: u64) -> Result<(), String> {
    let Some(address) = address_of(ctx) else {
        return Err("Not logged in — connect first".to_string());
    };
    let result = crate::interactions::clean(ctx, &address, hex_id);
    crate::player::log_outcome(ctx, &address, "clean", result.clone());
    result.into_result()
}

// ---------------------------------------------------------------------------
// World objects: gather grass/trees, plant trees from seeds
// ---------------------------------------------------------------------------

#[reducer]
pub fn gather_object(ctx: &ReducerContext, object_id: u64) -> Result<(), String> {
    let Some(address) = address_of(ctx) else {
        return Err("Not logged in — connect first".to_string());
    };
    crate::objects::gather_object(ctx, &address, object_id)
}

#[reducer]
pub fn plant_tree(ctx: &ReducerContext, hex_id: u64) -> Result<(), String> {
    let Some(address) = address_of(ctx) else {
        return Err("Not logged in — connect first".to_string());
    };
    crate::objects::plant_tree(ctx, &address, hex_id)
}

// ---------------------------------------------------------------------------
// Teleport (Spec 008)
// ---------------------------------------------------------------------------

#[reducer]
pub fn teleport_player(ctx: &ReducerContext, target_q: i32, target_r: i32) {
    let Some(address) = address_of(ctx) else { return };
    match crate::teleport::teleport(ctx, &address, target_q, target_r) {
        Ok(_) => tracing::info!("TELEPORT: {address} -> ({target_q},{target_r})"),
        Err(e) => tracing::warn!("TELEPORT-REJECTED {address}: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Vehicles (Spec 006)
// ---------------------------------------------------------------------------

#[reducer]
pub fn buy_vehicle(ctx: &ReducerContext, vehicle_type: String) {
    let Some(address) = address_of(ctx) else { return };
    match crate::vehicles::buy_vehicle(ctx, &address, &vehicle_type) {
        Ok(msg) => tracing::info!("BUY-VEHICLE: {address} {msg}"),
        Err(e) => tracing::warn!("BUY-VEHICLE-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn equip_vehicle(ctx: &ReducerContext, vehicle_type: String) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::vehicles::equip_vehicle(ctx, &address, &vehicle_type);
}

// ---------------------------------------------------------------------------
// Cosmetics (Spec 007)
// ---------------------------------------------------------------------------

#[reducer]
pub fn buy_cosmetic(ctx: &ReducerContext, category: String, tier: String) {
    let Some(address) = address_of(ctx) else { return };
    match crate::cosmetics::buy_cosmetic(ctx, &address, &category, &tier) {
        Ok(msg) => tracing::info!("BUY-COSMETIC: {address} {msg}"),
        Err(e) => tracing::warn!("BUY-COSMETIC-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn equip_cosmetic(ctx: &ReducerContext, cosmetic_id: u32) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::cosmetics::equip_cosmetic(ctx, &address, cosmetic_id);
}

// ---------------------------------------------------------------------------
// Marketplace (Spec 011)
// ---------------------------------------------------------------------------

#[reducer]
pub fn publish_listing(
    ctx: &ReducerContext,
    title: String,
    description: String,
    github_url: String,
    price_usdt: u64,
    category: String,
) {
    let Some(address) = address_of(ctx) else { return };
    match crate::market::publish(ctx, &address, title, description, github_url, price_usdt, category) {
        Ok(id) => tracing::info!("LISTING-PUBLISHED: {address} #{id}"),
        Err(e) => tracing::warn!("LISTING-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn buy_listing(ctx: &ReducerContext, listing_id: u64) {
    let Some(address) = address_of(ctx) else { return };
    match crate::market::buy(ctx, &address, listing_id) {
        Ok(()) => tracing::info!("LISTING-BOUGHT: {address} #{listing_id}"),
        Err(e) => tracing::warn!("LISTING-BUY-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn renew_listing(ctx: &ReducerContext, listing_id: u64) {
    let Some(address) = address_of(ctx) else { return };
    match crate::market::renew(ctx, &address, listing_id) {
        Ok(()) => tracing::info!("LISTING-RENEWED: {address} #{listing_id}"),
        Err(e) => tracing::warn!("LISTING-RENEW-REJECTED {address}: {e}"),
    }
}

#[reducer]
pub fn release_escrow(ctx: &ReducerContext, listing_id: u64) {
    match crate::market::release_escrow(ctx, listing_id) {
        Ok(()) => tracing::info!("ESCROW-RELEASED #{listing_id}"),
        Err(e) => tracing::warn!("ESCROW-REJECTED #{listing_id}: {e}"),
    }
}

#[reducer]
pub fn dispute_listing(ctx: &ReducerContext, listing_id: u64) {
    let Some(address) = address_of(ctx) else { return };
    match crate::market::dispute(ctx, &address, listing_id) {
        Ok(()) => tracing::info!("DISPUTE: {address} #{listing_id}"),
        Err(e) => tracing::warn!("DISPUTE-REJECTED {address}: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Idle gains (Spec 001)
// ---------------------------------------------------------------------------

/// Claim accumulated offline gains (Spec 001 FR2). The scheduler accrues
/// into the ledger every 5 minutes; claiming transfers to balances.
#[reducer]
pub fn claim_idle_gains(ctx: &ReducerContext) {
    let Some(address) = address_of(ctx) else { return };
    match crate::idle::claim(ctx, &address) {
        Ok((xp, gold)) => tracing::info!("IDLE-CLAIMED: {address} +{xp} XP +{gold}G"),
        Err(e) => tracing::warn!("IDLE-CLAIM-REJECTED {address}: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Voice (Spec 005)
// ---------------------------------------------------------------------------

#[reducer]
pub fn voice_join(ctx: &ReducerContext, hex_id: u64) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::voice::join(ctx, &address, hex_id);
}

#[reducer]
pub fn voice_leave(ctx: &ReducerContext, hex_id: u64) {
    let Some(address) = address_of(ctx) else { return };
    let _ = crate::voice::leave(ctx, &address, hex_id);
}