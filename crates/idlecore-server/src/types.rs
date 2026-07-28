//! Types para o servidor SpacetimeDB

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Table definitions
// ---------------------------------------------------------------------------

/// Hex tile in the world grid
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = hex_tile)]
pub struct HexTileDbEntry {
    #[primary_key]
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: String,
    pub plant: Option<String>,
    pub is_polluted: bool,
    pub eco_rating: i32,
}

/// Player account / persistence entity
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = player)]
pub struct PlayerDbEntry {
    #[primary_key]
    pub address: String,
    pub position_x: f32,
    pub position_y: f32,
    pub hex_id: u64,
    pub xp: u64,
    pub gold: u64,
    pub level: u32,
    pub eco_points: u64,
    pub last_seen: u64,
    pub is_online: bool,
    pub vehicle: String,
    pub cosmetics: String,
    pub templates: String,
    pub templates_limit: u32,
}

impl PlayerDbEntry {
    /// Try to parse and return the current vehicle string.
    pub fn current_vehicle_string(&self) -> &str {
        &self.vehicle
    }

    /// Set the current vehicle string (serialised).
    pub fn set_vehicle_string(&mut self, v: String) {
        self.vehicle = v;
    }

    /// Deduct gold from this player. Returns `Ok(())` or `Err` if insufficient funds.
    pub fn deduct_gold(&mut self, amount: u64) -> Result<(), String> {
        if self.gold < amount {
            return Err("Insufficient gold".to_string());
        }
        self.gold = self.gold.saturating_sub(amount);
        Ok(())
    }

    /// Add gold to this player.
    pub fn add_gold(&mut self, amount: u64) {
        self.gold += amount;
    }

    /// Add XP to this player.
    pub fn add_xp(&mut self, amount: u64) {
        self.xp += amount;
    }
}

/// Voice channel
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = voice_channel)]
pub struct VoiceChannelDbEntry {
    #[primary_key]
    pub hex_id: u64,
    pub players: String,
    pub created_at: u64,
    pub last_activity: u64,
    pub is_active: bool,
}

/// Market listing (template for sale)
#[derive(Clone, Debug)]
#[spacetimedb::table(accessor = market_listing)]
pub struct MarketListingDbEntry {
    #[primary_key]
    pub listing_id: u64,
    pub seller: String,
    pub title: String,
    pub github_url: String,
    pub description: String,
    pub price_usdt: f64,
    pub published_at: u64,
    pub sold: bool,
}

// ---------------------------------------------------------------------------
// PlantJson – serialised plant data stored on HexTileDbEntry.plant
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantJson {
    pub plant_type: String,
    pub planted_at: u64,
    pub growth_time_seconds: u64,
}

impl PlantJson {
    pub fn new(plant_type: &str, planted_at: u64) -> Self {
        let growth_time = match plant_type {
            "Wheat" => 3_600,
            "Corn" => 7_200,
            "Tree" => 21_600,
            "RareHerb" => 43_200,
            _ => 3_600,
        };
        Self {
            plant_type: plant_type.to_string(),
            planted_at,
            growth_time_seconds: growth_time,
        }
    }

    pub fn is_mature(&self, now: u64) -> bool {
        now >= self.planted_at + self.growth_time_seconds
    }

    pub fn time_remaining(&self, now: u64) -> u64 {
        let target = self.planted_at + self.growth_time_seconds;
        if now >= target {
            0
        } else {
            target - now
        }
    }
}

/// Simplified plant type enum for JSON transport (used in farming.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantTypeString {
    Wheat,
    Corn,
    Tree,
    RareHerb,
}

impl PlantTypeString {
    pub fn to_json(&self) -> String {
        match self {
            PlantTypeString::Wheat => "\"Wheat\"".to_string(),
            PlantTypeString::Corn => "\"Corn\"".to_string(),
            PlantTypeString::Tree => "\"Tree\"".to_string(),
            PlantTypeString::RareHerb => "\"RareHerb\"".to_string(),
        }
    }

    pub fn from_json(s: &str) -> Option<Self> {
        match s {
            "\"Wheat\"" => Some(PlantTypeString::Wheat),
            "\"Corn\"" => Some(PlantTypeString::Corn),
            "\"Tree\"" => Some(PlantTypeString::Tree),
            "\"RareHerb\"" => Some(PlantTypeString::RareHerb),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Idle gains tracking
// ---------------------------------------------------------------------------

/// Entry representing a player's pending idle gains (used by the scheduler).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleGainsEntry {
    pub player_id: String,
    pub pending_xp: u64,
    pub pending_gold: u64,
    pub last_calculated_at: u64,
}
