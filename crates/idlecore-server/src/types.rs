//! Types para o servidor SpacetimeDB

use serde::{Deserialize, Serialize};
use spacetimedb::table;

/// PlantType serialized as JSON string (Wheat, Corn, Tree, RareHerb)
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
            "Wheat" => Some(PlantTypeString::Wheat),
            "Corn" => Some(PlantTypeString::Corn),
            "Tree" => Some(PlantTypeString::Tree),
            "RareHerb" => Some(PlantTypeString::RareHerb),
            _ => None,
        }
    }
}

impl std::fmt::Display for PlantTypeString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlantTypeString::Wheat => write!(f, "Wheat"),
            PlantTypeString::Corn => write!(f, "Corn"),
            PlantTypeString::Tree => write!(f, "Tree"),
            PlantTypeString::RareHerb => write!(f, "RareHerb"),
        }
    }
}

/// Plant JSON for DB storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantJson {
    pub plant_type: String,
    pub planted_at: u64,
    pub growth_time_seconds: u64,
}

impl PlantJson {
    pub fn new(plant_type: &str, planted_at: u64) -> Self {
        let growth_time = match plant_type {
            "Wheat" => 3600,
            "Corn" => 5400,
            "Tree" => 21600,
            "RareHerb" => 43200,
            _ => 3600,
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
}

/// Struct pra representar um jogador no banco
#[table(accessor = player, public)]
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

/// Struct pra representar um hexágono no banco
#[derive(Clone)]
#[table(accessor = hex_tile, public)]
pub struct HexTileDbEntry {
    #[primary_key]
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: String,
    pub plant: Option<String>,  // JSON serialized Plant
    pub is_polluted: bool,
    pub eco_rating: u32,
}

/// Struct pra representar um channel de voz
#[table(accessor = voice_channel, public)]
pub struct VoiceChannelDbEntry {
    #[primary_key]
    pub hex_id: u64,
    pub players: String,
    pub created_at: u64,
    pub last_activity: u64,
}

/// Struct pra representar um listing de mercado
#[table(accessor = market_listing, public)]
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

/// Struct pra rastrear idle gains pendentes de cada jogador
#[table(accessor = idle_gains, public)]
pub struct IdleGainsEntry {
    #[primary_key]
    pub player_id: String,
    pub pending_xp: u64,
    pub pending_gold: u64,
    pub last_calculated_at: u64,
}
