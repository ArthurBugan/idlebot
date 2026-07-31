# Tasks 012: Smart Contracts (Solana/Anchor)

> **Implementation Checklist**

## Phase 1: Workspace Setup
- [ ] **T1.1** Create contracts/solana/ directory structure
- [ ] **T1.2** Create Cargo.toml (Anchor workspace manifest)
- [ ] **T1.3** Create Anchor.toml (Solana devnet config)
- [ ] **T1.4** Create programs/Cargo.toml (program configs)
- [ ] **T1.5** Install Anchor CLI locally

## Phase 2: Token Utils Program (token_utils.rs)
- [ ] **T2.1** Implement USDTImpl struct (SPLToken2022 wrapper with owner, decimals, transfer authority)
- [ ] **T2.2** Implement transfer_to(usr, amount) — transfer tokens to user's wallet
- [ ] **T2.3** Implement withdraw(amount, to_wallet) — withdraw from program wallet
- [ ] **T2.4** Implement has_enough_balance(mint, balance) — check balance >= amount
- [ ] **T2.5** Implement get_user_balance(wallet) — read user balance via connection
- [ ] **T2.6** Create token/USDTInterface.rs wrapper interface

## Phase 3: Marketplace Program (marketplace.rs)
- [ ] **T3.1** Define MarketplaceAccount struct with PDA (program_id, market_authority, listings, padding)
- [ ] **T3.2** Define Listing struct (listing_id, seller, title, github_url, description, price_usdt, sold, buyer, published_at)
- [ ] **T3.3** Implement publish_listing instruction (deduct 50 USDT, add listing, emit event)
- [ ] **T3.4** Implement purchase_listing instruction (charge fee, transfer to seller, mark sold)
- [ ] **T3.5** Implement withdraw_listing instruction (seller reclaims fees)
- [ ] **T3.6** Implement get_listing instruction (view-only)
- [ ] **T3.7** Implement cleanup_expired_listings handler (mark unsold >30 days as sold)
- [ ] **T3.8** Emit ListingCreated and ListingSold events
- [ ] **T3.9** Add validation helpers (is_address_listed, is_duplicate_listings)

## Phase 4: Subscription Program (subscription.rs)
- [ ] **T4.1** Define SubscriptionAccount struct (program_id, owner, premium_until, limit, last_transaction, padding)
- [ ] **T4.2** Define constants (PREMIUM_PRICE=1_000_000, MONTH_SECONDS, FREE_LIMIT=50, PREMIUM_LIMIT=500)
- [ ] **T4.3** Implement purchase_subscription (transfer 1 USDT, set premium_until)
- [ ] **T4.4** Implement refund_subscription (verify active, refund 1 USDT, reset)
- [ ] **T4.5** Implement cancel_subscription (verify inactive, refund signer)
- [ ] **T4.6** Implement withdraw_subscription (refund remaining)
- [ ] **T4.7** Emit SubscriptionPurchased and SubscriptionRefunded events

## Phase 5: Tests
- [ ] **T5.1** Token transfer_to test
- [ ] **T5.2** Marketplace listing creation test
- [ ] **T5.3** Marketplace purchase flow test (fee collection)
- [ ] **T5.4** Subscription purchase with 1 USDT test
- [ ] **T5.5** Subscription refund/cancel test
- [ ] **T5.6** Event emission test

## Verification
- [✓] All programs compile with Anchor (no warnings)
- [✓] PDA derivation matches across programs
- [✓] Marketplace listing creation + purchase flow verified
