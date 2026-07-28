# Spec 012: Smart Contracts (Solana / Anchor)

> **Objective:** Implement Solana blockchain smart contracts for marketplace transactions, subscriptions, and token transfers using the Anchor framework.

## Problem Statement

The IdleBot workspace needs on-chain automation for:
- Marketplace template listings and purchases
- USDT-compatible token transfers
- Optional premium subscription tiers
- Platform fee collection

Contracts must be auditable, gas-efficient, and support EVM-compatible wallet interaction.

## Stack

- **Framework:** Anchor (Anchorlang) on Solana
- **Runtime:** Solana mainnet / devnet
- **Token Standard:** SPL Token 2022 (USDC-branded, 6 decimals)
- **Dependency Tracking:** Anchor workspace + local `.solana-anchor-version`

## Workspace Structure

```
contracts/solana/
├── Cargo.toml          # Anchor workspace manifest
├── Anchor.toml         # Anchor config (Solana RPC, cluster)
├── programs/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs      # Workspace library
│       ├── marketplace.rs    # Template marketplace program
│       ├── subscription.rs   # Subscription program
│       └── token_utils.rs    # USDT transfer helpers
├── tests/
│   ├── marketplace.ts          # Marketplace tests (Anchor's anchor-test)
│   ├── subscription.ts         # Subscription tests
│   └── token_utils.ts          # Token transfer tests
└── token/
    └── USDTInterface.rs    # SPL Token wrapper interface (Rust, not Solidity)
```

## Core Concepts (from Solidity port)

### 1. SPL Token 2022 / USDT Implementation (token_utils.rs)
- **USDTImpl struct** — wraps a `SPLToken2022` mint with owner, decimals, transfer authority
- **transfer_to(usr, amount)** — transfer tokens to user's wallet
- **withdraw(amount, to_wallet)** — withdraw from program wallet
- **has_enough_balance(mint, balance)** — check balance >= amount
- **get_user_balance(wallet)** — read user balance via connection

### 2. Marketplace Program (marketplace.rs)

`#[program]` Anchor program with PDA-based storage.

**Struct (on-chain):**
```rust
struct MarketplaceAccount {
    pub program_id: Pubkey,  // CPI marker
    pub market_authority: Pubkey,  // PDA: (b"market_authority")
    pub listings: Vec<Listing>,
    pub padding: [u8; 12],  // alignment
}

struct Listing {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub title: String,        // Bump-slice: Anchor's StringsType
    pub github_url: String,
    pub description: String,
    pub price_usdt: u64,      // 6 decimal USDT
    pub sold: bool,
    pub buyer: Pubkey,         // = PublicKey::default() if unsold
    pub published_at: u64,
}
```

**Instructions:**

1. `publish_listing {
   pubkey: signer`,
   list_id: u64,
   title: String,
   github_url: String,
   description: String,
   price_usdt: u64,
   }
   - Require price >= 10_000 (0.01 USDT minimum)
   - Require caller is NOT already seller
   - Deduct 50 USDT from caller's wallet (publishing fee)
   - Add listing to MarketplaceAccount's vector
   - Emit `ListingCreated` event

2. `purchase_listing {
   pubkey: signer`,
   list_id: u64,
   }
   - Require listing not sold
   - Require caller != listing.seller
   - Charge `price_usdt * 5 / 100` fee to platform
   - Transfer `price_usdt - fee` to seller's wallet
   - Transfer fee to platform wallet
   - Set `listing.sold = true`
   - Emit `ListingSold` event

3. `withdraw_listing {
   pubkey: signer`,
   list_id: u64,
   }
   - Require caller == listing.seller
   - Require listing is sold
   - Transfer fee + seller_amount to seller wallet
   - Set `listing.sold = false`

4. `get_listing {
   pubkey: signer`,
   list_id: u64,
   }
   - View-only: return listing if exists (returns None if not)

5. `cleanup_expired_listings {}` (handler)
   - Scan all listings, mark unsold listings older than 30 days as sold

**Validation helpers:**
```rust
fn is_address_listed(addr: &Pubkey) -> bool {
    // Check if address exists in any seller or buyer field
    // Returns bool
}

fn is_duplicate_listings(listing_id: u64) -> bool {
    // Check if listing_id already used
    // Returns bool
}
```

### 3. Subscription Program (subscription.rs)

`#[program]` Anchor program with PDA-based storage.

**Struct:**
```rust
struct SubscriptionAccount {
    pub program_id: Pubkey,
    pub owner: Pubkey,          // User wallet
    pub premium_until: u64,     // unix timestamp
    pub limit: u32,             // FREE_LIMIT or PREMIUM_LIMIT
    pub last_transaction: u64,  // block timestamp of last tx
    pub padding: [u8; 56],      // alignment
}
```

**Constants:**
```rust
const PREMIUM_PRICE: u64 = 1_000_000; // 1 USDT
const MONTH_SECONDS: u64 = 30 * 24 * 3600;
const FREE_LIMIT: u32 = 50;
const PREMIUM_LIMIT: u32 = 500;
```

**Instructions:**

1. `purchase_subscription {
   pubkey: signer`,
   payment_token: Pubkey,  // USDT mint pubkey
   user: Pubkey,  // owner pubkey (can differ from signer for delegation)
   }
   - Verify user is not already subscribed (user.premium_until > now)
   - Transfer PREMIUM_PRICE (1 USDT) from signer to program wallet
   - Set user.premium_until = max(current, now) + MONTH_SECONDS
   - Update user.limit = PREMIUM_LIMIT
   - Emit `SubscriptionPurchased` event

2. `refund_subscription {
   pubkey: signer`,
   payment_token: Pubkey,
   user: Pubkey,
   }
   - Require user is active (premium_until > now)
   - Refund PREMIUM_PRICE to signer
   - Set user.premium_until = 0
   - Set user.limit = FREE_LIMIT
   - Emit `SubscriptionRefunded` event

3. `cancel_subscription {
   pubkey: signer`,
   payment_token: Pubkey,
   user: Pubkey,
   }
   - Require user is NOT active
   - Refund to signer
   - Revert user.limit to FREE_LIMIT

4. `withdraw_subscription {
   pubkey: signer`,
   payment_token: Pubkey,
   user: Pubkey,
   }
   - Require user.premium_until > now
   - Refund remaining amount to signer
   - Set user.premium_until = 0
   - Revert user.limit to FREE_LIMIT

**Validation helpers:**
```rust
fn get_time_now(connection: &Connection) -> u64 {
    connection.get_block_timestamp().await.unwrap().timestamp
}
```

### 4. Event Logs

**Marketplace events (subscribed via Anchor `pubsub`):**
```rust
#[event]
struct ListingCreated {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub title: String,
    pub price_usdt: u64,
}

#[event]
struct ListingSold {
    pub listing_id: u64,
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub price_usdt: u64,
}
```

**Subscription events:**
```rust
#[event]
struct SubscriptionPurchased {
    pub user: Pubkey,
    pub premium_until: u64,
}

#[event]
struct SubscriptionRefunded {
    pub user: Pubkey,
}
```

## Acceptance Criteria

- [ ] All programs compile with Anchor (no warnings)
- [ ] Anchor tests pass on devnet (anchor test run)
- [ ] Marketplace: listing creation + purchase flow verified
- [ ] Marketplace: fee collection (5%) verified
- [ ] Subscription: purchase with 1 USDT verified
- [ ] Subscription: refund/cancel flow verified
- [ ] Token transfer (transfer_to / withdraw) returns success
- [ ] Events emitted correctly and subscribable
- [ ] Gas usage within reasonable bounds (< 50k compute units)
- [ ] PDA derivation matches across programs (shared authorities)

## File Reference (Solidity → Anchor port)

| Solidity File | Anchor Replacement | Location |
|---|---|---|
| TemplateMarket.sol | marketplace.rs | `programs/src/` |
| Subscription.sol | subscription.rs | `programs/src/` |
| USDTInterface.sol | token_utils.rs | `programs/src/` |

## Risks

- R1: PDA collision between programs (prefix derivation)
- R2: Anchor workspace Cargo.toml version locking (use bump lockfile)
- R3: Testnet differences (compute budget, fee structure)
