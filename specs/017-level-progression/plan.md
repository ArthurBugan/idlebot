# Plan 017: Level Progression System

> **Implementation Plan**

## Architecture

### Progression Formula
- XP required to advance: `100 * level^2`
- Level 1: 0 XP
- Level 2: 100 XP
- Level 3: 400 XP
- Level 4: 900 XP
- Level 5: 1600 XP
- ...

### XP Contribution Mapping
| Activity | XP Contribution |
|---|---|
| Plant | +5 |
| Harvest | +10 |
| Clean | +15 |
| Idle Gains | See detailed table |

### Server Authority
- All level calculation on server
- Level-up events broadcast to all clients
- Atomic persistence of total_xp and current_level

## Files to Create/Modify

### Core (idlecore-core)
- `src/lib.rs` — Add ProgressionState struct

### Server (idlecore-server)
- `src/progression.rs` — Level calculation, level-up check, unlocks

### Client (idlecore-client)
- `src/progression.rs` — Display level and XP progress in UI
- `src/main.rs` — Wire progression reducer calls

## Testing Strategy
1. Unit test: xp_for_next_level(level) returns correct values
2. Unit test: calculate_level(total_xp) returns correct level
3. Integration test: Level-up event fires at correct XP thresholds
4. Edge case: Level cap (if applicable)

## Dependencies
- Depends on 001-idle-gains (idle XP earning)
- Depends on 004-interactions (action XP gains)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
