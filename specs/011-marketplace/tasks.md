# Tasks 011: Marketplace System

> **Implementation Checklist**

## Phase 1: Data Model
- [x] **T1.1** Define ListingCategory enum - Simplified to MarketListing struct
- [x] **T1.2** Define MarketplaceListing struct (id, seller_id, title, description, github_url, price_usdt, category, published_at, expires_at, is_sold, buyer_id)
- [x] **T1.3** Implement is_expired() method (30-day expiry)
- [x] **T1.4** Create MarketplaceSystem struct (listings vec, platform_fee_percent) - MarketplaceManager

## Phase 2: Publish Listing
- [x] **T2.1** Implement publish_listing() — validate inputs, deduct 50G - list_item()
- [x] **T2.2** Deduct 50G from player gold
- [x] **T2.3** Generate unique listing ID (incrementing counter) - next_listing_id
- [x] **T2.4** Set expires_at to 30 days from published_at - is_expired() uses 30-day check
- [x] **T2.5** Return created listing

## Phase 3: Browse Listings
- [x] **T2.6** Implement get_all_listings() — filter expired, unsold - active_listings()
- [x] **T2.7** Implement search/filter by category, title, price - search_by_title(), filter_by_max_price()
- [x] **T2.8** Listings exposed — subscribe_to_all_tables caches market_listing rows client-side

## Phase 4: Purchase Listing
- [x] **T2.9** Implement buy_listing() — validate not sold, check USDT - sell_listing()
- [x] **T2.10** Calculate platform fee (5% of price_usdt) - platform_fee()
- [x] **T2.11** Deduct full price from buyer USDT - mock
- [x] **T2.12** Credit seller with (price - fee) — via smart contract - mock
- [x] **T2.13** Mark listing as sold, set buyer_id
- [x] **T2.14** Transaction records — spend_usdt/spend_gold ledger rows on buy/publish

## Phase 5: Smart Contract Integration
- [x] **T3.1** N/A — chain layer replaced by the SpacetimeDB module (server-authoritative)
- [x] **T3.2** publish reducer + validate_publish pure rules
- [x] **T3.3** buy reducer + resolve_buy (fee/escrow) pure rules
- [x] **T3.4** release_escrow + dispute refund paths
- [x] **T3.5** scheduled_market_cleanup (hourly)
- [x] **T3.6** Table replication + tracing events mirror ListingCreated/ListingSold

## Phase 6: Client Integration
- [x] **T4.1** Marketplace UI — K toggles listing grid (net/market.rs)
- [x] **T4.2** Publish form — preset category buttons (Agent/Code/Template/Snippet), 10 USDT
- [x] **T4.3** Buy button per row — buy_listing reducer with result log
- [x] **T4.4** Fee display — 5% fee + 48h escrow noted on buy

## Phase 7: Testing
- [x] **T5.1** Publish with insufficient gold fails - test_marketplace_manager_list_insufficient_gold
- [x] **T5.2** tests_buy::buy_with_insufficient_usdt_fails
- [x] **T5.3** Platform fee calculated correctly (5%) - test_platform_fee
- [x] **T5.4** buy sets is_sold/buyer (resolve_buy gate + reducer mutation)
- [x] **T5.5** Expired listings handled correctly - test_listing_expired

## Verification
- [✓] MarketplaceListing struct matches spec
- [✓] publish_listing deducts 50G
- [✓] buy_listing marks listing as sold
