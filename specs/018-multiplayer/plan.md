# Plan 018: Multiplayer Architecture

> **Implementation Plan**

## Architecture

### Connection Flow
1. Client connects to SpacetimeDB server
2. Server authenticates via JWT/session token
3. Client connected to replication tables

### State Synchronization
- Player state at 100ms intervals (position, hex, vehicle, cosmetics)
- View radius filtering (clients only receive nearby player updates)
- Server-authoritative hex occupancy rules

### Conflict Resolution
- Two players on same hex: use proximity (within hex radius)
- All state changes validated server-side before applying
- Automatic disconnect handling (5s grace period)

## Files to Create/Modify

### Server (idlecore-server)
- `src/main.rs` — Register multiplayer reducers (player state updates, hex occupancy)

### Client (idlecore-client)
- `src/multiplayer.rs` — Multiplayer connection, state sync, view radius
- `src/main.rs` — Wire multiplayer systems

### Core (idlecore-core)
- `src/player.rs` — Add multi-player fields (current_hex, is_online)

## Testing Strategy
1. Unit test: View radius filtering (only nearby players visible)
2. Unit test: Conflict resolution (two players same hex)
3. Integration test: 10 players spawning and moving independently
4. Edge case: Player disconnect mid-game

## Dependencies
- Depends on 003-player-spawn (player state)
- Depends on 002-hex-grid (hex occupancy)
- Depends on 019-database-schema (player table schema)

## Timeline
- **Estimate:** 3-5 days
- **Phase:** Post-MVP Multiplayer
