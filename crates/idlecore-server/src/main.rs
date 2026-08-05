//! IdleBot Server Module
//!
//! Entry points for the SpacetimeDB IdleBot module

#![allow(dead_code)]

pub mod types;
pub mod world;
pub mod market;
pub mod vehicle;
pub mod voice;
pub mod scheduler;

use spacetimedb::{reducer, ReducerContext};

/// When the player changes hex (view)
#[reducer]
pub fn hex_changed(_ctx: &ReducerContext) {}

/// When the player loses idle time (view)
#[reducer]
pub fn idle_gained(_ctx: &ReducerContext) {}

/// When item is purchased
#[reducer]
pub fn item_purchased(_ctx: &ReducerContext) {}

/// When listing is created
#[reducer]
pub fn listing_created(_ctx: &ReducerContext) {}

/// When listing is sold
#[reducer]
pub fn listing_sold(_ctx: &ReducerContext) {}

/// When player joins voice channel
#[reducer]
pub fn voice_join(_ctx: &ReducerContext) {}

/// When player leaves voice channel
#[reducer]
pub fn voice_leave(_ctx: &ReducerContext) {}

/// Init -- runs once on deploy
#[reducer(init)]
pub fn init(_ctx: &ReducerContext) {
    tracing::info!("IdleBot module initialized");
}

/// Login / Register
#[reducer]
pub fn login(
    ctx: &ReducerContext,
    wallet_address: String,
    signature: String,
    nonce: u64,
) {
    // Mark player as online and reset idle tracking
    crate::world::handle_login(ctx, &wallet_address, &signature, nonce);
    crate::world::mark_online(ctx, &wallet_address);
    crate::world::reset_idle_tracking(ctx, &wallet_address);
    tracing::info!("Player logged in: {}", wallet_address);
}

/// Logout / Disconnect
#[reducer]
pub fn logout(ctx: &ReducerContext, wallet_address: String) {
    crate::world::mark_offline(ctx, &wallet_address);
    crate::world::reset_idle_tracking(ctx, &wallet_address);
    tracing::info!("Player logged out: {}", wallet_address);
}

/// Move player
#[reducer]
pub fn move_player(
    ctx: &ReducerContext,
    wallet_address: String,
    target_x: f32,
    target_y: f32,
) {
    crate::world::move_player(ctx, &wallet_address, target_x, target_y);
}

/// Teleport player
#[reducer]
pub fn teleport_player(
    ctx: &ReducerContext,
    wallet_address: String,
    target_hex_id: u64,
) {
    let cost = 100u64;
    crate::world::teleport_player(ctx, &wallet_address, target_hex_id, cost);
}

/// Interact with hex (plant, harvest, clean)
#[reducer]
pub fn interact_hex(
    ctx: &ReducerContext,
    wallet_address: String,
    hex_id: u64,
    action: String,
    plant_type: Option<String>,
) {
    let result = crate::world::interact_hex(ctx, &wallet_address, hex_id, &action, plant_type);
    match result {
        Ok(crate::world::ActionResult::Success {
            xp_gained,
            gold_gained,
            ..
        }) => {
            if xp_gained > 0 || gold_gained > 0 {
                // Trigger idle_gained event for notification
            }
        }
        Ok(crate::world::ActionResult::Failed { reason }) => {
            tracing::warn!("Action failed: {}", reason);
        }
        Err(e) => {
            tracing::warn!("Action failed: {}", e);
        }
    }
}

/// Buy item (vehicle or cosmetic)
#[reducer]
pub fn buy_item(
    ctx: &ReducerContext,
    wallet_address: String,
    item_type: String,
    item_name: String,
    cost: u64,
) {
    crate::world::buy_item(ctx, &wallet_address, &item_type, &item_name, cost);
}

/// Publish template on market
#[reducer]
pub fn publish_template(
    ctx: &ReducerContext,
    wallet_address: String,
    title: String,
    github_url: String,
    description: String,
    price_usdt: f64,
) {
    crate::market::publish_template(
        ctx,
        &wallet_address,
        title,
        github_url,
        description,
        price_usdt,
    );
}

/// Complete template purchase (called via blockchain event)
#[reducer]
pub fn complete_template_purchase(
    ctx: &ReducerContext,
    seller: String,
    buyer: String,
    listing_id: u64,
    price_usdt: f64,
) {
    crate::market::complete_purchase(ctx, &seller, &buyer, listing_id, price_usdt);
}

/// Join voice channel
#[reducer]
pub fn voice_join_hex(ctx: &ReducerContext, wallet_address: String, hex_id: u64) {
    crate::voice::join_channel(ctx, &wallet_address, hex_id);
}

/// Leave voice channel
#[reducer]
pub fn voice_leave_hex(ctx: &ReducerContext, wallet_address: String, hex_id: u64) {
    crate::voice::leave_channel(ctx, &wallet_address, hex_id);
}

/// Update plant growth (called periodically via scheduler)
#[reducer]
pub fn update_plants(ctx: &ReducerContext) {
    crate::world::update_plants(ctx);
}

/// Calculate idle gains (called periodically via scheduler -- 5 min interval)
#[reducer]
pub fn calculate_idle(_ctx: &ReducerContext) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    crate::scheduler::process_idle_gains(now);
}

/// Cleanup inactive voice channels
#[reducer]
pub fn cleanup_voice_channels(ctx: &ReducerContext) {
    crate::voice::cleanup_inactive_channels(ctx);
}

/// Cleanup old unsold listings
#[reducer]
pub fn cleanup_old_listings(ctx: &ReducerContext) {
    crate::market::cleanup_old_listings(ctx);
}

/// Empty main -- SpacetimeDB modules are compiled to WASM and don't use this.
fn main() {}
