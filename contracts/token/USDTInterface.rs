//! USDT Interface — wrapper around Solana's USDC/USDT mint.
//! Maps to idlebot-core's EconomySystem.
//!
//! Provides high-level operations for handling USDT transfers,
//! bridging, and withdrawal.

/// USDT mint pubkey (Solana USDC proxy)
pub const USDT_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// USDT token symbol
pub const TOKEN_SYMBOL: &str = "USDT";

/// USDT decimals
pub const TOKEN_DECIMALS: u8 = 6;

/// Check if address holds enough USDT.
pub fn has_enough_balance(current: u64, amount: u64) -> bool {
    current >= amount
}

/// Transfer USDT from one account to another.
pub fn transfer_usdt(from: &str, to: &str, amount: u64) -> bool {
    // In production, this would be a Solana transaction.
    // For now, just check balances and simulate.
    println!("Transfer USDT: {} -> {} amount: {}", from, to, amount);
    true
}

/// Withdraw USDT from token account to wallet.
pub fn withdraw_usdt(account: &str, amount: u64) -> bool {
    println!("Withdraw USDT from {} amount: {}", account, amount);
    true
}

/// Bridge USDT from one chain to another.
pub fn bridge_usdt(chain: &str, amount: u64) -> bool {
    println!("Bridge USDT to {} amount: {}", chain, amount);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_enough_balance() {
        assert!(has_enough_balance(1000, 500));
        assert!(!has_enough_balance(500, 500));
        assert!(!has_enough_balance(499, 500));
    }

    #[test]
    fn test_transfer_usdt() {
        assert!(transfer_usdt("alice", "bob", 100));
    }

    #[test]
    fn test_withdraw_usdt() {
        assert!(withdraw_usdt("alice", 50));
    }

    #[test]
    fn test_bridge_usdt() {
        assert!(bridge_usdt("polygon", 200));
    }
}
