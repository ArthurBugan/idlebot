//! Token utilities — USDT (SPL Token 2022) transfer helpers.
//! Maps Solidity USDTInterface.sol to Rust/Anchor equivalents.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey;
use anchor_spl::token::{transfer, Token, Transfer};

/// USDT mint pubkey (wrapped SOL placeholder until the USDC-branded SPL
/// Token 2022 mint is deployed; programs receive the real mint as an
/// instruction account so this constant is only used off-chain).
pub const USDT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Check whether a balance covers an amount.
pub fn has_enough_balance(current: u64, amount: u64) -> bool {
    current >= amount
}

/// Read the raw token balance of an account.
pub fn get_balance(token_account: &AccountInfo) -> Result<u64> {
    let account = anchor_spl::token::TokenAccount::try_deserialize(&mut &**token_account.data.borrow())?;
    Ok(account.amount)
}

/// Transfer tokens between accounts, with `authority` as signer.
pub fn transfer_usdt<'info>(
    token_program: &Program<'info, Token>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            token_program.to_account_info(),
            Transfer {
                from,
                to,
                authority,
            },
        ),
        amount,
    )
}

/// Transfer tokens from a PDA-owned account, signing with `signer_seeds`.
pub fn transfer_usdt_with_signer<'info>(
    token_program: &Program<'info, Token>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Transfer {
                from,
                to,
                authority,
            },
            signer_seeds,
        ),
        amount,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_checks() {
        assert!(has_enough_balance(1_000_000, 1_000_000));
        assert!(!has_enough_balance(999, 1_000));
    }
}