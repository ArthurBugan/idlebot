//! Player Identity Management — Avatar, display name, bio, and player stats.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Avatar type for player appearance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvatarType {
    Tetrahedron,
    Cube,
    Sphere,
    Cylinder,
    Cone,
}

impl Default for AvatarType {
    fn default() -> Self {
        AvatarType::Sphere
    }
}

impl AvatarType {
    /// Get display name for avatar
    pub fn display_name(&self) -> &str {
        match self {
            AvatarType::Tetrahedron => "Tetrahedron",
            AvatarType::Cube => "Cube",
            AvatarType::Sphere => "Sphere",
            AvatarType::Cylinder => "Cylinder",
            AvatarType::Cone => "Cone",
        }
    }
}

/// Player statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    pub level: u32,
    pub total_xp: u64,
    pub plants_planted: u64,
    pub plants_harvested: u64,
    pub pollution_cleaned: u64,
    pub templates_published: u64,
    pub templates_purchased: u64,
    pub play_time_seconds: u64,
}

impl PlayerStats {
    /// Create stats from player economy data
    pub fn from_economy(level: u32, total_xp: u64) -> Self {
        Self {
            level,
            total_xp,
            ..Default::default()
        }
    }

    /// Record a plant action
    pub fn record_plant(&mut self) {
        self.plants_planted += 1;
    }

    /// Record a harvest action
    pub fn record_harvest(&mut self) {
        self.plants_harvested += 1;
    }

    /// Record a clean action
    pub fn record_clean(&mut self) {
        self.pollution_cleaned += 1;
    }

    /// Record a template publish
    pub fn record_publish(&mut self) {
        self.templates_published += 1;
    }

    /// Record a template purchase
    pub fn record_purchase(&mut self) {
        self.templates_purchased += 1;
    }

    /// Add play time
    pub fn add_play_time(&mut self, seconds: u64) {
        self.play_time_seconds += seconds;
    }
}

/// Full player identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub address: String,
    pub display_name: String,
    pub avatar: AvatarType,
    pub bio: String,
    pub level: u32,
    pub total_xp: u64,
    pub gold: u64,
    pub eco_points: u64,
    pub position_x: f32,
    pub position_z: f32,
    pub current_hex: Option<u64>,
    pub created_at: u64,
    pub last_login: u64,
    pub stats: PlayerStats,
}

impl Player {
    /// Create a new player with defaults
    pub fn new(id: u64, address: &str) -> Self {
        let now = Self::now_secs();
        let name_suffix = if address.len() > 10 {
            &address[2..10]
        } else {
            &address[2..]
        };
        Self {
            id,
            address: address.to_lowercase(),
            display_name: format!("Player_{}", name_suffix),
            avatar: AvatarType::default(),
            bio: String::new(),
            level: 1,
            total_xp: 0,
            gold: 100,
            eco_points: 0,
            position_x: 0.0,
            position_z: 0.0,
            current_hex: None,
            created_at: now,
            last_login: now,
            stats: PlayerStats::default(),
        }
    }

    /// Update display name with validation
    pub fn update_display_name(&mut self, name: &str) -> Result<(), String> {
        if name.len() > 20 {
            return Err("Display name too long (max 20 chars)".to_string());
        }
        if name.is_empty() {
            return Err("Display name cannot be empty".to_string());
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err("Display name can only contain letters, numbers, _ and -".to_string());
        }
        self.display_name = name.to_string();
        Ok(())
    }

    /// Update player level and XP
    pub fn update_xp(&mut self, xp_gained: u64) {
        self.total_xp += xp_gained;
        // Simple level calculation: level = sqrt(xp / 100)
        self.level = (self.total_xp as f64 / 100.0).sqrt() as u32 + 1;
    }

    /// Update player position
    pub fn update_position(&mut self, x: f32, z: f32) {
        self.position_x = x;
        self.position_z = z;
    }

    /// Get current time in seconds
    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Player manager — in-memory store of all players
pub struct PlayerManager {
    players: HashMap<String, Player>, // address -> Player
    next_id: u64,
}

impl PlayerManager {
    /// Create a new PlayerManager
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new player from wallet address
    pub fn create_player(&mut self, address: &str) -> &Player {
        let addr_lower = address.to_lowercase();
        if !self.players.contains_key(&addr_lower) {
            let id = self.next_id;
            self.next_id += 1;
            let player = Player::new(id, address);
            self.players.insert(addr_lower, player);
        }
        self.players.get(&address.to_lowercase()).unwrap()
    }

    /// Get a player by address
    pub fn get_player(&self, address: &str) -> Option<&Player> {
        self.players.get(&address.to_lowercase())
    }

    /// Get a player by ID
    pub fn get_player_by_id(&self, id: u64) -> Option<&Player> {
        self.players.values().find(|p| p.id == id)
    }

    /// Update display name
    pub fn update_display_name(&mut self, address: &str, name: &str) -> Result<(), String> {
        let addr_lower = address.to_lowercase();
        if let Some(player) = self.players.get_mut(&addr_lower) {
            player.update_display_name(name)
        } else {
            Err("Player not found".to_string())
        }
    }

    /// Get player stats
    pub fn get_player_stats(&self, address: &str) -> Option<&PlayerStats> {
        self.players.get(&address.to_lowercase()).map(|p| &p.stats)
    }

    /// Update player position
    pub fn update_player_position(&mut self, address: &str, x: f32, z: f32) {
        if let Some(player) = self.players.get_mut(&address.to_lowercase()) {
            player.update_position(x, z);
        }
    }

    /// Record a harvest action for stats tracking
    pub fn record_harvest(&mut self, address: &str) {
        if let Some(player) = self.players.get_mut(&address.to_lowercase()) {
            player.stats.record_harvest();
        }
    }

    /// Record a clean action for stats tracking
    pub fn record_clean(&mut self, address: &str) {
        if let Some(player) = self.players.get_mut(&address.to_lowercase()) {
            player.stats.record_clean();
        }
    }

    /// Add play time
    pub fn add_play_time(&mut self, address: &str, seconds: u64) {
        if let Some(player) = self.players.get_mut(&address.to_lowercase()) {
            player.stats.add_play_time(seconds);
        }
    }

    /// Update last login time
    pub fn update_last_login(&mut self, address: &str) {
        if let Some(player) = self.players.get_mut(&address.to_lowercase()) {
            player.last_login = Player::now_secs();
        }
    }

    /// Get total player count
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avatar_display_name() {
        assert_eq!(AvatarType::Sphere.display_name(), "Sphere");
        assert_eq!(AvatarType::Cube.display_name(), "Cube");
    }

    #[test]
    fn test_player_new() {
        let player = Player::new(1, "0x123");
        assert_eq!(player.id, 1);
        assert_eq!(player.address, "0x123");
        assert_eq!(player.level, 1);
        assert_eq!(player.gold, 100);
    }

    #[test]
    fn test_player_update_display_name_valid() {
        let mut player = Player::new(1, "0x123");
        assert!(player.update_display_name("TestPlayer").is_ok());
        assert_eq!(player.display_name, "TestPlayer");
    }

    #[test]
    fn test_player_update_display_name_too_long() {
        let mut player = Player::new(1, "0x123");
        assert!(player.update_display_name("ThisNameIsWayTooLong12345").is_err());
    }

    #[test]
    fn test_player_update_display_name_invalid_chars() {
        let mut player = Player::new(1, "0x123");
        assert!(player.update_display_name("Invalid@Name!").is_err());
    }

    #[test]
    fn test_player_update_xp() {
        let mut player = Player::new(1, "0x123");
        player.update_xp(100);
        assert_eq!(player.total_xp, 100);
        assert_eq!(player.level, 2);
    }

    #[test]
    fn test_player_stats_record_actions() {
        let mut stats = PlayerStats::default();
        stats.record_plant();
        stats.record_harvest();
        stats.record_clean();
        stats.record_publish();
        stats.record_purchase();
        stats.add_play_time(300);
        
        assert_eq!(stats.plants_planted, 1);
        assert_eq!(stats.plants_harvested, 1);
        assert_eq!(stats.pollution_cleaned, 1);
        assert_eq!(stats.templates_published, 1);
        assert_eq!(stats.templates_purchased, 1);
        assert_eq!(stats.play_time_seconds, 300);
    }

    #[test]
    fn test_player_manager_create() {
        let mut pm = PlayerManager::new();
        let player = pm.create_player("0x123");
        assert_eq!(player.address, "0x123");
        assert_eq!(pm.player_count(), 1);
    }

    #[test]
    fn test_player_manager_get_by_address() {
        let mut pm = PlayerManager::new();
        pm.create_player("0x123");
        let player = pm.get_player("0x123");
        assert!(player.is_some());
        assert_eq!(player.unwrap().display_name, "Player_123");
    }

    #[test]
    fn test_player_manager_get_by_id() {
        let mut pm = PlayerManager::new();
        pm.create_player("0x123");
        let player = pm.get_player_by_id(1);
        assert!(player.is_some());
    }

    #[test]
    fn test_player_manager_update_display_name() {
        let mut pm = PlayerManager::new();
        pm.create_player("0x123");
        assert!(pm.update_display_name("0x123", "NewName").is_ok());
        assert_eq!(pm.get_player("0x123").unwrap().display_name, "NewName");
    }

    #[test]
    fn test_player_manager_record_harvest() {
        let mut pm = PlayerManager::new();
        pm.create_player("0x123");
        pm.record_harvest("0x123");
        let stats = pm.get_player_stats("0x123").unwrap();
        assert_eq!(stats.plants_harvested, 1);
    }

    #[test]
    fn test_player_manager_update_position() {
        let mut pm = PlayerManager::new();
        pm.create_player("0x123");
        pm.update_player_position("0x123", 100.0, 200.0);
        let player = pm.get_player("0x123").unwrap();
        assert_eq!(player.position_x, 100.0);
        assert_eq!(player.position_z, 200.0);
    }
}
