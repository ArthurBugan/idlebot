# Tasks 019: Database Schema

> **Implementation Checklist**

## Phase 1: Core Tables
- [✓] **T1.1** Define Player table (player_id, address, display_name, avatar, level, total_xp, gold, eco_points, position, hex, status) — **COMPLETE** (PlayerDbEntry exists)
- [✓] **T1.2** Define HexTile table (hex_id, center_x, center_y, terrain, plant, eco_rating, is_polluted) — **COMPLETE** (HexTileDbEntry exists)
- [✓] **T1.3** Define VoiceChannel table (hex_id, players, created_at, last_activity, is_active) — **COMPLETE** (VoiceChannelDbEntry exists, is_active added)
- [✓] **T1.4** Define MarketListing table (listing_id, seller, title, github_url, price_usdt, published_at, sold) — **COMPLETE** (MarketListingDbEntry exists)
- [ ] **T1.5** Define Vehicle table — **NOT IMPLEMENTED**
- [ ] **T1.6** Define Cosmetic table — **NOT IMPLEMENTED**
- [ ] **T1.7** Define Transaction table (ledger) — **NOT IMPLEMENTED**
- [ ] **T1.8** Define IdleGains table (player_id, pending_xp, pending_gold, last_calculated_at) — **COMPLETE** (IdleGainsEntry exists)
- [ ] **T1.9** Define ScheduledFunctionState table — **NOT IMPLEMENTED**

## Phase 2: Indexes
- [ ] **T2.1** Index on address for wallet auth lookups — **NOT IMPLEMENTED**
- [ ] **T2.2** Index on hex_id for hex occupancy queries — **NOT IMPLEMENTED**
- [ ] **T2.3** Index on player_id for player state updates — **NOT IMPLEMENTED**
- [ ] **T2.4** Replication filters limit client data to visible area — **NOT IMPLEMENTED**
- [ ] **T2.5** Foreign key relationships enforced — **NOT IMPLEMENTED**
