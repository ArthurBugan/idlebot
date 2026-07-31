# Tasks 011: Marketplace System

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define ListingCategory enum (Agent, Code, Template, Snippet)
- [ ] **T1.2** Define MarketplaceListing struct (id, seller_id, title, description, github_url, price_usdt, category, published_at, expires_at, is_sold, buyer_id)
- [ ] **T1.3** Implement is_expired() method (30-day expiry)
- [ ] **T1.4** Create MarketplaceSystem struct (listings vec, platform_fee_percent)

## Phase 2: Publish Listing
- [ ] **T2.1** Implement publish_listing() — validate inputs, deduct 50G
- [ ] **T2.2** Deduct 50G from player gold
- [ ] **T2.3** Generate unique listing ID (UUID v4)
- [ ] **T2.4** Set expires_at to 30 days from published_at
- [ ] **T2.5** Return created listing

## Phase 3: Browse Listings
- [ ] **T2.6** Implement get_all_listings() — filter expired, unsold
- [ ] **T2.7** Implement search/filter by category, title, price
- [ ] **T2.8** Expose listings to client subscription

## Phase 4: Purchase Listing
- [ ] **T2.9** Implement buy_listing() — validate not sold, check USDT
- [ ] **T2.10** Calculate platform fee (5% of price_usdt)
- [ ] **T2.11** Deduct full price from buyer USDT
- [ ] **T2.12** Credit seller with (price - fee) — via smart contract
- [ ] **T2.13** Mark listing as sold, set buyer_id
- [ ] **T2.14** Create transaction record

## Phase 5: Smart Contract Integration
- [ ] **T3.1** Deploy Anchor marketplace program
- [ ] **T3.2** Implement publish_listing instruction
- [ ] **T3.3** Implement buy_listing instruction (USDT transfer)
- [ ] **T3.4** Implement withdraw_listing instruction
- [ ] **T3.5** Implement cleanup_expired_listings scheduled function
- [ ] **T3.6** Emit events (ListingCreated, ListingSold)

## Phase 6: Client Integration
- [ ] **T4.1** Display marketplace UI with listing grid
- [ ] **T4.2** Implement publish listing form
- [ ] **T4.3** Implement buy listing button
- [ ] **T4.4** Show platform fee and seller amount

## Phase 7: Testing
- [ ] **T5.1** Publish with insufficient gold fails
- [ ] **T5.2** Purchase with insufficient USDT fails
- [ ] **T5.3** Platform fee calculated correctly (5%)
- [ ] **T5.4** Listing marked sold after purchase
- [ ] **T5.5** Expired listings handled correctly

## Verification
- [✓] MarketplaceListing struct matches spec
- [✓] publish_listing deducts 50G
- [✓] buy_listing marks listing as sold
