# Plan 008: Teleport Mechanic

> **Implementation Plan**

## Architecture

### Teleport System
- Click hex on minimap to select destination
- 100 Gold cost per teleport (or level-dependent)
- 60-second cooldown
- Server-authoritative execution
- Cooldown timer displayed in UI

### UI Integration
- Dual-mode minimap (local/global)
- Click hex to select source (first click) then destination (second click)
- Confirm button to execute teleport

## Files to Create/Modify

### Core (idlecore-core)
- `src/teleport.rs` — TeleportSystem struct, cost calculation, cooldown logic, execution

### Server (idlecore-server)
- `src/main.rs` — Register teleport reducer
- `src/progression.rs` — Teleport cost scales with level

### Client (idlecore-client)
- `src/minimap.rs` — Hex selection for teleport (if minimizing minimap)
- `src/teleport_ui.rs` — Cooldown display, confirm button
- `src/main.rs` — Wire teleport system

## Testing Strategy
1. Unit test: Teleport cost calculation (level 1 = 100G, level 2 = 200G, etc.)
2. Unit test: Cooldown timer logic
3. Integration test: Select hex → confirm → teleport → position update
4. Edge case: Insufficient gold

## Dependencies
- Depends on 003-player-spawn (player needs position tracking)
- Depends on 009-minimap (minimap hex selection)
- Depends on 010-economy (gold deduction)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
