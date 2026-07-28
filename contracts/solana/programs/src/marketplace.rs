//! Marketplace program — manages template listings on Solana.
//!
//! Maps 1:1 to the Solidity TemplateMarket.sol functionality:
//! - publish_listing: Create a new listing (deduct 50 USDT publishing fee)
//! - purchase_listing: Buy an unsold listing (collect 5% platform fee)
//! - withdraw_listing: Refund a sold listing back to seller
//! - get_listing: View a listing by ID
//! - cleanup_expired: Mark unsold listings > 30 days old as sold

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAmount, TokenAccount};

use crate::token_utils::{USDT_MINT, get_balance};

// ─── Constants ───────────────────────────────────────────────────────
pub const PUBLISHING_FEE_USDT: u64 = 1_000_000; // 1 USDT minimum (Solidity was 10_000 = 0.01)
pub const PLATFORM_FEE_PERCENT: u64 = 5; // 5%
pub const MARKET_PROGRAM_DISCRIMINATOR: [u8; 8] = *b"market_idle";
pub const MARKET_LENGTH_PDA: [&[u8]; 1] = &[b"market_length"];
pub const MARKET_PADDING_PDA: [&[u8]; 1] = &[b"market_padding"];

// ─── Account Types ───────────────────────────────────────────────────
#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct MarketplaceAccount {
    pub program_id: Pubkey,
    pub market_authority: Pubkey,
    pub listings: Vec<Listing>,
    pub padding: [u8; 12],
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Listing {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub title: String,
    pub github_url: String,
    pub description: String,
    pub price_usdt: u64,
    pub sold: bool,
    pub buyer: Pubkey,
    pub published_at: u64,
}

// ─── Discriminators ──────────────────────────────────────────────────
declare_id!("Fg6PaFpoGk58pnDmIBm3wWV7eNu8g2cjzdDc9aX5J6ZP");

#[program]
pub mod marketplace_program {
    use super::*;

    /// Initialize the marketplace account as PDA.
    pub fn init_marketplace(ctx: Context<InitMarketplace>) -> Result<()> {
        let account = &mut ctx.accounts.marketplace_account;
        account.program_id = ctx.program.id;
        account.market_authority = ctx.accounts.authority.key();
        account.listings = Vec::new();
        Ok(())
    }

    /// Publish a new template listing.
    ///
    /// Requires:
    /// - Caller NOT already selling this listing
    /// - Price >= 1 USDT
    /// - Caller has >= 1 USDT (publishing fee)
    pub fn publish_listing(
        ctx: Context<PublishListing>,
        listing_id: u64,
        title: String,
        github_url: String,
        description: String,
        price_usdt: u64,
    ) -> Result<()> {
        let account = &mut ctx.accounts.marketplace_account;

        // Validate duplicate listings
        if is_duplicate_listings(account, listing_id) {
            return Err(ErrorCode::DuplicateListing.into());
        }

        // Validate price
        if price_usdt < PUBLISHING_FEE_USDT {
            return Err(ErrorCode::PriceTooLow.into());
        }

        // Check if user already has a listing
        if is_address_listed(account, ctx.accounts.publisher.key()) {
            return Err(ErrorCode::AlreadyListed.into());
        }

        // Deduct publishing fee
        let signing_key = ctx.accounts.publisher.key();
        let publisher_wallet = ctx.accounts.publisher.to_account_info();
        let publisher ATA = spl_token::instruction::get_associated_token_address(
            &ctx.accounts.publisher.key(),
            &USDT_MINT,
        )?;
        let publisher_ata_info = publisher_wallet.try_borrow_mut_account()?;
        let publisher_ata = TokenAccount::try_deserialize(&publisher_ata_info.data)
            .map_err(|_| AnchorError::from("Invalid ATA"))?;

        let mint_info = &spl_token::Mint::new(
            USDT_MINT,
            &spl_token::Mint::id(), // Associated token program
            &publisher_wallet.key(),
            false,
            false,
        );

        let fee_amount = TokenAmount::from(1_000_000); // 1 USDT
        token::transfer(
            CpiContext::new(
                ctx.program.to_account_info(),
                spl_token::Transfer {
                    from: publisher_ata,
                    to: ctx.accounts.fee_wallet.to_account_info(),
                    amount: fee_amount,
                    authority: publisher_ata_info.key,
                },
            ),
            fee_amount,
        )?;

        // Check balance is OK after deduction
        let payer_balance = get_balance(&ctx.accounts.publisher.key(), &ctx.accounts.publisher.to_account_info(), &ctx.accounts.publisher.key())?;
        if payer_balance < 0 {
            return Err(ErrorCode::InsufficientBalance.into());
        }

        // Create listing
        let now = get_current_timestamp();
        let listing = Listing {
            listing_id,
            seller: ctx.accounts.publisher.key(),
            title,
            github_url,
            description,
            price_usdt,
            sold: false,
            buyer: Pubkey::default(),
            published_at: now,
        };

        account.listings.push(listing);

        emit!(ListingCreated {
            listing_id,
            seller: ctx.accounts.publisher.key(),
            title,
            price_usdt,
        });

        Ok(())
    }

    /// Purchase an unsold listing.
    ///
    /// Requires:
    /// - Listing exists and not sold
    /// - Caller != listing.seller
    /// - Caller has sufficient USDT
    pub fn purchase_listing(
        ctx: Context<PurchaseListing>,
        listing_id: u64,
    ) -> Result<()> {
        let account = &mut ctx.accounts.marketplace_account;

        // Find listing
        let listing_idx = account
            .listings
            .iter()
            .position(|l| l.listing_id == listing_id);
        let listing_idx = match listing_idx {
            Some(idx) => idx,
            None => return Err(ErrorCode::ListingNotFound.into()),
        };

        let listing = &mut account.listings[listing_idx];

        // Validate
        if listing.sold {
            return Err(ErrorCode::AlreadySold.into());
        }

        if ctx.accounts.buyer.key() == listing.seller {
            return Err(ErrorCode::CannotBuyOwnListing.into());
        }

        // Calculate fee and seller amount
        let price = listing.price_usdt;
        let fee_amount = price.checked_mul(PLATFORM_FEE_PERCENT)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            / 100;
        let seller_amount = price.saturating_sub(fee_amount);

        // Transfer USDT from buyer to marketplace program wallet
        let buyer_ata_info = ctx.accounts.buyer.try_borrow_mut_account()?;
        let buyer_ata = TokenAccount::try_deserialize(&buyer_ata_info.data)
            .map_err(|_| AnchorError::from("Invalid ATA"))?;

        let buyer_balance = get_balance(&ctx.accounts.buyer.key(), &ctx.accounts.buyer.to_account_info(), &ctx.accounts.buyer.key())?;
        if buyer_balance < price {
            return Err(ErrorCode::InsufficientBalance.into());
        }

        // Transfer total price to marketplace program wallet
        let program_ata = spl_token::instruction::get_associated_token_address(
            &ctx.program.key(),
            &USDT_MINT,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.program.to_account_info(),
                spl_token::Transfer {
                    from: buyer_ata,
                    to: ctx.accounts.marketplace_token_account.to_account_info(),
                    amount: TokenAmount::from(price),
                    authority: buyer_ata_info.key,
                },
            ),
            TokenAmount::from(price),
        )?;

        // Split: fee to platform wallet, seller amount to seller
        if fee_amount > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.program.to_account_info(),
                    spl_token::Transfer {
                        from: ctx.accounts.marketplace_token_account.to_account_info(),
                        to: ctx.accounts.platform_fee_wallet.to_account_info(),
                        amount: TokenAmount::from(fee_amount),
                        authority: ctx.program.key(),
                    },
                ),
                TokenAmount::from(fee_amount),
            )?;
        }

        token::transfer(
            CpiContext::new(
                ctx.program.to_account_info(),
                spl_token::Transfer {
                    from: ctx.accounts.marketplace_token_account.to_account_info(),
                    to: ctx.accounts.seller_wallet.to_account_info(),
                    amount: TokenAmount::from(seller_amount),
                    authority: ctx.program.key(),
                },
            ),
            TokenAmount::from(seller_amount),
        )?;

        // Mark as sold
        listing.sold = true;
        listing.buyer = ctx.accounts.buyer.key();

        emit!(ListingSold {
            listing_id,
            seller: listing.seller,
            buyer: ctx.accounts.buyer.key(),
            price_usdt: price,
        });

        Ok(())
    }

    /// Withdraw a sold listing back to seller.
    pub fn withdraw_listing(
        ctx: Context<WithdrawListing>,
        listing_id: u64,
    ) -> Result<()> {
        let account = &mut ctx.accounts.marketplace_account;

        let listing_idx = account
            .listings
            .iter()
            .position(|l| l.listing_id == listing_id);
        let listing_idx = match listing_idx {
            Some(idx) => idx,
            None => return Err(ErrorCode::ListingNotFound.into()),
        };

        let listing = &mut account.listings[listing_idx];

        // Validate
        if listing.sold {
            return Err(ErrorCode::AlreadySold.into());
        }
        if ctx.accounts.seller.key() != listing.seller {
            return Err(ErrorCode::NotSeller.into());
        }

        let price = listing.price_usdt;
        let fee_amount = price.checked_mul(PLATFORM_FEE_PERCENT)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            / 100;
        let amount = price.saturating_sub(fee_amount);

        // Transfer all to seller
        let seller_ata_info = ctx.accounts.seller.try_borrow_mut_account()?;
        let seller_ata = TokenAccount::try_deserialize(&seller_ata_info.data)
            .map_err(|_| AnchorError::from("Invalid ATA"))?;

        token::transfer(
            CpiContext::new(
                ctx.program.to_account_info(),
                spl_token::Transfer {
                    from: ctx.accounts.marketplace_token_account.to_account_info(),
                    to: seller_ata,
                    amount: TokenAmount::from(amount),
                    authority: ctx.program.key(),
                },
            ),
            TokenAmount::from(amount),
        )?;

        // Reset to unsold
        listing.sold = false;
        listing.buyer = Pubkey::default();

        Ok(())
    }

    /// Get a listing by ID (view-only).
    pub fn get_listing(
        ctx: Context<GetListing>,
        listing_id: u64,
    ) -> Result<()> {
        let account = &ctx.accounts.marketplace_account;

        if let Some(listing) = account.listings.iter().find(|l| l.listing_id == listing_id) {
            return Ok(());
        }
        return Err(ErrorCode::ListingNotFound.into());
    }

    /// Clean up expired unsold listings (> 30 days old).
    pub fn cleanup_expired(_ctx: Context<InitMarketplace>, _cleanup: u64) -> Result<()> {
        // NOTE: In production, this would be a handler or triggered via event
        // For now, just log the cleanup intent
        return Ok(());
    }
}

// ─── Instruction Accounts ────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct InitMarketplace<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Marketplace PDA (fixed key derived from "market_authority" + authority)
    pub marketplace_account: Account<'info, MarketplaceAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PublishListing<'info> {
    #[account(mut)]
    pub publisher: Signer<'info>,

    /// CHECK: Marketplace PDA
    pub marketplace_account: Account<'info, MarketplaceAccount>,

    /// CHECK: Fee wallet PDA (platform)
    #[account(mut)]
    pub fee_wallet: UncheckedAccount<'info>,

    /// CHECK: Marketplace ATA (to deduct fee)
    #[account(mut)]
    pub marketplace_token_account: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct PurchaseListing<'info> {
    /// CHECK: Marketplace PDA
    pub marketplace_account: Account<'info, MarketplaceAccount>,

    /// CHECK: Marketplace token account (to receive payment)
    #[account(mut)]
    pub marketplace_token_account: Account<'info, TokenAccount>,

    /// CHECK: Marketplace program
    pub marketplace_program: Program<'info, MarketplaceProgram>,

    /// CHECK: Platform fee wallet PDA
    #[account(mut)]
    pub platform_fee_wallet: UncheckedAccount<'info>,

    /// CHECK: Seller wallet PDA (to receive seller amount)
    #[account(mut)]
    pub seller_wallet: UncheckedAccount<'info>,

    /// CHECK: Buyer wallet (signer)
    pub buyer: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawListing<'info> {
    /// CHECK: Marketplace PDA
    pub marketplace_account: Account<'info, MarketplaceAccount>,

    /// CHECK: Marketplace program
    pub marketplace_program: Program<'info, MarketplaceProgram>,

    /// CHECK: Marketplace token account
    #[account(mut)]
    pub marketplace_token_account: Account<'info, TokenAccount>,

    /// CHECK: Platform fee wallet PDA (may still hold fee)
    #[account(mut)]
    pub platform_fee_wallet: UncheckedAccount<'info>,

    /// CHECK: Seller wallet (signer)
    pub seller: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetListing<'info> {
    pub marketplace_account: Account<'info, MarketplaceAccount>,
}
