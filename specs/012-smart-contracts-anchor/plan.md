# Plan 012: Smart Contracts (Solana/Anchor)

> **Implementation Plan**

## Architecture

### Anchor Workspace Structure
```
contracts/solana/
├── Cargo.toml          # Anchor workspace manifest
├── Anchor.toml         # Anchor config (Solana devnet)
├── programs/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── marketplace.rs
│       ├── subscription.rs
│       └── token_utils.rs
├── tests/
│   ├── marketplace.ts
│   ├── subscription.ts
│   └── token_utils.ts
└── token/
    └── USDTInterface.rs  # SPL Token wrapper interface
```

### Core Programs
1. **Token Utils** — SPL Token 2022 transfer helpers (transfer_to, withdraw, balance check)
2. **Marketplace** — Template marketplace with PDA storage (publish, purchase, withdraw, get_listing)
3. **Subscription** — Premium subscription with PDA storage (purchase, refund, cancel, withdraw)

## Files to Create

### Smart Contracts (new directory)
- `contracts/solana/Cargo.toml` — Anchor workspace config
- `contracts/solana/Anchor.toml` — Solana RPC/cluster config
- `contracts/solana/programs/Cargo.toml` — Program configs
- `contracts/solana/programs/src/lib.rs` — Workspace library
- `contracts/solana/programs/src/marketplace.rs` — Marketplace program
- `contracts/solana/programs/src/subscription.rs` — Subscription program
- `contracts/solana/programs/src/token_utils.rs` — Token transfer helpers
- `contracts/solana/tests/marketplace.ts` — Marketplace tests
- `contracts/solana/tests/subscription.ts` — Subscription tests
- `contracts/solana/tests/token_utils.ts` — Token transfer tests

### Dependency: token/ directory
- `token/USDTInterface.rs` — SPL Token wrapper interface (Rust, not Solidity)

## Dependencies
- Requires 011-marketplace (marketplace logic on server-side)
- Requires 013-wallet-auth (wallet address for token wallets)
- Requires Anchor framework installed locally

## Testing Strategy
1. Unit test: Token transfer_to returns success
2. Unit test: Marketplace listing creation + purchase
3. Unit test: Marketplace fee collection (5%)
4. Unit test: Subscription purchase with 1 USDT
5. Unit test: Subscription refund/cancel flow
6. Integration test: Full marketplace flow on devnet

## Timeline
- **Estimate:** 3-5 days
- **Phase:** Phase 2 (Marketplace + Smart Contracts)
- **Dependencies:** Requires Anchor installation, Solana devnet
