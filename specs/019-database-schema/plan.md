# Plan 019: Database Schema

> **Implementation Plan**

## Architecture

### Core Tables
1. **players** — wallet_address, display_name, avatar, bio, level, total_xp, gold, eco_points
2. **hex_tiles** — hex_id, terrain, owner, pollution_level, eco_rating
3. **market_listings** — listing_id, seller, title, github_url, description, price, is_sold
4. **voice_channels** — hex_id, player_ids (JSON), created_at, last_occupied
5. **idle_gains** — player_id, pending_xp, pending_gold, last_calculated_at
6. **subscriptions** — user, premium_until, limit
7. **vehicles** — player_id, vehicle_type, equipped, purchased
8. **cosmetics** — player_id, category, type, purchased, equipped
9. **transactions** — player_id, timestamp, action, gold_change, eco_change

## Files to Create/Modify

### Server (idlecore-server)
- `src/types.rs` — All table schemas, DB entry types
- `src/main.rs` — Register table initializations

### Core (idlecore-core)
- `src/lib.rs` — Export DB types

## Testing Strategy
1. Verify table schema in SpacetimeDB
2. Test CRUD operations for each table
3. Performance test: concurrent writes

## Dependencies
- Independent (foundation spec)
- Required by all other specs

## Timeline
- **Estimate:** 1 day
- **Phase:** Foundation
