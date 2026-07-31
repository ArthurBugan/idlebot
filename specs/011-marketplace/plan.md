# Plan 011: Marketplace System

> **Implementation Plan**

## Architecture

### Marketplace Listing Model
- Publish: Create listing with title, description, GitHub URL, USDT price (costs 50G)
- Browse: View all public listings with search/filter
- Purchase: Pay USDT via smart contract (platform fee: 5%)
- Delivery: Listing marked as sold, 30-day expiry

### Platform Integration
- Server records listings (server-authoritative)
- Smart contract (Solana/Anchor) handles USDT transfers
- Platform fee: 5% of sale price deducted automatically

## Files to Create/Modify

### Server (idlecore-server)
- `src/market.rs` — MarketplaceSystem struct, publish_listing(), buy_listing(), is_expired()

### Smart Contracts (new)
- `contracts/solana/programs/src/marketplace.rs` — Anchor marketplace program
- `contracts/solana/programs/src/token_utils.rs` — USDT transfer helpers

### Client (idlecore-client)
- Modify `src/interaction.rs` — Add publish/buy action handlers

## Dependencies
- Requires 010-economy (USDT currency system)
- Requires 012-smart-contracts (Anchor marketplace program)
- Requires 019-database-schema (table definitions)

## Testing Strategy
1. Unit test: Publish listing deducts 50G correctly
2. Unit test: Buy listing deducts USDT, marks sold
3. Integration test: Full publish→browse→buy flow
4. Edge case: Expired listing rejected, already-sold rejected

## Timeline
- **Estimate:** 3-4 days
- **Phase:** Phase 2 (Marketplace)
- **Blocked Until:** 010-economy and 012-smart-contracts must be complete
