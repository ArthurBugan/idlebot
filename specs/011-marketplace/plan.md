# Plan 011: Marketplace System

> **Implementation Plan**

## Architecture

### Marketplace Data Model
- Listing: id, seller_id, title, description, github_url, price_usdt, category, published_at, expires_at, is_sold, buyer_id
- Publish: Create listing with title, description, GitHub URL, USDT price (costs 50G)
- Browse: View all public listings
- Purchase: Pay USDT, platform fee 5%, seller receives remainder

### Smart Contract Integration
- Anchor-based marketplace program (see spec 012 for Anchor implementation)
- 1:1 mapping from Solidity TemplateMarket.sol to Rust

## Files to Create/Modify

### Core (idlecore-core)
- `src/marketplace.rs` — MarketplaceListing struct, publish/browse/purchase logic

### Server (idlecore-server)
- `src/market.rs` — Register marketplace reducers, smart contract interaction

### Client (idlecore-client)
- `src/marketplace_ui.rs` — Publish form, browse grid, purchase flow

## Testing Strategy
1. Unit test: Publishing deducts 50G correctly
2. Unit test: Purchase transfers USDT correctly
3. Unit test: Platform fee (5%) calculated correctly
4. Integration test: Publish → browse → purchase flow
5. Edge case: Expired listing cleanup

## Dependencies
- Depends on 010-economy (gold/USDT management)
- Depends on 012-smart-contracts-anchor (Anchor marketplace program)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
