# Plan 014: Player Identity Management

> **Implementation Plan**

## Architecture

### Player Identity
- Wallet address (64-char hex) as unique player ID
- Display name (optional, up to 20 alphanumeric chars)
- Avatar selection (5 default: Tetrahedron, Cube, Sphere, Cylinder, Cone)
- Bio (optional)
- Activity statistics (plants_planted, plants_harvested, pollution_cleaned, etc.)

### SpacetimeDB Schema
- Player table with all identity fields
- Index on address (unique), index on display_name
- Player profile updates (no conflict resolution needed — single owner)

## Files to Create/Modify

### Server (idlecore-server)
- Modify `src/types.rs` — Add Player struct
- Modify `src/main.rs` — Add player management functions
- Create `src/player.rs` — PlayerManager with create, update_name, get_stats

### Core (idlecore-core)
- Modify `src/player.rs` — Add avatar type enum, identity fields

### Client (idlecore-client)
- Modify `src/player.rs` — Display player info, avatar

## Dependencies
- Requires 013-wallet-auth (player creation from wallet address)
- Requires 014-player-identity (player data model)

## Testing Strategy
1. Unit test: Player creation from wallet address
2. Unit test: Display name validation (max 20 chars, alphanumeric only)
3. Integration test: Player stats tracking across actions
4. Edge case: Name collision handled

## Timeline
- **Estimate:** 2 days
- **Phase:** Phase 3 (Identity)
