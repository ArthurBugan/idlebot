//! Core types e utilitários compartilhados entre cliente, servidor e blockchain

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    pub fn to_hex(&self, hex_radius: f32) -> u64 {
        let q = (self.x * 2.0 / (3.0_f32.sqrt() * hex_radius)) as i64;
        let r = (self.y * 2.0 / (3.0 * hex_radius * 0.75)) as i64;
        let s = -q - r;
        let rq = q as f64;
        let rr = r as f64;
        let rs = s as f64;
        let fq = rq.round();
        let fr = rr.round();
        let fs = rs.round();
        let dq = (fq - rq).abs();
        let dr = (fr - rr).abs();
        let ds = (fs - rs).abs();
        let (fq, fr) = if dq > dr && dq > ds {
            (-fr - fs, fr)
        } else if dr > ds {
            (fq, -fq - fs)
        } else {
            (fq, fr)
        };
        ((fq as u64) << 32) | (fr as u64)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Forest,
    Water,
    Polluted,
    City,
    Desert,
}

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
    pub grow_duration: Duration,
    pub owner: Option<WalletAddress>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlantType {
    Wheat,
    Tomato,
    Tree,
    Sunflower,
    RareHerb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexTile {
    pub hex_id: u64,
    pub position: Position,
    pub terrain: TerrainType,
    pub plant: Option<Plant>,
    pub is_polluted: bool,
    pub eco_rating: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

impl TerrainType {
    pub fn color(&self) -> Color {
        match self {
            TerrainType::Grass => Color::srgb(0.35, 0.65, 0.2),
            TerrainType::Forest => Color::srgb(0.15, 0.55, 0.25),
            TerrainType::Water => Color::srgb(0.2, 0.4, 0.7),
            TerrainType::City => Color::srgb(0.7, 0.65, 0.55),
            TerrainType::Desert => Color::srgb(0.85, 0.7, 0.3),
            TerrainType::Polluted => Color::srgb(0.15, 0.15, 0.15),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cosmetic {
    pub cosmetic_id: u64,
    pub name: String,
    pub category: CosmeticCategory,
    pub cost_gold: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CosmeticCategory {
    Hat,
    Aura,
    Trail,
    VehicleSkin,
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
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
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

pub mod idle_config {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IdleGains {
        pub xp: u64,
        pub gold: u64,
    }

    pub fn gains_for_time(elapsed: Duration) -> IdleGains {
        let seconds = elapsed.as_secs();
        if seconds < 3600 {
            IdleGains { xp: 10, gold: 5 }
        } else if seconds < 21600 {
            IdleGains { xp: 60, gold: 30 }
        } else if seconds < 43200 {
            IdleGains { xp: 100, gold: 50 }
        } else {
            IdleGains { xp: 150, gold: 75 }
        }
    }

    pub const MAX_IDLE_SECONDS: u64 = 86400;
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
    pub voice_channel_timeout: Duration,
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
            voice_channel_timeout: Duration::from_secs(300),
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

    #[test]
    fn hex_tile_polluted() {
        let tile = HexTile {
            hex_id: 2,
            position: Position::new(10.0, 0.0),
            terrain: TerrainType::Polluted,
            plant: None,
            is_polluted: true,
            eco_rating: 10,
        };
        assert!(tile.is_polluted);
        assert_eq!(tile.terrain, TerrainType::Polluted);
    }

    #[test]
    fn plant_stage_progression() {
        let mut plant = Plant {
            plant_type: PlantType::Wheat,
            stage: PlantStage::Planted,
            planted_at: 0,
            grow_duration: Duration::from_secs(300),
            owner: None,
        };
        assert_eq!(plant.stage, PlantStage::Planted);
        plant.stage = PlantStage::Growing;
        assert_eq!(plant.stage, PlantStage::Growing);
        plant.stage = PlantStage::Ready;
        assert_eq!(plant.stage, PlantStage::Ready);
    }

    #[test]
    fn terrain_type_serde_roundtrip() {
        let terrains = [
            TerrainType::Grass,
            TerrainType::Forest,
            TerrainType::Water,
            TerrainType::Polluted,
            TerrainType::City,
            TerrainType::Desert,
        ];
        for t in &terrains {
            let json = serde_json::to_string(t).unwrap();
            let deserialized: TerrainType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deserialized);
        }
    }

    #[test]
    fn player_serde_roundtrip() {
        let p = Player::new("0xABCDEF1234567890".into(), Position::new(100.0, 200.0));
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: Player = serde_json::from_str(&json).unwrap();
        assert_eq!(p.address, deserialized.address);
        assert_eq!(p.xp, deserialized.xp);
        assert_eq!(p.gold, deserialized.gold);
    }

    #[test]
    fn hex_tile_serde_roundtrip() {
        let tile = HexTile {
            hex_id: 42,
            position: Position::new(5.0, 5.0),
            terrain: TerrainType::Forest,
            plant: None,
            is_polluted: false,
            eco_rating: 75,
        };
        let json = serde_json::to_string(&tile).unwrap();
        let deserialized: HexTile = serde_json::from_str(&json).unwrap();
        assert_eq!(tile.hex_id, deserialized.hex_id);
        assert_eq!(tile.terrain, deserialized.terrain);
    }

    #[test]
    fn max_idle_seconds() {
        assert_eq!(idle_config::MAX_IDLE_SECONDS, 86400);
    }

    #[test]
    fn all_vehicle_types_deserialize() {
        let jsons = [
            "\"None\"",
            "\"Bicycle\"",
            "\"Scooter\"",
            "\"Motorcycle\"",
            "\"Boat\"",
            "\"Airplane\"",
        ];
        for j in &jsons {
            let _: Vehicle = serde_json::from_str(j).unwrap();
        }
    }
}
