# Tasks 014: Player Identity Management

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define AvatarType enum (Tetrahedron, Cube, Sphere, Cylinder, Cone)
- [ ] **T1.2** Define Player struct (id, address, display_name, avatar, bio, level, total_xp, gold, eco_points, position, current_hex, created_at, last_login, stats)
- [ ] **T1.3** Define PlayerStats struct (level, total_xp, plants_planted, plants_harvested, pollution_cleaned, templates_published, templates_purchased, play_time)

## Phase 2: Player Manager
- [ ] **T2.1** Create PlayerManager struct (players HashMap<UUID, Player>)
- [ ] **T2.2** Implement create_player(address) — generate UUID, set defaults
- [ ] **T2.3** Implement update_display_name() with validation (≤20 chars, alphanumeric)
- [ ] **T2.4** Implement get_player_stats() — return PlayerStats
- [ ] **T2.5** Implement get_player_by_address()

## Phase 3: Database Integration
- [ ] **T3.1** Create SpacetimeDB player table schema (matches spec)
- [ ] **T3.2** Implement create_player_spacetimedb() — insert new player
- [ ] **T3.3** Implement update_player_spacetimedb() — update display_name, avatar, bio
- [ ] **T3.4** Implement get_player_spacetimedb() — look up by address

## Phase 4: SpacetimeDB Indexes
- [ ] **T3.5** Create index on players.address (unique)
- [ ] **T3.6** Create index on players.display_name

## Phase 5: Client Integration
- [ ] **T4.1** Display player profile after login (name, avatar, level, XP)
- [ ] **T4.2** Add display name edit button
- [ ] **T4.3** Add avatar selection UI

## Phase 6: Testing
- [ ] **T5.1** Player creation from wallet address works
- [ ] **T5.2** Display name validation (too long, invalid chars rejected)
- [ ] **T5.3** Player stats tracked correctly across actions
- [ ] **T5.4** Data persists across sessions

## Verification
- [✓] Player struct has all identity fields
- [✓] PlayerManager.create_player() generates unique UUID
