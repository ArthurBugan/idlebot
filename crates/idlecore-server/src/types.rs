//! IdleBot SpacetimeDB schema — all tables per Spec 019.
//!
//! Nine core tables + five internal scheduler tables. Economy values are u64
//! (no negatives); balances are enforced server-side by the reducers.

use spacetimedb::ReducerContext;
use spacetimedb::ScheduleAt;


/// Current unix timestamp in seconds (server clock only — never client input).
pub fn now_secs(ctx: &ReducerContext) -> u64 {
    ctx.timestamp.to_micros_since_unix_epoch() as u64 / 1_000_000
}

// ---------------------------------------------------------------------------
// 1. players
// ---------------------------------------------------------------------------

/// Player account. Wallet `address` is the unique identity; the SpacetimeDB
/// `identity` of the connection that claimed the address is bound at login.
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = player, public)]
pub struct Player {
    #[primary_key]
    pub address: String,
    /// SpacetimeDB identity that owns this account (bound on first login).
    pub identity: String,
    pub status: String, // "online" | "offline"
    pub display_name: Option<String>,
    pub avatar: String, // "Tetrahedron" | "Cube" | "Sphere" | "Cylinder" | "Cone"
    pub bio: Option<String>,

    pub level: u32,
    pub total_xp: u64,
    pub gold: u64,
    pub usdt: u64, // 6-decimal virtual USDT balance (chain-backed in prod)
    pub eco_points: u32,
    pub lifetime_gold_earned: u64,
    pub lifetime_gold_spent: u64,

    pub position_x: f32,
    pub position_y: f32,
    pub hex_q: i32,
    pub hex_r: i32,
    pub hex_id: u64,
    /// Equipped vehicle type ("None" | "Bicycle" | ...).
    pub vehicle: String,
    /// JSON array of equipped cosmetic names.
    pub cosmetics: String,
    /// JSON array of purchased templates (github URLs).
    pub templates: String,

    pub last_login: u64,
    pub last_seen: u64,
    pub last_action_at: u64, // interactions / teleport / spend
    pub last_spend: u64,     // idle-gain decay tracking
    pub created_at: u64,

    /// Idle-gain anti-cheat: rapid logins (< 5 min apart) flag the account.
    pub rapid_login_count: u32,
    /// No idle gains until this timestamp (90-day "new player" state).
    pub idle_gains_blocked_until: u64,

    // Activity statistics (Spec 014).
    pub total_play_time: u64,
    pub plants_planted: u64,
    pub plants_harvested: u64,
    pub pollution_cleaned: u64,
    pub templates_published: u64,
    pub templates_purchased: u64,
}

// ---------------------------------------------------------------------------
// 2. hex_tiles
// ---------------------------------------------------------------------------

/// One hex of the shared world. Terrain is generated once (seed-based,
/// server-authoritative); plants/pollution/eco mutate over time.
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = hex_tile, public)]
pub struct HexTile {
    #[primary_key]
    pub hex_id: u64,
    pub hex_q: i32,
    pub hex_r: i32,
    pub terrain: String, // Grass | Forest | Water | City | Desert | Polluted
    pub elevation: f32,
    pub eco_rating: i32,
    pub is_polluted: bool,
    pub plant: Option<String>, // serialized Plant (see Plant::to_json)
    pub planted_by: Option<String>,
    /// Set when a polluted hex was cleaned; pollution re-spreads after 48h.
    pub cleaned_at: Option<u64>,
    pub last_interaction: u64,
}

/// Plant data stored (JSON) on a hex row.
#[derive(Debug, Clone, PartialEq)]
pub struct Plant {
    pub plant_type: String, // Wheat | Corn | Sunflower | Tree | RareHerb
    pub planted_at: u64,
    pub growth_time: u64,
}

impl Plant {
    /// Growth times per the Ecosystem spec (2.5).
    pub fn growth_seconds(pt: &str) -> u64 {
        match pt {
            "Wheat" => 3_600,        // 1 h
            "Corn" => 5_400,         // 1.5 h
            "Sunflower" => 7_200,    // 2 h
            "Tree" => 21_600,        // 6 h
            "RareHerb" => 43_200,    // 12 h
            _ => 3_600,
        }
    }

    /// Planting cost in gold (Ecosystem spec 2.5).
    pub fn planting_cost(pt: &str) -> u64 {
        match pt {
            "Wheat" => 10,
            "Corn" => 15,
            "Sunflower" => 20,
            "Tree" => 50,
            "RareHerb" => 100,
            _ => 10,
        }
    }

    pub fn valid_type(pt: &str) -> bool {
        matches!(
            pt,
            "Wheat" | "Corn" | "Sunflower" | "Tree" | "RareHerb"
        )
    }

    pub fn is_mature(&self, now: u64) -> bool {
        now >= self.planted_at + self.growth_time
    }

    pub fn time_remaining(&self, now: u64) -> u64 {
        self.planted_at.saturating_add(self.growth_time).saturating_sub(now)
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"plant_type\":\"{}\",\"planted_at\":{},\"growth_time\":{}}}",
            self.plant_type, self.planted_at, self.growth_time
        )
    }

    /// Parse, tolerating both the current format and the legacy one.
    pub fn from_json(s: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(s).ok()?;
        let plant_type = v.get("plant_type")?.as_str()?.to_string();
        let planted_at = v.get("planted_at")?.as_u64()?;
        let growth_time = v
            .get("growth_time")
            .and_then(|g| g.as_u64())
            .unwrap_or_else(|| Self::growth_seconds(&plant_type));
        Some(Self {
            plant_type,
            planted_at,
            growth_time,
        })
    }
}

// ---------------------------------------------------------------------------
// 3. vehicles — one row per owned vehicle
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(
    accessor = player_vehicle,
    public,
    index(accessor = vehicle_by_player, btree(columns = [player]))
)]
pub struct VehicleOwned {
    #[primary_key]
    #[auto_inc]
    pub vehicle_id: u32,
    pub player: String,
    pub vehicle_type: String, // Bicycle | Scooter | Motorcycle | Boat | Airplane
    pub purchased_at: u64,
    pub equipped: bool,
    pub durability: u32,
    /// Last epoch-day on which maintenance was charged (5G/h, daily).
    pub last_maintenance_day: u32,
}

// ---------------------------------------------------------------------------
// 4. cosmetics — one row per owned cosmetic
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(
    accessor = player_cosmetic,
    public,
    index(accessor = cosmetic_by_player, btree(columns = [player]))
)]
pub struct CosmeticOwned {
    #[primary_key]
    #[auto_inc]
    pub cosmetic_id: u32,
    pub player: String,
    pub category: String, // Hat | Aura | Trail
    pub tier: String,     // Basic | Premium
    pub purchased_at: u64,
    pub equipped: bool,
}

// ---------------------------------------------------------------------------
// 5. voice_channels
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = voice_channel, public)]
pub struct VoiceChannel {
    #[primary_key]
    pub hex_id: u64,
    pub players: String, // JSON array of addresses
    pub created_at: u64,
    pub last_activity: u64,
    pub is_active: bool, // >= 2 players
}

// ---------------------------------------------------------------------------
// 6. market_listings
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(
    accessor = market_listing,
    public,
    index(accessor = listing_by_seller, btree(columns = [seller]))
)]
pub struct MarketListing {
    #[primary_key]
    #[auto_inc]
    pub listing_id: u64,
    pub seller: String,
    pub title: String,
    pub description: String,
    pub github_url: String,
    pub price_usdt: u64,
    pub category: String, // Agent | Code | Template | Snippet
    pub published_at: u64,
    pub expires_at: u64,
    pub is_sold: bool,
    pub buyer: Option<String>,
    /// 48h escrow; seller funds released when escrow_until passes or on
    /// explicit release. 0 = no pending escrow.
    pub escrow_until: u64,
    pub disputed: bool,
}

// ---------------------------------------------------------------------------
// 7. idle_gains — pending offline rewards + claim state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = idle_gain, public)]
pub struct IdleGain {
    #[primary_key]
    pub player: String,
    pub pending_xp: u64,
    pub pending_gold: u64,
    pub last_calculated_at: u64,
    pub claimed_at: Option<u64>,
}

// ---------------------------------------------------------------------------
// 8. transactions — economy ledger (Spec 010 FR7, Spec 019)
// ---------------------------------------------------------------------------

/// One immutable economy entry per action. Serves as the audit ledger and the
/// replication channel for level-up / eco / gold events.
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = transaction, public)]
pub struct Transaction {
    #[primary_key]
    #[auto_inc]
    pub tx_id: u64,
    pub player: String,
    pub timestamp: u64,
    pub action: String, // plant | harvest | clean | teleport | buy_vehicle | ...
    pub gold_change: i64,
    pub xp_change: i64,
    pub eco_points_change: i64,
    pub balance_after: u64,
}

// ---------------------------------------------------------------------------
// 9. scheduled_functions_state — audit trail for scheduler runs (Spec 015 FR6)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = scheduled_log, public)]
pub struct ScheduledLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub function_name: String,
    pub timestamp: u64,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// 10. eco_transactions — per-action eco ledger (Spec 020 T4.4/T6.7)
// ---------------------------------------------------------------------------

/// One eco point change with the hex rating before/after.
#[derive(Clone, Debug)]
#[spacetimedb::table(
    accessor = eco_transaction,
    public,
    index(accessor = eco_tx_by_player, btree(columns = [player]))
)]
pub struct EcoTransaction {
    #[primary_key]
    #[auto_inc]
    pub tx_id: u64,
    pub player: String,
    pub hex_id: u64,
    pub action: String, // plant | harvest | clean | spend
    pub points_earned: i64,
    pub rating_before: i32,
    pub rating_after: i32,
}

// ---------------------------------------------------------------------------
// Internal scheduler tables (SpacetimeDB recurring reducers)
// ---------------------------------------------------------------------------

/// Idle gain accrual — every 5 minutes (Spec 001 NFR2, Spec 015 FR1).
#[derive(Clone, Debug)]
#[spacetimedb::table(scheduled(idle_gains_tick), public, accessor = scheduled_idle_gains)]
pub struct ScheduledIdleGains {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub payload: u64,
}

/// Plant growth / maturity — every 10 seconds (Spec 015 FR2).
#[derive(Clone, Debug)]
#[spacetimedb::table(scheduled(plant_growth_tick), public, accessor = scheduled_plant_growth)]
pub struct ScheduledPlantGrowth {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub payload: u64,
}

/// Voice channel cleanup — every 1 minute (Spec 015 FR3).
#[derive(Clone, Debug)]
#[spacetimedb::table(scheduled(voice_cleanup_tick), public, accessor = scheduled_voice_cleanup)]
pub struct ScheduledVoiceCleanup {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub payload: u64,
}

/// Marketplace listing expiration + renewal — every 1 hour (Spec 015 FR4).
#[derive(Clone, Debug)]
#[spacetimedb::table(scheduled(market_cleanup_tick), public, accessor = scheduled_market_cleanup)]
pub struct ScheduledMarketCleanup {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub payload: u64,
}

/// Eco decay, pollution spread, vehicle maintenance — every 1 hour (Spec 020).
#[derive(Clone, Debug)]
#[spacetimedb::table(scheduled(eco_maintenance_tick), public, accessor = scheduled_eco_maintenance)]
pub struct ScheduledEcoMaintenance {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub payload: u64,
}

// ---------------------------------------------------------------------------
// Game constants (shared with the reducers)
// ---------------------------------------------------------------------------

/// Maximum concurrent players allowed in one hex (PROPOSAL 3.3).
pub const MAX_PLAYERS_PER_HEX: usize = 8;
/// Interaction lock timeout (PROPOSAL 3.3): 2 s to acquire a hex lock.
pub const HEX_LOCK_TIMEOUT_SECS: u64 = 2;
/// "Already harvested" rejection cooldown (PROPOSAL 3.3): 3 s.
pub const HARVEST_CONFLICT_COOLDOWN: u64 = 3;
/// Action cooldown between interactions (Spec 004): 5 s.
pub const ACTION_COOLDOWN_SECS: u64 = 5;
/// Vehicles cost 5G/h, applied daily at midnight (Ecosystem spec 2.1).
pub const VEHICLE_MAINTENANCE_PER_HOUR: u64 = 5;
/// Plant harvest rewards.
pub const HARVEST_GOLD_REWARD: u64 = 15;
pub const HARVEST_XP_REWARD: u64 = 10;
/// Clean pollution (PROPOSAL 2.3): pay 20G, earn 20G back, +15 XP.
pub const CLEAN_COST: u64 = 20;
pub const CLEAN_GOLD_REWARD: u64 = 20;
pub const CLEAN_XP_REWARD: u64 = 15;
/// Eco points per action (Spec 020): clean +10, plant tree +5, harvest tree +2.
pub const ECO_FOR_CLEAN: i64 = 10;
pub const ECO_FOR_PLANT_TREE: i64 = 5;
pub const ECO_FOR_HARVEST_TREE: i64 = 2;
/// Hex eco rating changes (Spec 020): +10 clean, +5 plant, +2 harvest.
pub const RATING_FOR_CLEAN: i32 = 10;
pub const RATING_FOR_PLANT: i32 = 5;
pub const RATING_FOR_HARVEST: i32 = 2;
/// Polluted hex reverts to polluted 48h after being cleaned (PROPOSAL 3.5).
pub const POLLUTION_RESPAWN_SECS: u64 = 48 * 3600;
/// Marketplace publishing cost (Spec 011 FR1): 50G.
pub const LISTING_PUBLISH_COST: u64 = 50;
/// Listing renewal: 10G per 7 days (Ecosystem spec 2.4).
pub const LISTING_RENEWAL_COST: u64 = 10;
pub const LISTING_DURATION_SECS: u64 = 30 * 24 * 3600;
pub const LISTING_RENEWAL_PERIOD: u64 = 7 * 24 * 3600;
/// Listing grace period after expiry before deactivation: 24 h.
pub const LISTING_GRACE_SECS: u64 = 24 * 3600;
/// Marketplace commission (PROPOSAL 4.5): 5%.
pub const PLATFORM_FEE_PERMILLE: u64 = 50; // 5.0%
/// Escrow dispute window: 48 h (PROPOSAL 4.2).
pub const ESCROW_SECS: u64 = 48 * 3600;
/// Buyer-wins penalty when seller never delivers (PROPOSAL 4.2): 2%.
pub const DISPUTE_REFUND_PENALTY_PERMILLE: u64 = 20; // 2.0%
/// 90-day "new player" idle ban for rapid logins (PROPOSAL 2.2).
pub const RAPID_LOGIN_BAN_SECS: u64 = 90 * 24 * 3600;
pub const RAPID_LOGIN_WINDOW_SECS: u64 = 300;
/// Idle decay: no spending for 7 days → -10%/day, -25%/day past 15, -50% past 30.
pub const IDLE_DECAY_GRACE_SECS: u64 = 7 * 24 * 3600;
/// Initial player gold (Spec 014 / PROPOSAL): 100G.
pub const STARTING_GOLD: u64 = 100;

/// Teleport base cost and cooldown (Spec 008 / Ecosystem 2.3).
pub const TELEPORT_BASE_COST: u64 = 100;
pub const TELEPORT_COOLDOWN_SECS: u64 = 60;

/// Encode axial (q, r) into the u64 hex id: `(q << 32) | r`.
pub fn hex_id_of(q: i32, r: i32) -> u64 {
    ((q as u64) << 32) | (r as u32 as u64)
}

/// Decode q/r from a hex id (inverse of [`hex_id_of`], delegated to the
/// shared core encoding so client and server can never diverge).
pub fn hex_coords_of(hex_id: u64) -> (i32, i32) {
    let c = idlecore_core::hex::HexCoord::from_id(hex_id);
    (c.q, c.r)
}

/// Hex axial distance (cube coords).
pub fn hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
    let s1 = -q1 - r1;
    let s2 = -q2 - r2;
    ((q1 - q2).abs() + (r1 - r2).abs() + (s1 - s2).abs()) / 2
}

/// World position of a hex center (spec PROPOSAL 9.1): flat-top axial.
pub fn hex_center(q: i32, r: i32) -> (f32, f32) {
    let size = 10.0f32;
    let x = size * 3.0_f32.sqrt() * (q as f32 + r as f32 / 2.0);
    let y = size * 1.5 * r as f32;
    (x, y)
}
// ---------------------------------------------------------------------------
// Scheduled reducer wrappers (Spec 015) — the host invokes these on the
// schedule-table rows; bodies live in `scheduler.rs`.
// ---------------------------------------------------------------------------

use spacetimedb::reducer;

#[reducer]
pub fn idle_gains_tick(ctx: &ReducerContext, _row: ScheduledIdleGains) {
    crate::scheduler::idle_gains_tick_body(ctx);
}

#[reducer]
pub fn plant_growth_tick(ctx: &ReducerContext, _row: ScheduledPlantGrowth) {
    crate::scheduler::plant_growth_tick_body(ctx);
}

#[reducer]
pub fn voice_cleanup_tick(ctx: &ReducerContext, _row: ScheduledVoiceCleanup) {
    crate::scheduler::voice_cleanup_tick_body(ctx);
}

#[reducer]
pub fn market_cleanup_tick(ctx: &ReducerContext, _row: ScheduledMarketCleanup) {
    crate::scheduler::market_cleanup_tick_body(ctx);
}

#[reducer]
pub fn eco_maintenance_tick(ctx: &ReducerContext, _row: ScheduledEcoMaintenance) {
    crate::scheduler::eco_maintenance_tick_body(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plant_json_round_trip() {
        let p = Plant {
            plant_type: "Wheat".into(),
            planted_at: 1234,
            growth_time: 3600,
        };
        let s = p.to_json();
        let back = Plant::from_json(&s).expect("parse own output");
        assert_eq!(back.plant_type, "Wheat");
        assert_eq!(back.planted_at, 1234);
        assert_eq!(back.growth_time, 3600);
    }

    #[test]
    fn plant_json_tolerates_legacy_missing_growth_time() {
        let s = r#"{"plant_type":"Tree","planted_at":42}"#;
        let p = Plant::from_json(s).expect("tolerant parse");
        assert_eq!(p.plant_type, "Tree");
        assert_eq!(p.growth_time, Plant::growth_seconds("Tree"));
    }

    #[test]
    fn plant_json_rejects_garbage() {
        assert!(Plant::from_json("not json").is_none());
        assert!(Plant::from_json(r#"{"planted_at":1}"#).is_none());
    }

    #[test]
    fn plant_maturity_window() {
        let p = Plant {
            plant_type: "Wheat".into(),
            planted_at: 1000,
            growth_time: 3600,
        };
        assert!(!p.is_mature(3000), "not yet grown");
        assert!(p.is_mature(1000 + 3600), "fully grown");
        assert!(p.is_mature(9000), "overgrown stays mature");
        assert_eq!(p.time_remaining(1000), 3600);
    }
}
