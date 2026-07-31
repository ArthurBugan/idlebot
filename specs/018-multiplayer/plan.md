# Plan 018: Multiplayer Architecture

> **Implementation Plan**

## Architecture

### Connection Flow
1. Wallet auth → JWT → SpacetimeDB client connection
2. Subscribe to replication tables (players, hex_tiles, voice_channels, market_listings)
3. Server-side view radius filtering (3-hex radius)

### State Sync
- Position updates at 100ms intervals
- Client-side movement prediction + server correction
- Conflict resolution: proximity rule (within hex radius)

### Disconnect/Reconnect
- Server marks Disconnecting on disconnect
- 5-second grace period, then cleanup
- Reconnect: validate JWT, restore position, mark Online

## Files to Create/Modify

### Server (idlecore-server)
- Modify `src/main.rs` — Connection flow, subscription management
- Create `src/multiplayer.rs` — State sync, conflict resolution
- Modify `src/voice.rs` — Voice channel management on disconnect

### Client (idlecore-client)
- Modify `src/lib.rs` — Subscribe to replication tables
- Modify `src/player.rs` — Movement prediction system
- Modify `src/world/hex_renderer.rs` — Show other players

## Dependencies
- Requires 013-wallet-auth (connection flow)
- Requires 014-player-identity (player state)
- Requires 005-voice-chat (voice channels)
- Requires 009-minimap (player positions)

## Testing Strategy
1. Unit test: Hex occupancy conflict resolution
2. Unit test: View radius filtering (3-hex radius)
3. Integration test: 2+ players in same hex
4. Edge case: Rapid disconnect/reconnect cycles

## Timeline
- **Estimate:** 3-5 days
- **Phase:** Phase 3 (Multiplayer)
- **Blocked Until:** Most specs already have core code (player, hex grid, voice)
