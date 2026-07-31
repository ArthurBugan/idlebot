# Plan 019: Database Schema Design

> **Implementation Plan**

## Architecture

### Schema Overview
9 tables: players, hex_tiles, vehicles, cosmetics, voice_channels, market_listings, idle_gains, transactions, scheduled_functions_state

### Index Strategy
- SpacetimeDB auto-creates PK/FK indexes
- Explicit indexes on frequently queried fields (address, hex_id, player_id, seller_id)

### Replication Filters
- Server-side filtering by view radius (manhattan_distance ≤ 5 for hexes, ≤ 3 for players)
- Active-only filtering (voice channels, scheduled functions)

## Files to Create

### Chain (idlecore-chain)
- Create `src/schema.rs` — SpacetimeDB table definitions, indexes, filters

### Core (idlecore-core)
- Modify `src/lib.rs` — Import schema types

### Server (idlecore-server)
- Modify `src/types.rs` — Add SpacetimeDB table types

### Client (idlecore-client)
- Modify `src/lib.rs` — Import schema for subscriptions

## Dependencies
- Requires 013-wallet-auth (player creation)
- Requires 014-player-identity (player data model)
- Requires 015-scheduler-security (scheduler state)

## Testing Strategy
1. Unit test: Table schema compiles and validates
2. Integration test: Create player → query by address
3. Integration test: View radius filter returns correct hexes
4. Edge case: Empty tables handle gracefully

## Timeline
- **Estimate:** 2 days
- **Phase:** Phase 3 (Infrastructure)

## Ponytail Note
ponytail: Minimal schema definition — types only, no index implementation until first table is used in production.
