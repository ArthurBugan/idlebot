//! Core types shared between client, server, and blockchain.

use serde::{Deserialize, Serialize};
use crate::terrain::TerrainType;
use crate::cosmetic::CosmeticCategory;

pub mod actions;
pub mod economy;
pub mod grid;
pub mod hex;
pub mod hex_tile;
pub mod marketplace;
pub mod plant;
pub mod cosmetic;
pub mod player;
pub mod progression;
pub mod teleport;
pub mod terrain;
pub mod ui;
pub mod vehicle;
pub mod voice;
pub mod idle_config;

/// Simple RGBA color (0.0–1.0 per channel)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn srgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Convert to Bevy Color type for materials.
    pub fn to_bevy(&self) -> bevy::color::Color {
        bevy::color::Color::srgb(self.r, self.g, self.b)
    }
}

/// Endereço wallet Polygon (20 bytes, hex)
pub type WalletAddress = String;

/// Coordenada no mapa (em metros do centro)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Convert world 2D position to axial hex coordinates.
    pub fn to_hex(&self, hex_radius: f32) -> u64 {
        hex::world_pos_to_hex(self.x, self.y, hex_radius).0 as u64
    }
}

/// Core hex tile data (shared between core, server, client).
pub use hex_tile::HexTileData;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlantStage {
    Planted,
    Growing,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plant {
    pub plant_type: PlantType,
    pub stage: PlantStage,
    pub planted_at: u64,
    pub grow_duration: std::time::Duration,
    pub owner: Option<WalletAddress>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlantType {
    Wheat,
    Corn,
    Tree,
    RareHerb,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Vehicle {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

impl Vehicle {
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Vehicle::None => 1.0,
            Vehicle::Bicycle => 2.0,
            Vehicle::Scooter => 3.0,
            Vehicle::Motorcycle => 5.0,
            Vehicle::Boat => 4.0,
            Vehicle::Airplane => 10.0,
        }
    }

    pub fn purchase_cost(&self) -> u64 {
        match self {
            Vehicle::None => 0,
            Vehicle::Bicycle => 500,
            Vehicle::Scooter => 1000,
            Vehicle::Motorcycle => 2500,
            Vehicle::Boat => 2000,
            Vehicle::Airplane => 10000,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Vehicle::None => "None",
            Vehicle::Bicycle => "Bicycle",
            Vehicle::Scooter => "Scooter",
            Vehicle::Motorcycle => "Motorcycle",
            Vehicle::Boat => "Boat",
            Vehicle::Airplane => "Airplane",
        }
    }

    pub fn all_vehicles() -> &'static [Vehicle] {
        &[
            Vehicle::Bicycle,
            Vehicle::Scooter,
            Vehicle::Motorcycle,
            Vehicle::Boat,
            Vehicle::Airplane,
        ]
    }
}

impl TerrainType {
    /// Get the Bevy Color for this terrain.
    pub fn color(&self) -> Color {
        match self {
            TerrainType::Grass => Color::srgb(0.496, 0.792, 0.322),
            TerrainType::Forest => Color::srgb(0.133, 0.545, 0.133),
            TerrainType::Water => Color::srgb(0.255, 0.404, 0.882),
            TerrainType::City => Color::srgb(0.502, 0.502, 0.502),
            TerrainType::Desert => Color::srgb(0.953, 0.643, 0.376),
            TerrainType::Polluted => Color::srgb(0.294, 0.000, 0.514),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cosmetic {
    pub cosmetic_id: u64,
    pub name: String,
    pub category: CosmeticCategory,
    pub cost_gold: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub listing_id: u64,
    pub title: String,
    pub github_url: String,
    pub author: WalletAddress,
    pub price_paid_usdt: f64,
    pub purchased_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub address: WalletAddress,
    pub position: Position,
    pub hex_id: u64,
    pub vehicle: Vehicle,
    pub xp: u64,
    pub gold: u64,
    pub level: u32,
    pub eco_points: u64,
    pub last_seen: u64,
    pub is_online: bool,
    pub cosmetics: Vec<Cosmetic>,
    pub templates: Vec<Template>,
    pub templates_limit: u32,
}

impl Player {
    pub fn new(address: WalletAddress, spawn_position: Position) -> Self {
        Self {
            address,
            position: spawn_position,
            hex_id: spawn_position.to_hex(10.0),
            vehicle: Vehicle::None,
            xp: 0,
            gold: 100,
            level: 1,
            eco_points: 0,
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            is_online: false,
            cosmetics: Vec::new(),
            templates: Vec::new(),
            templates_limit: 50,
        }
    }

    pub fn xp_for_next_level(level: u32) -> u64 {
        100 * (level as u64).pow(2)
    }

    pub fn calculate_level(total_xp: u64) -> u32 {
        let mut level = 1u32;
        let mut xp_needed = 100u64;
        let mut remaining = total_xp;
        while remaining >= xp_needed {
            remaining -= xp_needed;
            level += 1;
            xp_needed = Self::xp_for_next_level(level);
        }
        level
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HexAction {
    Plant(PlantType),
    Harvest,
    CleanPollution,
    ClearTerrain,
    Teleport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    Success {
        xp_gained: u64,
        gold_gained: u64,
        message: String,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuyableItem {
    Vehicle(Vehicle),
    Cosmetic(Cosmetic),
}

impl BuyableItem {
    pub fn cost_gold(&self) -> u64 {
        match self {
            BuyableItem::Vehicle(v) => match v {
                Vehicle::None => 0,
                Vehicle::Bicycle => 500,
                Vehicle::Scooter => 1_000,
                Vehicle::Motorcycle => 2_500,
                Vehicle::Boat => 2_000,
                Vehicle::Airplane => 10_000,
            },
            BuyableItem::Cosmetic(c) => c.cost_gold,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            BuyableItem::Vehicle(v) => match v {
                Vehicle::None => "None",
                Vehicle::Bicycle => "Electric Bicycle",
                Vehicle::Scooter => "Electric Scooter",
                Vehicle::Motorcycle => "Electric Motorcycle",
                Vehicle::Boat => "Electric Boat",
                Vehicle::Airplane => "Electric Airplane",
            },
            BuyableItem::Cosmetic(c) => &c.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketListing {
    pub listing_id: u64,
    pub seller: WalletAddress,
    pub title: String,
    pub github_url: String,
    pub description: String,
    pub price_usdt: f64,
    pub published_at: u64,
    pub sold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub listing_id: u64,
    pub buyer: WalletAddress,
    pub seller: WalletAddress,
    pub price_usdt: f64,
    pub tx_hash: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionEvent {
    pub user: WalletAddress,
    pub active: bool,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub hex_radius_meters: f32,
    pub max_players_per_hex: u32,
    pub voice_channel_timeout: std::time::Duration,
    pub idle_max_hours: u32,
    pub market_fee_percent: f64,
    pub min_template_price_usdt: f64,
    pub template_inventory_limit: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            hex_radius_meters: 10.0,
            max_players_per_hex: 20,
            voice_channel_timeout: std::time::Duration::from_secs(300),
            idle_max_hours: 24,
            market_fee_percent: 0.05,
            min_template_price_usdt: 0.01,
            template_inventory_limit: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_new() {
        let pos = Position::new(10.0, 20.0);
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn position_distance_to() {
        let a = Position::new(0.0, 0.0);
        let b = Position::new(3.0, 4.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn position_distance_same() {
        let a = Position::new(5.0, 10.0);
        assert!((a.distance_to(&a)).abs() < 0.001);
    }

    #[test]
    fn position_to_hex_center() {
        let pos = Position::new(0.0, 0.0);
        let hex = pos.to_hex(10.0);
        assert_eq!(hex, 0);
    }

    #[test]
    fn position_to_hex_asymmetric() {
        let a = Position::new(10.0, 0.0);
        let b = Position::new(-10.0, 0.0);
        assert_ne!(a.to_hex(10.0), b.to_hex(10.0));
    }

    #[test]
    fn player_new_default() {
        let p = Player::new("0x1234".into(), Position::new(0.0, 0.0));
        assert_eq!(p.xp, 0);
        assert_eq!(p.gold, 100);
        assert_eq!(p.level, 1);
        assert!(!p.is_online);
    }

    #[test]
    fn player_xp_for_next_level() {
        assert_eq!(Player::xp_for_next_level(1), 100);
        assert_eq!(Player::xp_for_next_level(2), 400);
        assert_eq!(Player::xp_for_next_level(3), 900);
    }

    #[test]
    fn player_calculate_level() {
        assert_eq!(Player::calculate_level(0), 1);
        assert_eq!(Player::calculate_level(99), 1);
        assert_eq!(Player::calculate_level(100), 2);
        assert_eq!(Player::calculate_level(499), 2);
        assert_eq!(Player::calculate_level(500), 3);
    }

    #[test]
    fn vehicle_speed() {
        assert_eq!(Vehicle::None.speed_multiplier(), 1.0);
        assert_eq!(Vehicle::Bicycle.speed_multiplier(), 2.0);
        assert_eq!(Vehicle::Scooter.speed_multiplier(), 3.0);
        assert_eq!(Vehicle::Motorcycle.speed_multiplier(), 5.0);
        assert_eq!(Vehicle::Boat.speed_multiplier(), 4.0);
        assert_eq!(Vehicle::Airplane.speed_multiplier(), 10.0);
    }

    #[test]
    fn buyable_vehicle_cost() {
        assert_eq!(BuyableItem::Vehicle(Vehicle::Bicycle).cost_gold(), 500);
        assert_eq!(BuyableItem::Vehicle(Vehicle::Airplane).cost_gold(), 10_000);
    }

    #[test]
    fn buyable_vehicle_name() {
        assert_eq!(
            BuyableItem::Vehicle(Vehicle::Bicycle).name(),
            "Electric Bicycle"
        );
        assert_eq!(
            BuyableItem::Vehicle(Vehicle::Airplane).name(),
            "Electric Airplane"
        );
    }

    #[test]
    fn game_config_default() {
        let cfg = GameConfig::default();
        assert_eq!(cfg.hex_radius_meters, 10.0);
        assert_eq!(cfg.max_players_per_hex, 20);
        assert_eq!(cfg.idle_max_hours, 24);
        assert!((cfg.market_fee_percent - 0.05).abs() < 0.001);
    }

    #[test]
    fn idle_gains_structure() {
        let gains = idle_config::IdleGains { xp: 10, gold: 5 };
        assert_eq!(gains.xp, 10);
        assert_eq!(gains.gold, 5);
    }
}
