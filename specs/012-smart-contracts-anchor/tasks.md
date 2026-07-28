# Tasks 012: Smart Contracts (Solana / Anchor)

> **Implementation Checklist**

## Phase 1: Workspace Setup
- [✓] **T1.1** Create Anchor workspace (Cargo.toml, Anchor.toml)
- [✓] **T1.2** Create program crate (Cargo.toml, src/lib.rs)
- [✓] **T1.3** Define markets table with Borsh serialization
- [✓] **T1.4** Define listings table with Borsh serialization

## Phase 2: Marketplace Program
- [ ] **T1.5** Create marketplace.rs with publish_listing, purchase_listing, withdraw_listing, get_listing
- [ ] **T1.6** Validate listing price (must be > 0)
- [ ] **T1.7** Validate seller has USDT for listing
- [ ] **T1.8** Transfer USDT on purchase
- [ ] **T1.9** Implement withdraw for unsold listings

## Phase 3: Subscription Program
- [ ] **T1.10** Create subscription.rs with purchase_subscription, refund_subscription, cancel_subscription
- [ ] **T1.11** Validate subscription duration
- [ ] **T1.12** Charge USDT for subscription
- [ ] **T1.13** Implement auto-renewal
- [ ] **T1.14** Implement refund processing

## Phase 4: Token Utilities
- [✓] **T1.15** Create token_utils.rs with transfer_to, withdraw, balance checks
- [✓] **T1.16** Validate USDT transfer amounts
- [✓] **T1.17** Handle token decimals

## Phase 5: Testing
- [✓] **T1.18** Test publish listing
- [✓] **T1.19** Test purchase listing
- [✓] **T1.20** Test withdraw listing
- [✓] **T1.21** Test subscribe
- [✓] **T1.22** Test token transfers
- [✓] **T1.23** Test insufficient funds
