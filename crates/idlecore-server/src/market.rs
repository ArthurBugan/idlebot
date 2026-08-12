//! Marketplace (Spec 011) — publish (50G, 30-day expiry), renew (10G/7d),
//! buy with USDT + 5% platform fee + 48 h escrow, disputes.

use spacetimedb::{ReducerContext, Table};
use crate::economy::{add_usdt, spend_gold, spend_usdt};
use crate::types::{player, 
    market_listing, now_secs, MarketListing, DISPUTE_REFUND_PENALTY_PERMILLE, ESCROW_SECS,
    LISTING_DURATION_SECS, LISTING_GRACE_SECS, LISTING_PUBLISH_COST, LISTING_RENEWAL_COST,
    LISTING_RENEWAL_PERIOD, PLATFORM_FEE_PERMILLE,
};

fn valid_category(c: &str) -> bool {
    matches!(c, "Agent" | "Code" | "Template" | "Snippet")
}

fn permille_of(price: u64, permille: u64) -> u64 {
    (price as u128 * permille as u128 / 1000) as u64
}

/// Spec 011 FR1/FR2: publish a listing. Costs 50G.
pub fn publish(
    ctx: &ReducerContext,
    address: &str,
    title: String,
    description: String,
    github_url: String,
    price_usdt: u64,
    category: String,
) -> Result<u64, String> {
    if title.trim().is_empty() || !title.chars().next().is_some_and(|c| c.is_alphanumeric()) {
        return Err("Title must be non-empty".to_string());
    }
    if price_usdt == 0 {
        return Err("Price must be > 0 USDT".to_string());
    }
    if !github_url.starts_with("https://github.com/") {
        return Err("GitHub URL required (https://github.com/...)".to_string());
    }
    if !valid_category(&category) {
        return Err("Category must be Agent|Code|Template|Snippet".to_string());
    }

    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    spend_gold(ctx, &mut p, LISTING_PUBLISH_COST, "publish_listing")?;

    let now = now_secs(ctx);
    let listing_row = ctx.db.market_listing().insert(MarketListing {
        listing_id: 0,
        seller: p.address.clone(),
        title,
        description,
        github_url,
        price_usdt,
        category,
        published_at: now,
        expires_at: now + LISTING_DURATION_SECS,
        is_sold: false,
        buyer: None,
        escrow_until: 0,
        disputed: false,
    });

    p.templates_published = p.templates_published.saturating_add(1);
    ctx.db.player().address().update(p);
    let id = listing_row.listing_id;
    tracing::info!("LISTING: {} published #{id}", address);
    Ok(id)
}

/// Spec 011 FR5-FR7: buy a listing with USDT; 5% platform fee, seller paid
/// minus fee, then 48 h escrow before the seller's payout is considered final.
pub fn buy(ctx: &ReducerContext, buyer: &str, listing_id: u64) -> Result<(), String> {
    let Some(mut listing) = ctx
        .db
        .market_listing()
        .listing_id()
        .find(listing_id)
    else {
        return Err("Listing not found".to_string());
    };
    if listing.is_sold {
        return Err("Listing already sold".to_string());
    }
    if listing.seller == buyer {
        return Err("Cannot buy your own listing".to_string());
    }
    let now = now_secs(ctx);
    if listing.expires_at < now {
        return Err("Listing expired".to_string());
    }

    let mut b = crate::economy::find_player(ctx, &buyer.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    if b.usdt < listing.price_usdt {
        return Err("Insufficient USDT".to_string());
    }

    // Charge buyer, hold seller payout in escrow (PROPOSAL 4.2).
    spend_usdt(ctx, &mut b, listing.price_usdt, "buy_listing")?;
    let fee = permille_of(listing.price_usdt, PLATFORM_FEE_PERMILLE);
    let seller_payout = listing.price_usdt - fee;

    listing.is_sold = true;
    listing.buyer = Some(buyer.to_lowercase());
    listing.escrow_until = now + ESCROW_SECS;
    ctx.db.market_listing().listing_id().update(listing.clone());

    b.templates_purchased = b.templates_purchased.saturating_add(1);
    let mut templates: Vec<String> = serde_json::from_str(&b.templates).unwrap_or_default();
    templates.push(listing.github_url.clone());
    b.templates = serde_json::to_string(&templates).unwrap();
    ctx.db.player().address().update(b);

    tracing::info!(
        "SALE: #{listing_id} {buyer} paid {}u6 (fee {fee}) escrow until {}",
        listing.price_usdt,
        listing.escrow_until
    );
    let _ = seller_payout; // credited on escrow release
    Ok(())
}

/// Release the escrow to the seller (called after the 48 h window, or
/// explicitly). 5% platform fee already withheld at purchase time.
pub fn release_escrow(ctx: &ReducerContext, listing_id: u64) -> Result<(), String> {
    let Some(mut listing) = ctx
        .db
        .market_listing()
        .listing_id()
        .find(listing_id)
    else {
        return Err("Listing not found".to_string());
    };
    if !listing.is_sold || listing.escrow_until == 0 {
        return Err("Listing has no pending escrow".to_string());
    }
    let now = now_secs(ctx);
    if now < listing.escrow_until {
        return Err("Escrow window not over".to_string());
    }

    let fee = permille_of(listing.price_usdt, PLATFORM_FEE_PERMILLE);
    let payout = listing.price_usdt - fee;
    if let Some(mut s) = crate::economy::find_player(ctx, &listing.seller) {
        add_usdt(ctx, &mut s, payout, "escrow_release");
        ctx.db.player().address().update(s);
    }

    listing.escrow_until = 0;
    ctx.db.market_listing().listing_id().update(listing);
    tracing::info!("ESCROW: #{listing_id} released {payout}u6 to seller");
    Ok(())
}

/// Dispute: buyer gets a refund minus a 2% penalty, listing removed
/// (PROPOSAL 4.2 — automatic buyer-wins path).
pub fn dispute(ctx: &ReducerContext, buyer: &str, listing_id: u64) -> Result<(), String> {
    let Some(mut listing) = ctx
        .db
        .market_listing()
        .listing_id()
        .find(listing_id)
    else {
        return Err("Listing not found".to_string());
    };
    if listing.buyer.as_deref() != Some(buyer) {
        return Err("Only the buyer can dispute".to_string());
    }
    if !listing.is_sold || listing.escrow_until == 0 {
        return Err("Nothing to dispute".to_string());
    }

    let penalty = permille_of(listing.price_usdt, DISPUTE_REFUND_PENALTY_PERMILLE);
    let refund = listing.price_usdt - penalty;
    if let Some(mut b) = crate::economy::find_player(ctx, buyer) {
        add_usdt(ctx, &mut b, refund, "dispute_refund");
        ctx.db.player().address().update(b);
    }

    listing.escrow_until = 0;
    listing.disputed = true;
    ctx.db.market_listing().listing_id().update(listing);
    tracing::info!("DISPUTE: #{listing_id} refunded {refund}u6 minus 2% penalty");
    Ok(())
}

/// Spec 011 FR8/F renewal: extend expiry (auto-renewed by the scheduler or
/// manually) for 10G / 7 days (Ecosystem 2.4).
pub fn renew(ctx: &ReducerContext, seller: &str, listing_id: u64) -> Result<(), String> {
    let Some(mut listing) = ctx
        .db
        .market_listing()
        .listing_id()
        .find(listing_id)
    else {
        return Err("Listing not found".to_string());
    };
    if listing.seller != seller {
        return Err("Only the seller can renew".to_string());
    }
    if listing.is_sold {
        return Err("Sold listings cannot be renewed".to_string());
    }

    let mut p = crate::economy::find_player(ctx, &seller.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    spend_gold(ctx, &mut p, LISTING_RENEWAL_COST, "renew_listing")?;

    let now = now_secs(ctx);
    listing.expires_at = listing.expires_at.max(now) + LISTING_RENEWAL_PERIOD;
    ctx.db.market_listing().listing_id().update(listing);
    tracing::info!("RENEW: #{listing_id} renewed by {seller}");
    Ok(())
}

/// Scheduler (hourly): deactivate expired listings past the 24 h grace
/// period (Ecosystem 2.4); release matured escrows.
pub fn cleanup(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    let mut expired = 0u64;
    let mut released = 0u64;

    let listing_ids: Vec<u64> = ctx.db.market_listing().iter().map(|l| l.listing_id).collect();
    for id in listing_ids {
        let Some(mut l) = ctx.db.market_listing().listing_id().find(id) else {
            continue;
        };
        if !l.is_sold && l.expires_at + LISTING_GRACE_SECS < now {
            // Expired: remove (publish gold was already spent).
            ctx.db.market_listing().listing_id().delete(l.listing_id);
            expired += 1;
        } else if l.is_sold && l.escrow_until > 0 && l.escrow_until <= now && !l.disputed {
            drop(l);
            let _ = release_escrow(ctx, id);
            released += 1;
        }
    }
    if expired > 0 || released > 0 {
        tracing::info!("MARKET-TICK: {expired} expired, {released} escrow released");
    }
}