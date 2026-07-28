# Plan 012: Smart Contracts (Solana / Anchor)

> **Implementation Plan**

## Architecture

### Workspace Structure
```
contracts/solana/
├── Cargo.toml
├── Anchor.toml
├── programs/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── marketplace.rs
│       ├── subscription.rs
│       └── token_utils.rs
└── tests/
    ├── marketplace.ts
    └── subscription.ts
```

### Marketplace Program (Anchor)
- Publish: Create listing (deduct 50G publishing fee)
- Purchase: Buy listing (collect 5% platform fee)
- Withdraw: Refund sold listing back to seller

### Subscription Program (Anchor)
- Purchase: Pay 1 USDT for 30-day premium (500 templates)
- Refund: Refund if active
- Cancel: Refund if not active

### Token Utilities
- SPL Token 2022 for USDT transfers
- balance checks, transfers, withdrawals

## Files to Create/Modify

### Anchor Workspace
- `contracts/solana/Cargo.toml` — Workspace manifest
- `contracts/solana/Anchor.toml` — Solana devnet config
- `contracts/solana/programs/Cargo.toml` — Program package

### Programs
- `contracts/solana/programs/src/lib.rs` — Module declarations
- `contracts/solana/programs/src/marketplace.rs` — Marketplace program
- `contracts/solana/programs/src/subscription.rs` — Subscription program
- `contracts/solana/programs/src/token_utils.rs` — Token transfer helpers

## Testing Strategy
1. Unit test: Marketplace program compiles
2. Unit test: Subscription program compiles
3. Integration test: Marketplace test (Anchor test framework)
4. Integration test: Subscription test
5. Edge case: PDA collision detection

## Dependencies
- Depends on 010-economy (currency definitions)
- Depends on 011-marketplace (marketplace concepts)
- Depends on 013-wallet-auth (wallet interaction)

## Timeline
- **Estimate:** 3-5 days
- **Phase:** MVP Core Loop
