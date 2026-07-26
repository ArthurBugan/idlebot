//! Sistema de mercado - publish e compra de templates

use super::types::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Publishar template no market
pub fn publish_template(
    wallet_address: &str,
    title: String,
    github_url: String,
    description: String,
    price_usdt: f64,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Verificar se player tem Gold suficiente pra publicar
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    if player.gold < 50 {
        tracing::warn!("Not enough gold to publish template");
        return;
    }
    
    // Deduzir gold
    deduct_gold(wallet_address, 50);
    
    // Criar listing (ID seria gerado de outra forma em produção)
    let listing_id = generate_listing_id();
    
    let listing = MarketListingDbEntry {
        listing_id,
        seller: wallet_address.to_string(),
        title,
        github_url,
        description,
        price_usdt,
        published_at: now,
        sold: false,
    };
    
    db::market_listing::table().insert(listing);
    
    tracing::info!("Template published: {}", listing_id);
}

/// Completar compra de template (chamado via blockchain event)
pub fn complete_purchase(
    seller: &str,
    buyer: &str,
    listing_id: u64,
    price_usdt: f64,
) {
    let listing: MarketListingDbEntry = db::market_listing::table()
        .get(listing_id)
        .expect("Listing not found");
    
    if listing.sold {
        tracing::warn!("Listing already sold");
        return;
    }
    
    // Mark as sold
    let mut listing = listing;
    listing.sold = true;
    db::market_listing::table().update(listing);
    
    // Adicionar template ao inventário do comprador
    let buyer: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == buyer)
        .first()
        .expect("Buyer not found");
    
    let mut buyer = buyer;
    let template_json = format!(
        "{{\"id\":{},\"title\":\"{}\",\"url\":\"{}\",\"author\":\"{}\",\"price\":{}}}",
        listing_id, listing.title, listing.github_url, listing.seller, price_usdt
    );
    
    buyer.templates = if buyer.templates.is_empty() {
        template_json
    } else {
        format!("{},{}", buyer.templates, template_json)
    };
    
    db::player::table().update(buyer);
    
    tracing::info!("Template purchased: {} by {}", listing_id, buyer);
}

/// Cleanup listings antigos não vendidos (maior que 30 dias)
pub fn cleanup_old_listings() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let cutoff = now - (30 * 24 * 3600); // 30 dias
    
    // Buscar listings antigos e marcá-los como expirados
    // (Em produção, usar query mais específica)
    tracing::trace!("Cleanup old listings");
}

/// Gerar ID único pra listing (simplificado)
fn generate_listing_id() -> u64 {
    // Em produção, usar auto-increment ou UUID
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
}

/// Deduzir gold
fn deduct_gold(wallet_address: &str, amount: u64) {
    let player: PlayerDbEntry = db::player::table()
        .filter(|p: &PlayerDbEntry| p.address == wallet_address)
        .first()
        .expect("Player not found");
    
    let mut player = player;
    player.gold = player.gold.saturating_sub(amount);
    db::player::table().update(player);
}
