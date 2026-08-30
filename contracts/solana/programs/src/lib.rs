//! IdleBot Anchor workspace — Solana programs for marketplace and subscriptions
//! (Spec 012), plus SPL token helpers.
//!
//! Marketplace and subscription instructions are implemented in their own
//! modules (matching the Solidity file layout they port) and dispatched
//! through thin wrappers here, so the crate builds as a single Anchor program:
//!
//! - marketplace: template listings with publishing fee, 5% platform fee,
//!   seller withdrawal and 30-day expiry
//! - subscription: 1 USDT / 30-day premium tier with refunds
//! - token_utils: USDT (SPL Token) transfer helpers
//!
//! Note (anchor 0.30): the `#[derive(Accounts)]` structs MUST live at the
//! crate root — the `#[program]` macro generates client-account modules named
//! after the first path segment of each `Context<...>` type, which only
//! resolves for crate-root structs.
//!
//! Build: `anchor build` (or `cargo build-sbf`) from `contracts/solana`.
//! Test: `anchor test` (needs the Solana toolchain + `anchor` CLI).

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use marketplace::{MarketplaceAccount, MAX_LISTINGS, LISTING_SPACE};
use subscription::SubscriptionAccount;

pub mod marketplace;
pub mod subscription;
pub mod token_utils;

declare_id!("j9194JViAkUpfSgZc3CMxGSS8ANCCAHqZAJvfhJzdaZ");

/// The IdleBot on-chain program: marketplace + subscription instructions.
#[program]
pub mod idlebot_program {
    use super::*;

    // ── Marketplace (TemplateMarket.sol port) ────────────────────────────
    pub fn init_marketplace(
        ctx: Context<InitMarketplace>,
    ) -> Result<()> {
        marketplace::init_marketplace(ctx)
    }

    pub fn publish_listing(
        ctx: Context<PublishListing>,
        listing_id: u64,
        title: String,
        github_url: String,
        description: String,
        price_usdt: u64,
    ) -> Result<()> {
        marketplace::publish_listing(ctx, listing_id, title, github_url, description, price_usdt)
    }

    pub fn purchase_listing(
        ctx: Context<PurchaseListing>,
        listing_id: u64,
    ) -> Result<()> {
        marketplace::purchase_listing(ctx, listing_id)
    }

    pub fn withdraw_listing(
        ctx: Context<WithdrawListing>,
        listing_id: u64,
    ) -> Result<()> {
        marketplace::withdraw_listing(ctx, listing_id)
    }

    pub fn get_listing(ctx: Context<GetListing>, listing_id: u64) -> Result<()> {
        marketplace::get_listing(ctx, listing_id)
    }

    pub fn cleanup_expired(ctx: Context<CleanupExpired>) -> Result<()> {
        marketplace::cleanup_expired(ctx)
    }

    // ── Subscription (Subscription.sol port) ─────────────────────────────
    pub fn init_subscription(
        ctx: Context<InitSubscription>,
    ) -> Result<()> {
        subscription::init_subscription(ctx)
    }

    pub fn purchase_subscription(
        ctx: Context<PurchaseSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        subscription::purchase_subscription(ctx, payment_token, user)
    }

    pub fn refund_subscription(
        ctx: Context<RefundSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        subscription::refund_subscription(ctx, payment_token, user)
    }

    pub fn cancel_subscription(
        ctx: Context<CancelSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        subscription::cancel_subscription(ctx, payment_token, user)
    }

    pub fn withdraw_subscription(
        ctx: Context<WithdrawSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        subscription::withdraw_subscription(ctx, payment_token, user)
    }
}

// ─── Instruction accounts (crate root — see module docs) ───────────────────

#[derive(Accounts)]
pub struct InitMarketplace<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 4 + MAX_LISTINGS * LISTING_SPACE,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, MarketplaceAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PublishListing<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, MarketplaceAccount>,
    #[account(mut)]
    pub publisher: Signer<'info>,
    /// Seller's USDT ATA (pays the publishing fee).
    #[account(mut)]
    pub publisher_ata: Account<'info, TokenAccount>,
    /// Marketplace PDA's USDT ATA (receives the fee).
    #[account(
        mut,
        constraint = marketplace_ata.owner == marketplace.key() @ marketplace::MarketplaceError::ListingNotFound
    )]
    pub marketplace_ata: Account<'info, TokenAccount>,
    /// USDT mint (must match the ATA mint).
    #[account(constraint = publisher_ata.mint == mint.key() @ marketplace::MarketplaceError::MintMismatch)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseListing<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, MarketplaceAccount>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    /// Buyer's USDT ATA (pays the full price).
    #[account(mut)]
    pub buyer_ata: Account<'info, TokenAccount>,
    /// Marketplace PDA's USDT ATA (holds proceeds).
    #[account(mut)]
    pub marketplace_ata: Account<'info, TokenAccount>,
    /// Platform fee wallet ATA (receives the 5% fee).
    #[account(mut)]
    pub platform_fee_ata: Account<'info, TokenAccount>,
    /// Seller's USDT ATA (receives the remainder).
    #[account(mut)]
    pub seller_ata: Account<'info, TokenAccount>,
    /// USDT mint (must match the ATAs).
    #[account(constraint = buyer_ata.mint == mint.key() @ marketplace::MarketplaceError::MintMismatch)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawListing<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, MarketplaceAccount>,
    #[account(mut)]
    pub seller: Signer<'info>,
    #[account(mut)]
    pub marketplace_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub seller_ata: Account<'info, TokenAccount>,
    #[account(constraint = seller_ata.mint == mint.key() @ marketplace::MarketplaceError::MintMismatch)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetListing<'info> {
    #[account(seeds = [b"marketplace"], bump)]
    pub marketplace: Account<'info, MarketplaceAccount>,
}

#[derive(Accounts)]
pub struct CleanupExpired<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, MarketplaceAccount>,
}

#[derive(Accounts)]
pub struct InitSubscription<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 8 + 4 + 8 + 56,
        seeds = [b"subscription", authority.key().as_ref()],
        bump
    )]
    pub subscription: Account<'info, SubscriptionAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", subscription.owner.as_ref()],
        bump
    )]
    pub subscription: Account<'info, SubscriptionAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// Payer's USDT ATA (pays the premium).
    #[account(mut)]
    pub payer_ata: Account<'info, TokenAccount>,
    /// Subscription PDA's USDT ATA (holds premium funds).
    #[account(mut)]
    pub subscription_ata: Account<'info, TokenAccount>,
    #[account(constraint = payer_ata.mint == mint.key() @ subscription::SubscriptionError::NotActive)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RefundSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", subscription.owner.as_ref()],
        bump
    )]
    pub subscription: Account<'info, SubscriptionAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub payer_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub subscription_ata: Account<'info, TokenAccount>,
    #[account(constraint = payer_ata.mint == mint.key() @ subscription::SubscriptionError::NotActive)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", subscription.owner.as_ref()],
        bump
    )]
    pub subscription: Account<'info, SubscriptionAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", subscription.owner.as_ref()],
        bump
    )]
    pub subscription: Account<'info, SubscriptionAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub payer_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub subscription_ata: Account<'info, TokenAccount>,
    #[account(constraint = payer_ata.mint == mint.key() @ subscription::SubscriptionError::NotActive)]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}