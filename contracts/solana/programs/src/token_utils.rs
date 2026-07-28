//! Token utilities — USDT (SPL Token 2022) transfer helpers.
//! Maps Solidity USDTInterface.sol to Rust/Anchor equivalents.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

/// USDT mint pubkey (Solana native wrapped SOL — standard USDT proxy)
pub const USDT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Check if balance is sufficient.
pub fn has_enough_balance(current: u64, amount: u64) -> bool {
    current >= amount
}

/// Get balance from a token account.
pub fn get_balance(
    token_account: &AccountInfo,
) -> Result<u64> {
    let ata = token_account.try_borrow_account()?;
    let account = TokenAccount::try_deserialize(&ata.data)
        .map_err(|_| AnchorError::msg("Invalid token account"))?;
    Ok(account.amount)
}

/// Transfer tokens from a source account to a destination (CPI).
pub fn transfer_usdt<'info>(
    program_id: Pubkey,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let mint_pubkey = pubkey!(USDT_MINT);
    let dest_ata = spl_token::instruction::get_associated_token_address(&to.key(), &mint_pubkey)?;

    token::transfer(
        CpiContext::new(
            program_id,
            spl_token::Transfer {
                from,
                to,
                amount: TokenAmount::from(amount),
                authority: from.key,
            },
        ),
        TokenAmount::from(amount),
    )?;
    Ok(())
}

/// Approve spending authority on a token account.
pub fn approve_token<'info>(
    program_id: Pubkey,
    token_account: AccountInfo<'info>,
    spender: Pubkey,
    amount: u64,
) -> Result<()> {
    token::approve(
        CpiContext::new(program_id, spl_token::Approve {
            account: token_account,
            authority: token_account.key,
            delegate: spender,
            amount: TokenAmount::from(amount),
        }),
        TokenAmount::from(amount),
    )?;
    Ok(())
}
