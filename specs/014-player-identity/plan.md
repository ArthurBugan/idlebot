# Plan 014: Player Identity Management

> **Implementation Plan**

## Architecture

### Player Data Model
- Wallet address as unique player ID (64-char hex)
- Player profile stored in SpacetimeDB
- Support for display name (optional, up to 20 chars), avatar (5 options), bio
- Activity statistics (play time, actions, etc.)

### Database Schema
```sql
CREATE TABLE player_identity (
    wallet_address TEXT PRIMARY KEY,
    display_name TEXT DEFAULT NULL,
    avatar TEXT DEFAULT 'tetrahedron',
    bio TEXT DEFAULT NULL,
    play_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX idx_player_name ON player_identity(display_name);
```

### Session Management
- JWT token with wallet_address as claim
- Session expiration: 24 hours
- Refresh token mechanism

## Files to Create/Modify

### Core (idlecore-core)
- `src/player.rs` — Player struct with identity fields

### Server (idlecore-server)
- `src/types.rs` — PlayerIdentityDbEntry table schema
- `src/main.rs` — Register player identity reducers

### Client (idlecore-client)
- `src/identity.rs` — Player profile data, avatar display
- `src/main.rs` — Wire identity system

## Testing Strategy
1. Unit test: Player identity creation on first login
2. Unit test: Display name validation (length, characters)
3. Unit test: Avatar selection
4. Integration test: Login → profile display → logout

## Dependencies
- Depends on 013-wallet-auth (wallet signature login)
- Depends on 019-database-schema (table schema)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
