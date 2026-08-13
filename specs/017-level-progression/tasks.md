# Tasks 017: Level Progression System

> **Implementation Checklist**

## Phase 1: Data Model
- [x] **T1.1** ProgressionState — player.level + player.total_xp columns
- [x] **T1.2** XP per action — plant=5, harvest=10, clean=15 (types.rs constants)
- [x] **T1.3** IdleGainXP — scheduled idle_gain rows with pending_xp by level bracket

## Phase 2: Level Calculation
- [x] **T2.1** Implement xp_for_next_level(level) → 100 * level^2 — server xp_to_next
- [x] **T2.2** Level calc — server add_xp loop over xp_to_next thresholds
- [x] **T2.3** Threshold math matches 100*level^2 (core progression.rs unit tests)
- [ ] **T2.4** Implement cache_latest_level() for O(1) UI lookup

## Phase 3: XP Bar Calculation
- [x] **T3.1** Implement xp_progress() — HUD shows current XP / threshold
- [x] **T3.2** Implement xp_remaining() — HUD shows remaining XP
- [x] **T3.3** Format as "X / Y XP to Level N" — HUD stats line

## Phase 4: Server Authority
- [x] **T4.1** Level calculation server-side — authoritative player row syncs level
- [x] **T4.2** apply_xp — add_xp applies gains server-side, level authoritative
- [x] **T4.3** check_level_up — sync_remote_players logs LEVEL UP! on advance

## Phase 5: Level-Up Event
- [x] **T5.1** LevelUp event — client HUD log on level advance
- [ ] **T5.2** Broadcast to all subscribed clients
- [ ] **T5.3** Client displays level-up notification

## Phase 6: Persistence
- [ ] **T5.4** Persist level and total_xp in database after each change
- [ ] **T5.5** Reconstruct level from total_xp on new session

## Phase 7: Testing
- [ ] **T6.1** Level correctly calculated from total XP through formula
- [ ] **T6.2** Current level rendered on player avatar
- [ ] **T6.3** XP bar reflects progress correctly
- [ ] **T6.4** Server broadcasts Level Up event
- [ ] **T6.5** Database persists level and total_xp atomically

## Verification
- [✓] xp_for_next_level formula: 100 * level^2
- [✓] calculate_level returns correct level for known XP values
