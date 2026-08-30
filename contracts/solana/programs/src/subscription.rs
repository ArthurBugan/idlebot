//! Subscription program — premium template limits on Solana.
//!
//! Maps 1:1 to the Solidity Subscription.sol (Spec 012):
//! - purchase_subscription: pay 1 USDT for 30 days of premium (500 templates)
//! - refund_subscription: refund the price if still active
//! - cancel_subscription: deactivate an expired subscription
//! - withdraw_subscription: refund the remaining time of an active subscription

use anchor_lang::prelude::*;

use crate::token_utils::{transfer_usdt, transfer_usdt_with_signer};
use crate::{InitSubscription, PurchaseSubscription, RefundSubscription, CancelSubscription, WithdrawSubscription};

/// Premium length: 30 days.
pub const MONTH_SECONDS: i64 = 30 * 24 * 3600;
/// Premium price: 1 USDT (6 decimals).
pub const PREMIUM_PRICE_USDT: u64 = 1_000_000;
/// Free tier template limit.
pub const FREE_LIMIT: u32 = 50;
/// Premium tier template limit.
pub const PREMIUM_LIMIT: u32 = 500;

/// Subscription state: PDA seeds `[b"subscription", owner]`.
#[account]
pub struct SubscriptionAccount {
    pub program_id: Pubkey,
    pub owner: Pubkey,
    /// Unix timestamp until which the subscription is premium.
    pub premium_until: i64,
    pub limit: u32,
    /// Block timestamp of the last subscription transaction.
    pub last_transaction: i64,
    pub padding: [u8; 56],
}

#[error_code]
pub enum SubscriptionError {
    #[msg("Subscription is not active")]
    NotActive,
    #[msg("Subscription is already active")]
    AlreadyActive,
    #[msg("Insufficient token balance")]
    InsufficientBalance,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("User does not match the subscription owner")]
    InvalidUser,
}

// ─── Events ──────────────────────────────────────────────────────────
#[event]
pub struct SubscriptionPurchased {
    pub user: Pubkey,
    pub premium_until: i64,
}

#[event]
pub struct SubscriptionRefunded {
    pub user: Pubkey,
}

// ─── Instructions ────────────────────────────────────────────────────
/// Initialize a user's subscription account.
pub fn init_subscription(ctx: Context<InitSubscription>) -> Result<()> {
    let account = &mut ctx.accounts.subscription;
    account.program_id = *ctx.program_id;
    account.owner = ctx.accounts.authority.key();
    account.premium_until = 0;
    account.limit = FREE_LIMIT;
    account.last_transaction = Clock::get()?.unix_timestamp;
    Ok(())
}

/// Purchase premium: 1 USDT for 30 days (extends an active subscription).
pub fn purchase_subscription(
    ctx: Context<PurchaseSubscription>,
    _payment_token: Pubkey,
    user: Pubkey,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let subscription = &mut ctx.accounts.subscription;

    require!(
        user == subscription.owner,
        SubscriptionError::InvalidUser
    );

    let payer_balance =
        crate::token_utils::get_balance(&ctx.accounts.payer_ata.to_account_info())?;
    require!(
        payer_balance >= PREMIUM_PRICE_USDT,
        SubscriptionError::InsufficientBalance
    );

    // 1 USDT from payer ATA -> subscription PDA's ATA.
    transfer_usdt(
        &ctx.accounts.token_program,
        ctx.accounts.payer_ata.to_account_info(),
        ctx.accounts.subscription_ata.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        PREMIUM_PRICE_USDT,
    )?;

    subscription.premium_until = subscription
        .premium_until
        .max(now)
        .checked_add(MONTH_SECONDS)
        .ok_or(SubscriptionError::Overflow)?;
    subscription.limit = PREMIUM_LIMIT;
    subscription.last_transaction = now;

    emit!(SubscriptionPurchased {
        user,
        premium_until: subscription.premium_until,
    });

    Ok(())
}

/// Refund the price while the subscription is still active.
pub fn refund_subscription(
    ctx: Context<RefundSubscription>,
    _payment_token: Pubkey,
    user: Pubkey,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    require!(
        ctx.accounts.subscription.premium_until > now,
        SubscriptionError::NotActive
    );

    // Refund 1 USDT from the subscription PDA's ATA back to the payer.
    let owner = ctx.accounts.subscription.owner;
    let seeds: &[&[u8]] = &[b"subscription", owner.as_ref(), &[ctx.bumps.subscription]];
    let subscription_info = ctx.accounts.subscription.to_account_info();
    transfer_usdt_with_signer(
        &ctx.accounts.token_program,
        ctx.accounts.subscription_ata.to_account_info(),
        ctx.accounts.payer_ata.to_account_info(),
        subscription_info,
        &[seeds],
        PREMIUM_PRICE_USDT,
    )?;

    let subscription = &mut ctx.accounts.subscription;
    subscription.premium_until = 0;
    subscription.limit = FREE_LIMIT;
    subscription.last_transaction = now;

    emit!(SubscriptionRefunded { user });
    Ok(())
}

/// Cancel an expired subscription (nothing is refunded — it already lapsed).
pub fn cancel_subscription(
    ctx: Context<CancelSubscription>,
    _payment_token: Pubkey,
    user: Pubkey,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let subscription = &mut ctx.accounts.subscription;

    require!(
        subscription.premium_until <= now,
        SubscriptionError::AlreadyActive
    );

    subscription.premium_until = 0;
    subscription.limit = FREE_LIMIT;
    subscription.last_transaction = now;

    emit!(SubscriptionRefunded { user });
    Ok(())
}

/// Withdraw an active subscription: refund the remaining premium time and
/// deactivate. The unexpired fraction of the 30-day window (computed from
/// `last_transaction`, set at purchase) is returned to the payer — Spec 012
/// ("refund remaining amount").
pub fn withdraw_subscription(
    ctx: Context<WithdrawSubscription>,
    _payment_token: Pubkey,
    user: Pubkey,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    require!(
        ctx.accounts.subscription.premium_until > now,
        SubscriptionError::NotActive
    );

    let sub = &ctx.accounts.subscription;
    let remaining_secs = (sub.premium_until - now).max(0) as u128;
    let refund =
        (PREMIUM_PRICE_USDT as u128 * remaining_secs / MONTH_SECONDS as u128) as u64;

    let owner = sub.owner;
    let seeds: &[&[u8]] = &[b"subscription", owner.as_ref(), &[ctx.bumps.subscription]];
    let subscription_info = sub.to_account_info();
    if refund > 0 {
        transfer_usdt_with_signer(
            &ctx.accounts.token_program,
            ctx.accounts.subscription_ata.to_account_info(),
            ctx.accounts.payer_ata.to_account_info(),
            subscription_info,
            &[seeds],
            refund,
        )?;
    }

    let subscription = &mut ctx.accounts.subscription;
    subscription.premium_until = 0;
    subscription.limit = FREE_LIMIT;
    subscription.last_transaction = now;

    emit!(SubscriptionRefunded { user });
    Ok(())
}

// ─── Instruction accounts ────────────────────────────────────────────
// The `#[derive(Accounts)]` structs live at the crate root (lib.rs) — anchor
// 0.30 generates client-account modules named after the first segment of the
// `Context<...>` type, which only resolves for crate-root structs.