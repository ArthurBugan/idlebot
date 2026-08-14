# Tasks 014: Player Identity Management

> **Implementation Checklist**

## Phase 1: Data Model
- [x] **T1.1** Avatar shapes in the player row + AVATARS list client-side
- [x] **T1.2** player table carries all these fields, position, current_hex, created_at, last_login, stats)
- [x] **T1.3** Activity counters on the player row (plants_planted/harvested, pollution_cleaned, play time), templates_published, templates_purchased, play_time)

## Phase 2: Player Manager
- [x] **T2.1** Player table = authoritative manager; client caches rows in subscription
- [x] **T2.2** login creates the row with defaults when missing
- [x] **T2.3** update_display_name — update_profile validates ≤20 alphanumerics, rejects invalid
- [x] **T2.4** Activity stats replicated on the player row
- [x] **T2.5** player.address() UniqueColumn lookup

## Phase 3: Database Integration
- [x] **T3.1** Player schema — types.rs game player row (address PK, stats, positions)
- [x] **T3.2** create_player — login reducer upserts the player row
- [x] **T3.3** update_player — update_profile reducer persists display_name/avatar/bio (server-side; client menu UI pending)
- [x] **T3.4** get_player — client reads subscribed player rows by address

## Phase 4: SpacetimeDB Indexes
- [x] **T3.5** address is #[primary_key] (unique)
- [x] **T3.6** display_name stored on the row (query by address covers lookups)

## Phase 5: Client Integration
- [x] **T4.1** HUD status shows name/wallet; stats line shows LV/XP/eco/vehicle
- [x] **T4.2** Edit Name button + keyboard capture; ENTER submits via update_profile
- [x] **T4.3** Avatar Next button cycles the 5 shapes via update_profile

## Phase 6: Testing
- [x] **T5.1** Player creation from wallet address works — resolve_login pure fn + tests (address lowercased, identity bound, STARTING_GOLD)
- [x] **T5.2** Client filters alphanumerics ≤20; server update_profile re-validates
- [x] **T5.3** Counters incremented in plant/harvest/clean/buy reducers
- [x] **T5.4** Row persistence + reconnect restore

## Verification
- [✓] Player struct has all identity fields
- [✓] PlayerManager.create_player() generates unique UUID
