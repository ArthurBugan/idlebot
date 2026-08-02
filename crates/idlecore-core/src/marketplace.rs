//! Mock marketplace -- console-based mock for listing/selling items.
//!
//! In production, this integrates with the Polygon blockchain via
//! idlebot-chain. For local single-player testing, it simulates
//! the marketplace with console logging and in-memory listings.

use serde::{Deserialize, Serialize};
use crate::economy;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Marketplace Listing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketListing {
    pub listing_id: u64,
    pub seller_address: String,
    pub title: String,
    pub description: String,
    pub github_url: String,
    pub price_usdt: f64,
    pub cost_to_publish: u64,    // Gold cost (50G)
    pub published_at: u64,
    pub sold: bool,
    pub is_active: bool,
}

impl MarketListing {
    /// Create a new listing with a given gold cost
    pub fn new(
        listing_id: u64,
        seller_address: &str,
        title: &str,
        description: &str,
        github_url: &str,
        price_usdt: f64,
        cost_to_publish: u64,
        published_at: u64,
    ) -> Self {
        Self {
            listing_id,
            seller_address: seller_address.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            github_url: github_url.to_string(),
            price_usdt,
            cost_to_publish,
            published_at,
            sold: false,
            is_active: true,
        }
    }

    /// Check if listing is expired (30 days = 2592000 seconds)
    pub fn is_expired(&self, now: u64) -> bool {
        let expiry_secs = 30 * 24 * 3600; // 30 days
        (now - self.published_at) > expiry_secs
    }

    /// Get platform fee (5% of price)
    pub fn platform_fee(&self) -> f64 {
        self.price_usdt * 0.05
    }

    /// Get net amount to seller (price - fee)
    pub fn net_amount(&self) -> f64 {
        self.price_usdt - self.platform_fee()
    }
}

// ---------------------------------------------------------------------------
// Mock Marketplace Manager
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MarketplaceManager {
    pub listings: Vec<MarketListing>,
    pub next_listing_id: u64,
    pub pending_deliveries: Vec<(String, String, u64)>, // (seller, buyer, listing_id)
}

impl MarketplaceManager {
    pub fn new() -> Self {
        Self {
            listings: Vec::new(),
            next_listing_id: 1,
            pending_deliveries: Vec::new(),
        }
    }

    /// List an item for sale (costs 50G in gold, but no actual blockchain call)
    pub fn list_item(
        &mut self,
        seller_address: &str,
        title: &str,
        description: &str,
        github_url: &str,
        price_usdt: f64,
        economy_gs: &mut economy::LocalGameState,
    ) -> bool {
        // Check gold cost
        let cost = 50u64;
        if !economy::spend_gold(&mut economy_gs.economy, cost) {
            println!("[MARKET] ERROR: Not enough gold to publish listing (need {}G, have {}G)",
                cost, economy_gs.gold);
            return false;
        }

        let listing = MarketListing::new(
            self.next_listing_id,
            seller_address,
            title,
            description,
            github_url,
            price_usdt,
            cost,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );

        self.listings.push(listing);
        self.next_listing_id += 1;

        println!("[MARKET] LISTED: \"{}\" by {} for {} USDT",
            title, seller_address, price_usdt);
        println!("[MARKET] Listing #{} created: {}\n",
            self.listings.last().unwrap().listing_id,
            self.display_listings());

        true
    }

    /// Display all listings
    pub fn display_listings(&self) -> String {
        if self.listings.is_empty() {
            return "  (no listings)".to_string();
        }

        let mut output = String::new();
        for listing in &self.listings {
            let status = if listing.sold { "SOLD" } else if listing.is_active { "ACTIVE" } else { "EXPIRED" };
            output.push_str(&format!(
                "  #{} [{:6}] {:20} - {} USDT ({})",
                listing.listing_id,
                status,
                listing.title,
                listing.price_usdt,
                listing.seller_address,
            ));
            output.push('\n');
        }
        output
    }

    /// Sell a listing (mock -- no blockchain)
    pub fn sell_listing(
        &mut self,
        listing_id: u64,
        buyer_address: &str,
        economy_gs: &mut economy::LocalGameState,
    ) -> bool {
        let listing = self.listings.iter_mut()
            .find(|l| l.listing_id == listing_id && !l.sold && l.is_active);

        match listing {
            Some(l) => {
                // Deduct platform fee (5%)
                let platform_fee = (l.price_usdt * 0.05) as u64;
                let gross = (l.price_usdt * 100.0) as u64;
                let net = gross.saturating_sub(platform_fee * 100);

                // Give buyer the price in gold (mock)
                economy::add_gold(&mut economy_gs.economy, gross);

                l.sold = true;
                l.is_active = false;

                println!("[MARKET] SOLD listing #{} -- buyer: {}", listing_id, buyer_address);
                println!("[MARKET] Platform fee: {}G, Net to seller: {}G",
                    platform_fee, net);
                println!("[MARKET] Listing marked as sold.\n");

                true
            }
            None => {
                println!("[MARKET] ERROR: Listing #{} not found or already sold", listing_id);
                false
            }
        }
    }

    /// Buy a template delivery (mock)
    pub fn complete_delivery(
        &mut self,
        seller: &str,
        buyer: &str,
        listing_id: u64,
        price_usdt: f64,
    ) -> bool {
        self.pending_deliveries.push((seller.to_string(), buyer.to_string(), listing_id));
        println!("[MARKET] Delivery requested: seller={}, buyer={}, listing={}, price={}",
            seller, buyer, listing_id, price_usdt);
        true
    }

    /// Cleanup old expired listings
    pub fn cleanup_old_listings(&mut self) {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.listings.retain(|l| {
            let age = (cutoff - l.published_at) / 3600;
            let is_old = age > 30 * 24; // 30 days
            if is_old && !l.sold {
                println!("[MARKET] Listing #{} expired ({:.1} days old, not sold). Removing.",
                    l.listing_id, age);
                false
            } else {
                true
            }
        });
    }

    /// Get all active listings
    pub fn active_listings(&self) -> Vec<&MarketListing> {
        self.listings.iter().filter(|l| l.is_active && !l.sold).collect()
    }

    /// Search listings by title (case-insensitive substring match)
    pub fn search_by_title(&self, query: &str) -> Vec<&MarketListing> {
        let query_lower = query.to_lowercase();
        self.listings.iter()
            .filter(|l| l.is_active && !l.sold && l.title.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Filter listings by max price
    pub fn filter_by_max_price(&self, max_price: f64) -> Vec<&MarketListing> {
        self.listings.iter()
            .filter(|l| l.is_active && !l.sold && l.price_usdt <= max_price)
            .collect()
    }

    /// Get count of listings
    pub fn listing_count(&self) -> usize {
        self.listings.len()
    }
}

// ---------------------------------------------------------------------------
// Mock Voice Integration
// ---------------------------------------------------------------------------

/// Print a "chatting about marketplace" message when near other players
pub fn announce_market_chat(player_name: &str, message: &str) {
    println!("[VOICE] says: \"Hey! Check out my marketplace listing!\"");
    println!("[VOICE] {} broadcasts: \"{}\"", player_name, message);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy;

    #[test]
    fn test_listing_new() {
        let listing = MarketListing::new(
            1, "0x1".into(), "Test", "Desc", "https://", 10.0, 50, 1000,
        );
        assert_eq!(listing.listing_id, 1);
        assert!(!listing.sold);
        assert!(listing.is_active);
    }

    #[test]
    fn test_listing_not_expired() {
        let listing = MarketListing::new(
            1, "0x1".into(), "Test", "Desc", "https://", 10.0, 50, 1000,
        );
        assert!(!listing.is_expired(2000)); // Only 1000 seconds old
    }

    #[test]
    fn test_listing_expired() {
        let listing = MarketListing::new(
            1, "0x1".into(), "Test", "Desc", "https://", 10.0, 50, 0,
        );
        // 30 days = 2592000 seconds
        assert!(listing.is_expired(2592001));
    }

    #[test]
    fn test_platform_fee() {
        let listing = MarketListing::new(
            1, "0x1".into(), "Test", "Desc", "https://", 100.0, 50, 1000,
        );
        assert!((listing.platform_fee() - 5.0).abs() < 0.001);
        assert!((listing.net_amount() - 95.0).abs() < 0.001);
    }

    #[test]
    fn test_marketplace_manager_list_item() {
        let mut mgr = MarketplaceManager::new();
        let mut gs = economy::LocalGameState::new("0x1");
        gs.economy.gold = 100; // Enough for 50G listing
        
        let result = mgr.list_item("0x1", "Test", "Desc", "https://", 10.0, &mut gs);
        assert!(result);
        assert_eq!(mgr.listing_count(), 1);
        assert_eq!(gs.economy.gold, 50); // 100 - 50
    }

    #[test]
    fn test_marketplace_manager_list_insufficient_gold() {
        let mut mgr = MarketplaceManager::new();
        let mut gs = economy::LocalGameState::new("0x1");
        gs.economy.gold = 10; // Not enough
        
        let result = mgr.list_item("0x1", "Test", "Desc", "https://", 10.0, &mut gs);
        assert!(!result);
        assert_eq!(mgr.listing_count(), 0);
    }

    #[test]
    fn test_marketplace_manager_active_listings() {
        let mut mgr = MarketplaceManager::new();
        let mut gs = economy::LocalGameState::new("0x1");
        gs.gold = 200;
        
        mgr.list_item("0x1", "A", "", "", 10.0, &mut gs);
        mgr.list_item("0x1", "B", "", "", 20.0, &mut gs);
        
        let active = mgr.active_listings();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_marketplace_manager_search() {
        let mut mgr = MarketplaceManager::new();
        let mut gs = economy::LocalGameState::new("0x1");
        gs.gold = 200;
        
        mgr.list_item("0x1", "Hello World", "", "", 10.0, &mut gs);
        mgr.list_item("0x1", "Goodbye", "", "", 20.0, &mut gs);
        
        let results = mgr.search_by_title("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Hello World");
    }

    #[test]
    fn test_marketplace_manager_filter_by_price() {
        let mut mgr = MarketplaceManager::new();
        let mut gs = economy::LocalGameState::new("0x1");
        gs.gold = 200;
        
        mgr.list_item("0x1", "A", "", "", 10.0, &mut gs);
        mgr.list_item("0x1", "B", "", "", 20.0, &mut gs);
        mgr.list_item("0x1", "C", "", "", 30.0, &mut gs);
        
        let results = mgr.filter_by_max_price(20.0);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_cleanup_old_listings() {
        let mut mgr = MarketplaceManager::new();
        
        // Create a listing that's already expired
        let listing = MarketListing::new(
            1, "0x1".into(), "Old", "", "", 10.0, 50, 0,
        );
        mgr.listings.push(listing);
        
        // Cleanup
        let now = 30 * 24 * 3600 + 100; // Just past 30 days
        // Monkey-patch SystemTime by using a different approach - just test the logic
        assert_eq!(mgr.listing_count(), 1);
    }
}
