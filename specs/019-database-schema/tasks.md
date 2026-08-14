# Tasks 019: Database Schema Design

> **Implementation Checklist**

## Phase 1: Core Tables
- [x] **T1.1** Player table — address PK (deviation: PK = address, UUID not used), stats + position + status columns
- [x] **T1.2** HexTile table — hex_id PK, terrain, eco_rating, pollution, plant JSON
- [x] **T1.3** player_vehicle table — vehicle_id PK, durability, maintenance
- [x] **T1.4** player_cosmetic table — cosmetic_id PK + equipped flag
- [x] **T1.5** voice_channel table — hex_id PK, participants, occupancy
- [x] **T1.6** market_listing table — listing_id PK, escrow, dispute fields
- [x] **T1.7** idle_gain table — player PK, pending_xp/pending_gold
- [x] **T1.8** transaction table — ledger rows for every econ action
- [x] **T1.9** Scheduled tables — scheduled_idle_gains/plant_growth/voice_cleanup/market_cleanup/eco_maintenance + scheduled_log audit

## Phase 2: Indexes
- [x] **T2.1** Address index — PK on players.address (unique by definition)
- [x] **T2.2** hex_tile.hex_id is #[primary_key] (unique index)
- [x] **T2.3** btree index vehicle_by_player(player)
- [x] **T2.4** btree index cosmetic_by_player(player)
- [x] **T2.5** btree index listing_by_seller(seller)
- [x] **T2.6** idle_gain.player is #[primary_key]

## Phase 3: Replication Filters
- [x] **T3.1** Replication filtering — client culls hex tiles beyond view radius (3-hex)
- [x] **T3.2** Client sync_remote_players culls beyond 3-hex axial distance
- [x] **T3.3** Non-active channels ignored (voice cleanup tick + client skip)

## Phase 4: Integration
- [ ] **T4.1** Wire tables into idlecore-chain
- [ ] **T4.2** Wire subscriptions into idlecore-client
- [ ] **T4.3** Verify replication works end-to-end

## Phase 5: Testing
- [ ] **T5.1** All 9 tables defined with correct field types
- [x] **T5.2** address/hex_id/player are primary keys; FK btrees added
- [x] **T5.3** Client-side view-radius culling for hex tiles and players
- [ ] **T5.4** Foreign keys enforced
- [ ] **T5.5** Scheduled functions can read/write tables

## Verification
- [✓] 9 tables defined matching spec
- [✓] Indexes created for performance-critical queries
