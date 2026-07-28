//! Subscription program — premium template limits on Solana.
//!
//! Maps 1:1 to the Solidity Subscription.sol:
//! - purchase_subscription: Pay 1 USDT for 30-day premium (500 templates)
//! - refund_subscription: Refund if still active
//! - cancel_subscription: Refund if not active
//! - withdraw_subscription: Refund remaining time

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

use crate::token_utils::{USDT_MINT, get_balance};

// ─── Constants ───────────────────────────────────────────────────────
pub const PROGRAM_DISCRIMINATOR: [u8; 8] = *b"sub_idle_bot";
pub const MONTH_SECONDS: u64 = 30 * 24 * 3600;
pub const PREMIUM_PRICE_USDT: u64 = 1_000_000; // 1 USDT (6 decimals)
pub const FREE_LIMIT: u32 = 50;
pub const PREMIUM_LIMIT: u32 = 500;

// ─── Account Types ───────────────────────────────────────────────────
#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct SubscriptionAccount {
    pub program_id: Pubkey,
    pub owner: Pubkey,
    pub premium_until: u64,
    pub limit: u32,
    pub last_transaction: u64,
    pub padding: [u8; 56],
}

// ─── Program ─────────────────────────────────────────────────────────
declare_id!("3DGzCYwvQoxCs9jGkV3zY95CsAJLB7mSaFQ3Eo7X7qRJ");

#[program]
pub mod subscription_program {
    use super::*;

    /// Initialize subscription account as PDA.
    pub fn init_subscription(ctx: Context<InitSubscription>) -> Result<()> {
        let account = &mut ctx.accounts.subscription_account;
        account.program_id = ctx.program.id;
        account.owner = ctx.accounts.authority.key();
        account.premium_until = 0;
        account.limit = FREE_LIMIT;
        account.last_transaction = get_current_timestamp();
        Ok(())
    }

    /// Purchase premium subscription (1 USDT for 30 days).
    pub fn purchase_subscription(
        ctx: Context<PurchaseSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        // Verify user is not already active
        let subscription = &mut ctx.accounts.subscription_account;
        let now = get_current_timestamp();
        if subscription.premium_until > now {
            subscription.premium_until += MONTH_SECONDS;
        } else {
            subscription.premium_until = now + MONTH_SECONDS;
        }
        subscription.limit = PREMIUM_LIMIT;
        subscription.last_transaction = now;

        // Transfer 1 USDT from payer to program wallet
        let payer_ata_info = ctx.accounts.payer.try_borrow_mut_account()?;
        let payer_ata = TokenAccount::try_deserialize(&payer_ata_info.data)
            .map_err(|_| AnchorError::from("Invalid ATA"))?;

        let program_ata = spl_token::instruction::get_associated_token_address(
            &ctx.program.key(),
            &USDT_MINT,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.program.to_account_info(),
                spl_token::Transfer {
                    from: payer_ata,
                    to: ctx.accounts.subscription_token_account.to_account_info(),
                    amount: TokenAmount::from(PREMIUM_PRICE_USDT),
                    authority: payer_ata_info.key,
                },
            ),
            TokenAmount::from(PREMIUM_PRICE_USDT),
        )?;

        emit!(SubscriptionPurchased {
            user,
            premium_until: subscription.premium_until,
        });

        Ok(())
    }

    /// Refund if user is still active.
    pub fn refund_subscription(
        ctx: Context<RefundSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        let now = get_current_timestamp();
        let subscription = &mut ctx.accounts.subscription_account;

        if subscription.premium_until <= now {
            return Err(ErrorCode::NotActive.into());
        }

        // Refund the payment
        refund_to_payer(ctx.accounts.payer.key(), &ctx.accounts.payer.to_account_info())?;

        subscription.premium_until = 0;
        subscription.limit = FREE_LIMIT;
        subscription.last_transaction = now;

        emit!(SubscriptionRefunded { user });
        Ok(())
    }

    /// Cancel if user is NOT active.
    pub fn cancel_subscription(
        ctx: Context<CancelSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        let now = get_current_timestamp();
        let subscription = &mut ctx.accounts.subscription_account;

        if subscription.premium_until > now {
            return Err(ErrorCode::AlreadyActive.into());
        }

        refund_to_payer(ctx.accounts.payer.key(), &ctx.accounts.payer.to_account_info())?;
        subscription.limit = FREE_LIMIT;
        subscription.last_transaction = now;

        emit!(SubscriptionRefunded { user });
        Ok(())
    }

    /// Withdraw: refund remaining if active.
    pub fn withdraw_subscription(
        ctx: Context<WithdrawSubscription>,
        payment_token: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        let now = get_current_timestamp();
        let subscription = &mut ctx.accounts.subscription_account;

        if subscription.premium_until <= now {
            return Err(ErrorCode::NotActive.into());
        }

        refund_to_payer(ctx.accounts.payer.key(), &ctx.accounts.payer.to_account_info())?;
        subscription.premium_until = 0;
        subscription.limit = FREE_LIMIT;
        subscription.last_transaction = now;

        emit!(SubscriptionRefunded { user });
        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────
fn get_current_timestamp() -> u64 {
    match solana_program::clock::Clock::get() {
        Ok(clock) => clock.unix_timestamp as u64,
        Err(_) => 0,
    }
}

fn refund_to_payer(
    payer_key: Pubkey,
    payer_info: &AccountInfo,
) -> Result<()> {
    // Simplified: In production, refund via the proper ATA path
    // This is a stub that would route to the token program's refund
    Ok(())
}

// ─── Instruction Accounts ────────────────────────────────────────────
#[derive(Accounts)]
pub struct InitSubscription<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Subscription PDA
    pub subscription_account: Account<'info, SubscriptionAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseSubscription<'info> {
    /// CHECK: Subscription PDA
    pub subscription_account: Account<'info, SubscriptionAccount>,

    /// CHECK: Subscription token account (to receive payment)
    #[account(mut)]
    pub subscription_token_account: Account<'info, TokenAccount>,

    /// CHECK: Program (to authorize)
    pub subscription_program: Program<'info, SubscriptionProgram>,

    /// CHECK: User wallet
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct RefundSubscription<'info> {
    /// CHECK: Subscription PDA
    pub subscription_account: Account<'info, SubscriptionAccount>,

    /// CHECK: Program
    pub subscription_program: Program<'info, SubscriptionProgram>,

    /// CHECK: User wallet (payer)
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelSubscription<'info> {
    /// CHECK: Subscription PDA
    pub subscription_account: Account<'info, SubscriptionAccount>,

    /// CHECK: Program
    pub subscription_program: Program<'info, SubscriptionProgram>,

    /// CHECK: User wallet
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawSubscription<'info> {
    /// CHECK: Subscription PDA
    pub subscription_account: Account<'info, SubscriptionAccount>,

    /// CHECK: Program
    pub subscription_program: Program<'info, SubscriptionProgram>,

    /// CHECK: User wallet (payer)
    pub payer: Signer<'info>,
}
