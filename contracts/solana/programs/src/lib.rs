//! IdleBot Anchor workspace — Solana programs for marketplace and subscriptions.
//!
//! This workspace contains multiple Anchor programs:
//! - marketplace: Template marketplace with listings, purchase, and fees
//! - subscription: Premium subscription with template limits
//! - token_utils: SPL Token transfer helpers
//!
//! Each program is compiled independently via `anchor build --program <name>`.
//! For local development: `anchor localnet --provider.cluster devnet`.

pub mod marketplace;
pub mod subscription;
pub mod token_utils;

// Re-export key types for convenience
pub use marketplace::{
    init_marketplace, publish_listing, purchase_listing, withdraw_listing, get_listing,
    ListingCreated, ListingSold,
};
pub use subscription::{
    init_subscription, purchase_subscription, refund_subscription, cancel_subscription,
    SubscriptionPurchased, SubscriptionRefunded,
};
pub use token_utils::*;
