//! Marketplace program — template listings, purchases and platform fees.
//!
//! Maps 1:1 to the Solidity TemplateMarket.sol functionality (Spec 012):
//! - publish_listing: publish a template (min 0.01 USDT, 50 USDT publishing fee)
//! - purchase_listing: buy an unsold listing (5% platform fee)
//! - withdraw_listing: seller withdraws the proceeds of a sold listing
//! - get_listing: view a listing by ID
//! - cleanup_expired: mark unsold listings older than 30 days as sold

use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;

use crate::token_utils::{transfer_usdt, transfer_usdt_with_signer};
use crate::{InitMarketplace, PublishListing, PurchaseListing, WithdrawListing, GetListing, CleanupExpired};

/// Minimum listing price: 0.01 USDT (6 decimals).
pub const MIN_PRICE_USDT: u64 = 10_000;
/// Publishing fee: 50 USDT.
pub const PUBLISHING_FEE_USDT: u64 = 50_000_000;
/// Platform fee: 5% of the price.
pub const PLATFORM_FEE_BPS: u64 = 500;
/// Listings expire after 30 days.
pub const EXPIRE_SECONDS: i64 = 30 * 24 * 3600;
/// Maximum listings kept in the marketplace account (Vec realloc budget).
pub const MAX_LISTINGS: usize = 100;

/// Marketplace state: PDA seeds `[b"marketplace"]`.
#[account]
pub struct MarketplaceAccount {
    pub program_id: Pubkey,
    pub market_authority: Pubkey,
    pub listings: Vec<Listing>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct Listing {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub title: String,
    pub github_url: String,
    pub description: String,
    pub price_usdt: u64,
    pub sold: bool,
    pub buyer: Pubkey,
    pub published_at: i64,
}

/// Listing size with 64-char title, 96-char URL and 128-char description.
pub const LISTING_SPACE: usize = 8 + 32 + 4 + 64 + 4 + 96 + 4 + 128 + 8 + 1 + 32 + 8;

#[error_code]
pub enum MarketplaceError {
    #[msg("Listing id already used")]
    DuplicateListing,
    #[msg("Price must be at least 0.01 USDT")]
    PriceTooLow,
    #[msg("Address already has a listing")]
    AlreadyListed,
    #[msg("Listing not found")]
    ListingNotFound,
    #[msg("Listing already sold")]
    AlreadySold,
    #[msg("Cannot buy your own listing")]
    CannotBuyOwnListing,
    #[msg("Only the seller can withdraw")]
    NotSeller,
    #[msg("Insufficient token balance")]
    InsufficientBalance,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Token account mint mismatch")]
    MintMismatch,
}

/// Check whether an address appears as seller or buyer of a listing.
pub fn is_address_listed(account: &MarketplaceAccount, addr: &Pubkey) -> bool {
    account
        .listings
        .iter()
        .any(|l| l.seller == *addr || l.buyer == *addr)
}

/// Check whether a listing id is already in use.
pub fn is_duplicate_listings(account: &MarketplaceAccount, listing_id: u64) -> bool {
    account
        .listings
        .iter()
        .any(|l| l.listing_id == listing_id)
}

// ─── Events ──────────────────────────────────────────────────────────
#[event]
pub struct ListingCreated {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub title: String,
    pub price_usdt: u64,
}

#[event]
pub struct ListingSold {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub price_usdt: u64,
}

// ─── Instructions ────────────────────────────────────────────────────
/// Initialize the marketplace as a PDA: seeds `[b"marketplace"]`.
pub fn init_marketplace(ctx: Context<InitMarketplace>) -> Result<()> {
    let account = &mut ctx.accounts.marketplace;
    account.program_id = *ctx.program_id;
    account.market_authority = ctx.accounts.authority.key();
    account.listings = Vec::new();
    Ok(())
}

/// Publish a template listing.
///
/// Requires: unique listing id, price >= 0.01 USDT, caller not already
/// listed, and a 50 USDT publishing fee (caller ATA -> marketplace ATA).
pub fn publish_listing(
    ctx: Context<PublishListing>,
    listing_id: u64,
    title: String,
    github_url: String,
    description: String,
    price_usdt: u64,
) -> Result<()> {
    let account = &mut ctx.accounts.marketplace;

    require!(
        !is_duplicate_listings(account, listing_id),
        MarketplaceError::DuplicateListing
    );
    require!(price_usdt >= MIN_PRICE_USDT, MarketplaceError::PriceTooLow);
    require!(
        !is_address_listed(account, &ctx.accounts.publisher.key()),
        MarketplaceError::AlreadyListed
    );

    let publisher_balance = crate::token_utils::get_balance(&ctx.accounts.publisher_ata.to_account_info())?;
    require!(
        publisher_balance >= PUBLISHING_FEE_USDT,
        MarketplaceError::InsufficientBalance
    );

    // Publishing fee: seller ATA -> marketplace ATA (marketplace PDA authority).
    transfer_usdt(
        &ctx.accounts.token_program,
        ctx.accounts.publisher_ata.to_account_info(),
        ctx.accounts.marketplace_ata.to_account_info(),
        ctx.accounts.publisher.to_account_info(),
        PUBLISHING_FEE_USDT,
    )?;

    let now = Clock::get()?.unix_timestamp;
    account.listings.push(Listing {
        listing_id,
        seller: ctx.accounts.publisher.key(),
        title: title.clone(),
        github_url: github_url.clone(),
        description: description.clone(),
        price_usdt,
        sold: false,
        buyer: Pubkey::default(),
        published_at: now,
    });

    emit!(ListingCreated {
        listing_id,
        seller: ctx.accounts.publisher.key(),
        title,
        price_usdt,
    });

    Ok(())
}

/// Purchase an unsold listing. The buyer pays the full price to the
/// marketplace ATA; 5% goes to the platform fee wallet, the rest to the
/// seller (both from the marketplace ATA, signed by the marketplace PDA).
pub fn purchase_listing(ctx: Context<PurchaseListing>, listing_id: u64) -> Result<()> {
    let listing_idx = ctx
        .accounts
        .marketplace
        .listings
        .iter()
        .position(|l| l.listing_id == listing_id)
        .ok_or(MarketplaceError::ListingNotFound)?;

    let listing = &ctx.accounts.marketplace.listings[listing_idx];

    require!(!listing.sold, MarketplaceError::AlreadySold);
    require!(
        ctx.accounts.buyer.key() != listing.seller,
        MarketplaceError::CannotBuyOwnListing
    );

    let price = listing.price_usdt;
    let seller = listing.seller;
    let buyer_balance =
        crate::token_utils::get_balance(&ctx.accounts.buyer_ata.to_account_info())?;
    require!(buyer_balance >= price, MarketplaceError::InsufficientBalance);

    // Buyer pays the full price into the marketplace ATA.
    transfer_usdt(
        &ctx.accounts.token_program,
        ctx.accounts.buyer_ata.to_account_info(),
        ctx.accounts.marketplace_ata.to_account_info(),
        ctx.accounts.buyer.to_account_info(),
        price,
    )?;

    // Split: platform fee and seller amount, signed by the marketplace PDA.
    let fee = price
        .checked_mul(PLATFORM_FEE_BPS)
        .ok_or(MarketplaceError::Overflow)?
        / 10_000;
    let seller_amount = price.saturating_sub(fee);

    let seeds: &[&[u8]] = &[b"marketplace", &[ctx.bumps.marketplace]];
    let marketplace_info = ctx.accounts.marketplace.to_account_info();
    if fee > 0 {
        transfer_usdt_with_signer(
            &ctx.accounts.token_program,
            ctx.accounts.marketplace_ata.to_account_info(),
            ctx.accounts.platform_fee_ata.to_account_info(),
            marketplace_info.clone(),
            &[seeds],
            fee,
        )?;
    }
    if seller_amount > 0 {
        transfer_usdt_with_signer(
            &ctx.accounts.token_program,
            ctx.accounts.marketplace_ata.to_account_info(),
            ctx.accounts.seller_ata.to_account_info(),
            marketplace_info,
            &[seeds],
            seller_amount,
        )?;
    }

    let account = &mut ctx.accounts.marketplace;
    account.listings[listing_idx].sold = true;
    account.listings[listing_idx].buyer = ctx.accounts.buyer.key();

    emit!(ListingSold {
        listing_id,
        seller,
        buyer: ctx.accounts.buyer.key(),
        price_usdt: price,
    });

    Ok(())
}

/// Seller withdraws the proceeds of a sold listing; the listing becomes
/// unsold again and can be re-sold.
pub fn withdraw_listing(ctx: Context<WithdrawListing>, listing_id: u64) -> Result<()> {
    let listing_idx = ctx
        .accounts
        .marketplace
        .listings
        .iter()
        .position(|l| l.listing_id == listing_id)
        .ok_or(MarketplaceError::ListingNotFound)?;

    let listing = &ctx.accounts.marketplace.listings[listing_idx];

    require!(listing.sold, MarketplaceError::AlreadySold);
    require!(
        ctx.accounts.seller.key() == listing.seller,
        MarketplaceError::NotSeller
    );

    let price = listing.price_usdt;
    let fee = price
        .checked_mul(PLATFORM_FEE_BPS)
        .ok_or(MarketplaceError::Overflow)?
        / 10_000;
    let amount = price.saturating_sub(fee);

    let seeds: &[&[u8]] = &[b"marketplace", &[ctx.bumps.marketplace]];
    let marketplace_info = ctx.accounts.marketplace.to_account_info();
    transfer_usdt_with_signer(
        &ctx.accounts.token_program,
        ctx.accounts.marketplace_ata.to_account_info(),
        ctx.accounts.seller_ata.to_account_info(),
        marketplace_info,
        &[seeds],
        amount,
    )?;

    let account = &mut ctx.accounts.marketplace;
    account.listings[listing_idx].sold = false;
    account.listings[listing_idx].buyer = Pubkey::default();

    Ok(())
}

/// View a listing by id (returns an error if it does not exist).
pub fn get_listing(ctx: Context<GetListing>, listing_id: u64) -> Result<()> {
    require!(
        ctx.accounts
            .marketplace
            .listings
            .iter()
            .any(|l| l.listing_id == listing_id),
        MarketplaceError::ListingNotFound
    );
    Ok(())
}

/// Mark unsold listings older than 30 days as sold (expired).
pub fn cleanup_expired(ctx: Context<CleanupExpired>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let account = &mut ctx.accounts.marketplace;
    for listing in account.listings.iter_mut() {
        if !listing.sold && now.saturating_sub(listing.published_at) > EXPIRE_SECONDS {
            listing.sold = true;
        }
    }
    Ok(())
}

// ─── Instruction accounts ────────────────────────────────────────────
// The `#[derive(Accounts)]` structs live at the crate root (lib.rs) — anchor
// 0.30 generates client-account modules named after the first segment of the
// `Context<...>` type, which only resolves for crate-root structs.

/// Helper to compute the marketplace PDA authority's USDT ATA.
pub fn marketplace_ata_address(marketplace: &Pubkey, mint: &Pubkey) -> Pubkey {
    get_associated_token_address(marketplace, mint)
}