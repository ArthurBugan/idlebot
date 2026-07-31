# Plan 017: Level Progression System

> **Implementation Plan**

## Architecture

### Progression Formula
XP requirement for next level: `100 * level^2`
- Level 1→2: 100 XP
- Level 2→3: 400 XP
- Level 3→4: 900 XP
- Level 4→5: 1600 XP
- ...

### Implementation
- Server-authoritative calculation
- O(1) cached level lookup for UI
- Incremental loop for initial calculation
- XP bar: `(total_xp - xp_at_previous_level) / xp_for_next_level`

### XP Sources
- Plant: +5 XP
- Harvest: +10 XP
- Clean: +15 XP
- Idle Gains: varies by level bracket

## Files to Create/Modify

### Core (idlecore-core)
- Modify `src/progression.rs` — Level calculation, XP bar calculation

### Server (idlecore-server)
- Modify `src/progression.rs` — Server-side level update, level-up event

### Client (idlecore-client)
- Modify `src/progression.rs` — Display level and XP bar

## Dependencies
- Requires 001-idle-gains (idle XP earning)
- Requires 014-player-identity (player state)

## Testing Strategy
1. Unit test: calculate_level(total_xp) matches formula
2. Unit test: xp_for_next_level(1)=100, xp_for_next_level(10)=10000
3. Integration test: Level-up event fires at correct thresholds
4. Edge case: XP overflow, very high levels

## Timeline
- **Estimate:** 1-2 days
- **Phase:** Phase 3 (Progression)
