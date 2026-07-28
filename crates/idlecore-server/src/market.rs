//! Sistema de mercado - publish e compra de templates

use super::types::MarketListingDbEntry;
use spacetimedb::{ReducerContext, Table};
use crate::types::{market_listing, player};
use std::time::{SystemTime, UNIX_EPOCH};

/// Publishar template no market
pub fn publish_template(
    ctx: &ReducerContext,
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
    let player = ctx.db.player().iter()
        .find(|p| p.address == wallet_address)
        .expect("Player not found");

    if player.gold < 50 {
        tracing::warn!("Not enough gold to publish template");
        return;
    }

    // Deduzir gold
    let mut player = player;
    player.gold = player.gold.saturating_sub(50);
    ctx.db.player().address().update(player);

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

    ctx.db.market_listing().insert(listing);

    tracing::info!("Template published: {}", listing_id);
}

/// Completar compra de template (chamado via blockchain event)
pub fn complete_purchase(
    ctx: &ReducerContext,
    _seller: &str,
    buyer: &str,
    listing_id: u64,
    price_usdt: f64,
) {
    let listing = ctx.db.market_listing().iter()
        .find(|l| l.listing_id == listing_id)
        .expect("Listing not found");

    if listing.sold {
        tracing::warn!("Listing already sold");
        return;
    }

    // Mark as sold
    let listing_id = listing.listing_id;
    let title = listing.title.clone();
    let github_url = listing.github_url.clone();
    let seller_addr = listing.seller.clone();
    let mut listing = listing;
    listing.sold = true;
    ctx.db.market_listing().listing_id().update(listing);

    // Adicionar template ao inventário do comprador
    let buyer_player = ctx.db.player().iter()
        .find(|p| p.address == buyer)
        .expect("Buyer not found");

    let mut buyer_player = buyer_player;
    let template_json = format!(
        "{{\"id\":{},\"title\":\"{}\",\"url\":\"{}\",\"author\":\"{}\",\"price\":{}}}",
        listing_id, title, github_url, seller_addr, price_usdt
    );

    buyer_player.templates = if buyer_player.templates.is_empty() {
        template_json
    } else {
        format!("{},{}", buyer_player.templates, template_json)
    };

    ctx.db.player().address().update(buyer_player);

    tracing::info!("Template purchased: {} by {}", listing_id, buyer);
}

/// Cleanup listings antigos não vendidos (maior que 30 dias)
pub fn cleanup_old_listings(ctx: &ReducerContext) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let cutoff = now - (30 * 24 * 3600); // 30 dias

    // Buscar listings antigos e marcá-los como expirados
    for mut listing in ctx.db.market_listing().iter() {
        if !listing.sold && listing.published_at < cutoff {
            let id = listing.listing_id;
            listing.sold = true;
            ctx.db.market_listing().listing_id().update(listing);
            tracing::debug!("Expired old listing: {}", id);
        }
    }
}

/// Gerar ID único pra listing (simplificado)
fn generate_listing_id() -> u64 {
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
}
