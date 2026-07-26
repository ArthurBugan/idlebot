//! Types para o servidor SpacetimeDB

use spacetimedb::table;
use serde::{Deserialize, Serialize};

/// Struct pra representar um jogador no banco
#[table(name = "player", public)]
pub struct PlayerDbEntry {
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
#[table(name = "hex_tile", public)]
pub struct HexTileDbEntry {
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: String,
    pub plant: Option<String>,
    pub is_polluted: bool,
    pub eco_rating: u32,
}

/// Struct pra representar um channel de voz
#[table(name = "voice_channel", public)]
pub struct VoiceChannelDbEntry {
    pub hex_id: u64,
    pub players: String,
    pub created_at: u64,
    pub last_activity: u64,
}

/// Struct pra representar um listing de mercado
#[table(name = "market_listing", public)]
pub struct MarketListingDbEntry {
    pub listing_id: u64,
    pub seller: String,
    pub title: String,
    pub github_url: String,
    pub description: String,
    pub price_usdt: f64,
    pub published_at: u64,
    pub sold: bool,
}
