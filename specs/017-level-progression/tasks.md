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
- [x] **T2.4** Level cached client-side (ClientPlayer.level synced from row; computed via calculate_level server-side)

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
- [x] **T5.2** Table replication broadcasts player row changes to all clients
- [x] **T5.3** LEVEL UP! entry appended to the game log (economy.rs level-up path)

## Phase 6: Persistence
- [x] **T5.4** add_xp persists total_xp + level atomically on the player row
- [x] **T5.5** login/load recomputes level via calculate_level(total_xp)

## Phase 7: Testing
- [x] **T6.1** calculate_level/xp_for_next_level formula in progression.rs
- [x] **T6.2** HUD LV stat + player label shows level
- [x] **T6.3** XP x/y progress line in HUD
- [x] **T6.4** Replicated row + LEVEL_UP log broadcast
- [x] **T6.5** Single row update commits level+total_xp together

## Verification
- [✓] xp_for_next_level formula: 100 * level^2
- [✓] calculate_level returns correct level for known XP values
