# Tasks 019: Database Schema Design

> **Implementation Checklist**

## Phase 1: Core Tables
- [ ] **T1.1** Define Player table (player_id UUID PK, address TEXT UNIQUE, display_name, avatar, level, total_xp, gold, eco_points, position, hex_q, hex_r, vehicle_id, cosmetics_hash, status, last_login, created_at)
- [ ] **T1.2** Define HexTile table (hex_q, hex_r, hex_s, hex_id u64 PK, terrain, eco_rating, has_plant, has_pollution, plant_type, elevation)
- [ ] **T1.3** Define Vehicle table (vehicle_id PK, player_id UUID FK, vehicle_type, purchased_at, equipped)
- [ ] **T1.4** Define Cosmetic table (cosmetic_id PK, player_id UUID FK, category, cosmetic_type, purchased_at, equipped)
- [ ] **T1.5** Define VoiceChannel table (channel_id UUID PK, hex_id, players, created_at, last_occupied, is_empty, peer_data)
- [ ] **T1.6** Define MarketListing table (listing_id UUID PK, seller_id UUID FK, title, description, github_url, price_usdt, category, published_at, expires_at, is_sold, buyer_id)
- [ ] **T1.7** Define IdleGain table (player_id UUID FK, pending_xp, pending_gold, last_calculated_at, claimed_at)
- [ ] **T1.8** Define Transaction table (transaction_id UUID PK, player_id UUID FK, timestamp, action_type, gold_change, xp_change, eco_points_change, balance_after)
- [ ] **T1.9** Define ScheduledFunctionState table (function_name, last_run_at, next_run_at, status, error_count)

## Phase 2: Indexes
- [ ] **T2.1** Create index on players(address) — unique
- [ ] **T2.2** Create index on hex_tiles(hex_id) — unique
- [ ] **T2.3** Create index on vehicles(player_id) — FK
- [ ] **T2.4** Create index on cosmetics(player_id) — FK
- [ ] **T2.5** Create index on market_listings(seller_id)
- [ ] **T2.6** Create index on idle_gains(player_id)

## Phase 3: Replication Filters
- [ ] **T3.1** Implement hex_tile_filter() — manhattan_distance ≤ 5
- [ ] **T3.2** Implement player_state_filter() — manhattan_distance ≤ 3
- [ ] **T3.3** Implement voice_channel_filter() — active only, distance ≤ 3

## Phase 4: Integration
- [ ] **T4.1** Wire tables into idlecore-chain
- [ ] **T4.2** Wire subscriptions into idlecore-client
- [ ] **T4.3** Verify replication works end-to-end

## Phase 5: Testing
- [ ] **T5.1** All 9 tables defined with correct field types
- [ ] **T5.2** Indexes created on address, hex_id, player_id
- [ ] **T5.3** Replication filters limit data to view radius
- [ ] **T5.4** Foreign keys enforced
- [ ] **T5.5** Scheduled functions can read/write tables

## Verification
- [✓] 9 tables defined matching spec
- [✓] Indexes created for performance-critical queries
