//! USDTInterface — SPL Token wrapper interface (port of USDTInterface.sol,
//! Spec 012). The on-chain helpers live in `programs/src/token_utils.rs`;
//! this file documents the full interface surface so off-chain wallets can
//! interact with the USDC-branded SPL Token 2022 mint (6 decimals).

use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

/// USDTImpl — wraps an SPL Token 2022 mint (owner, decimals, transfer
/// authority). Mirrors the Solidity `USDTImpl` struct.
pub struct USDTImpl {
    pub mint: Pubkey,
    pub decimals: u8,
    pub authority: Pubkey,
}

impl USDTImpl {
    /// Construct a wrapper for a given mint, defaulting to 6 decimals.
    pub fn new(mint: Pubkey, authority: Pubkey) -> Self {
        Self {
            mint,
            decimals: 6,
            authority,
        }
    }
}

/// Check whether a token account balance covers `amount`.
pub fn has_enough_balance(balance: u64, amount: u64) -> bool {
    balance >= amount
}

/// Read a wallet's USDT balance from its ATA.
pub fn get_user_balance(token_account: &Account<TokenAccount>) -> u64 {
    token_account.amount
}

/// Transfer `amount` from `from` to `to` with `authority` as signer.
pub fn transfer_to<'info>(
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

/// Withdraw `amount` from a PDA-owned account, signing with `seeds`.
pub fn withdraw<'info>(
    token_program: &Program<'info, Token>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    seeds: &[&[&[u8]]],
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
            seeds,
        ),
        amount,
    )
}

/// The mint the marketplace and subscription programs accept.
pub const USDT_DECIMALS: u8 = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_default_to_six() {
        let usdt = USDTImpl::new(Pubkey::default(), Pubkey::default());
        assert_eq!(usdt.decimals, 6);
    }

    #[test]
    fn balance_check() {
        assert!(has_enough_balance(1_000_000, 1_000_000));
        assert!(!has_enough_balance(999, 1_000));
    }
}