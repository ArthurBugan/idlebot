# Tasks 017: Level Progression System

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define ProgressionState struct (current_level, total_xp)
- [ ] **T1.2** Define XP contribution per action (plant=5, harvest=10, clean=15)
- [ ] **T1.3** Define IdleGainXP struct (level bracket → XP amount)

## Phase 2: Level Calculation
- [ ] **T2.1** Implement xp_for_next_level(level) → 100 * level^2
- [ ] **T2.2** Implement calculate_level(total_xp) — incremental loop
- [ ] **T2.3** Verify: calculate_level(0)=1, calculate_level(100)=2, calculate_level(500)=3
- [ ] **T2.4** Implement cache_latest_level() for O(1) UI lookup

## Phase 3: XP Bar Calculation
- [ ] **T3.1** Implement xp_progress() → current_level / xp_for_next_level
- [ ] **T3.2** Implement xp_remaining() → xp_for_next_level - current_level
- [ ] **T3.3** Format as "X / Y XP to Level N"

## Phase 4: Server Authority
- [ ] **T4.1** Move level calculation to server-side
- [ ] **T4.2** Implement apply_xp(gained) — server calls calculate_level
- [ ] **T4.3** Implement check_level_up() — return new_level if advanced

## Phase 5: Level-Up Event
- [ ] **T5.1** Emit LevelUp event on advancement
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
