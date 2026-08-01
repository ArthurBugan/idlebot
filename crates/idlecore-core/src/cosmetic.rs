//! Cosmetic system: items, inventory, equip/unequip, visual rendering.
//!
//! Categories: Hat, Aura, Trail
//! Tiers: Basic (gold), Premium (USDT)
//! Visual only — no gameplay advantage.

use serde::{Deserialize, Serialize};

// Cosmetic Categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CosmeticCategory {
    Hat,
    Aura,
    Trail,
}

// Cosmetic Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CosmeticType {
    Basic,
    Premium,
}

// Cosmetic Item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosmeticItem {
    pub id: u64,
    pub name: String,
    pub category: CosmeticCategory,
    pub cosmetic_type: CosmeticType,
    pub cost_gold: u64,
    pub cost_usdt: f64,
    pub purchased: bool,
    pub equipped: bool,
}

// Cosmetic Inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosmeticInventory {
    pub items: Vec<CosmeticItem>,
}

// Player with cosmetics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub cosmetics: CosmeticInventory,
    pub equipped_hat: Option<CosmeticItem>,
    pub equipped_aura: Option<CosmeticItem>,
    pub equipped_trail: Option<CosmeticItem>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            cosmetics: CosmeticInventory { items: Vec::new() },
            equipped_hat: None,
            equipped_aura: None,
            equipped_trail: None,
        }
    }

    pub fn add_cosmetic(&mut self, item: CosmeticItem) {
        self.cosmetics.items.push(item);
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

// Cosmetic functions
pub fn equip_cosmetic(player: &mut Player, category: CosmeticCategory, index: usize) {
    // Unequip current
    match category {
        CosmeticCategory::Hat => player.equipped_hat = None,
        CosmeticCategory::Aura => player.equipped_aura = None,
        CosmeticCategory::Trail => player.equipped_trail = None,
    }

    // Equip new
    if let Some(cosmetic) = player.cosmetics.items.get_mut(index) {
        if cosmetic.category == category {
            cosmetic.equipped = true;
            match category {
                CosmeticCategory::Hat => player.equipped_hat = Some(cosmetic.clone()),
                CosmeticCategory::Aura => player.equipped_aura = Some(cosmetic.clone()),
                CosmeticCategory::Trail => player.equipped_trail = Some(cosmetic.clone()),
            }
        }
    }
}

pub fn unequip_cosmetic(player: &mut Player, category: CosmeticCategory) {
    match category {
        CosmeticCategory::Hat => {
            if let Some(hat) = player.equipped_hat.take() {
                if let Some(item) = player.cosmetics.items.iter_mut().find(|c| c.id == hat.id) {
                    item.equipped = false;
                }
            }
        }
        CosmeticCategory::Aura => {
            if let Some(aura) = player.equipped_aura.take() {
                if let Some(item) = player.cosmetics.items.iter_mut().find(|c| c.id == aura.id) {
                    item.equipped = false;
                }
            }
        }
        CosmeticCategory::Trail => {
            if let Some(trail) = player.equipped_trail.take() {
                if let Some(item) = player.cosmetics.items.iter_mut().find(|c| c.id == trail.id) {
                    item.equipped = false;
                }
            }
        }
    }
}

// Visual rendering helpers
pub fn render_hat() -> String {
    "• Hat Visual".to_string()
}

pub fn render_aura() -> String {
    "☼ Aura Glow".to_string()
}

pub fn render_trail() -> String {
    "~~ Trail Effect".to_string()
}

// Purchase functions
pub fn can_purchase_gold(item: &CosmeticItem, player_gold: u64) -> bool {
    !item.purchased && item.cost_gold > 0 && player_gold >= item.cost_gold
}

pub fn can_purchase_usdt(item: &CosmeticItem, player_usdt: f64) -> bool {
    !item.purchased && item.cost_usdt > 0.0 && player_usdt >= item.cost_usdt
}

pub fn purchase_with_gold(item: &mut CosmeticItem, player_gold: &mut u64) -> bool {
    if can_purchase_gold(item, *player_gold) {
        item.purchased = true;
        *player_gold -= item.cost_gold;
        true
    } else {
        false
    }
}

pub fn purchase_with_usdt(item: &mut CosmeticItem, player_usdt: &mut f64) -> bool {
    if can_purchase_usdt(item, *player_usdt) {
        item.purchased = true;
        *player_usdt -= item.cost_usdt;
        true
    } else {
        false
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_new() {
        let player = Player::new();
        assert!(player.cosmetics.items.is_empty());
        assert!(player.equipped_hat.is_none());
        assert!(player.equipped_aura.is_none());
        assert!(player.equipped_trail.is_none());
    }

    #[test]
    fn test_add_cosmetic() {
        let mut player = Player::new();
        let hat = CosmeticItem {
            id: 1,
            name: "Test Hat".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Basic,
            cost_gold: 200,
            cost_usdt: 0.0,
            purchased: false,
            equipped: false,
        };
        player.add_cosmetic(hat);
        assert_eq!(player.cosmetics.items.len(), 1);
    }

    #[test]
    fn test_equip_cosmetic() {
        let mut player = Player::new();
        let hat = CosmeticItem {
            id: 1,
            name: "Test Hat".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Basic,
            cost_gold: 200,
            cost_usdt: 0.0,
            purchased: true,
            equipped: false,
        };
        player.add_cosmetic(hat);
        equip_cosmetic(&mut player, CosmeticCategory::Hat, 0);
        assert!(player.equipped_hat.is_some());
        assert!(player.equipped_hat.as_ref().unwrap().equipped);
    }

    #[test]
    fn test_unequip_cosmetic() {
        let mut player = Player::new();
        let hat = CosmeticItem {
            id: 1,
            name: "Test Hat".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Basic,
            cost_gold: 200,
            cost_usdt: 0.0,
            purchased: true,
            equipped: true,
        };
        player.add_cosmetic(hat);
        unequip_cosmetic(&mut player, CosmeticCategory::Hat);
        assert!(player.equipped_hat.is_none());
    }

    #[test]
    fn test_purchase_with_gold() {
        let mut player = Player::new();
        let mut hat = CosmeticItem {
            id: 1,
            name: "Test Hat".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Basic,
            cost_gold: 200,
            cost_usdt: 0.0,
            purchased: false,
            equipped: false,
        };
        let mut player_gold = 300;
        player.add_cosmetic(hat);
        player.cosmetics.items[0].purchased = false;
        assert!(purchase_with_gold(&mut player.cosmetics.items[0], &mut player_gold));
        assert!(player.cosmetics.items[0].purchased);
        assert_eq!(player_gold, 100);
    }

    #[test]
    fn test_purchase_with_usdt() {
        let mut player = Player::new();
        let mut hat = CosmeticItem {
            id: 2,
            name: "Premium Hat".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Premium,
            cost_gold: 0,
            cost_usdt: 1.0,
            purchased: false,
            equipped: false,
        };
        let mut player_usdt = 2.0;
        player.add_cosmetic(hat);
        player.cosmetics.items[0].purchased = false;
        assert!(purchase_with_usdt(&mut player.cosmetics.items[0], &mut player_usdt));
        assert!(player.cosmetics.items[0].purchased);
        assert!((player_usdt - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_render_hats() {
        let hat = render_hat();
        assert_eq!(hat, "• Hat Visual");
    }

    #[test]
    fn test_render_aura() {
        let aura = render_aura();
        assert_eq!(aura, "☼ Aura Glow");
    }

    #[test]
    fn test_render_trail() {
        let trail = render_trail();
        assert_eq!(trail, "~~ Trail Effect");
    }

    #[test]
    fn test_can_purchase_gold() {
        let item = CosmeticItem {
            id: 1,
            name: "Test".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Basic,
            cost_gold: 200,
            cost_usdt: 0.0,
            purchased: false,
            equipped: false,
        };
        assert!(can_purchase_gold(&item, 300));
        assert!(!can_purchase_gold(&item, 100));
        assert!(!can_purchase_gold(&item, 200)); // Exact amount
    }

    #[test]
    fn test_can_purchase_usdt() {
        let item = CosmeticItem {
            id: 2,
            name: "Test".to_string(),
            category: CosmeticCategory::Hat,
            cosmetic_type: CosmeticType::Premium,
            cost_gold: 0,
            cost_usdt: 1.0,
            purchased: false,
            equipped: false,
        };
        assert!(can_purchase_usdt(&item, 2.0));
        assert!(!can_purchase_usdt(&item, 0.5));
    }
}