# Plan 013: Wallet Authentication

> **Implementation Plan**

## Architecture

### Authentication Flow
1. Client requests login → Server generates random nonce
2. Client signs nonce with wallet (MetaMask, etc.)
3. Client submits nonce + signature → Server
4. Server verifies signature, recovers address
5. Server creates/looks up player, generates JWT/session token

### Session Management
- JWT with address + expiration (e.g., 24 hours)
- SpacetimeDB stores player records (existing)
- No password storage — wallet address is unique identifier

## Files to Create/Modify

### Core (idlecore-core)
- Modify `src/lib.rs` — Add wallet auth integration

### Chain (idlecore-chain)
- Modify `src/auth.rs` — Add signature verification, nonce generation

### Server (idlecore-server)
- Modify `src/main.rs` — Add verify_login handler
- Modify `src/types.rs` — Add Session struct

### Client (idlecore-client)
- Modify `src/main.rs` — Add wallet connection UI
- Modify `src/input.rs` — Add signMessage handler

## Dependencies
- Requires 014-player-identity (player creation from wallet address)
- Requires polygon wallet library (alloy or ethers)
- Requires JWT library

## Testing Strategy
1. Unit test: Nonce generation (32 bytes random)
2. Unit test: Signature verification (recover address matches)
3. Integration test: Full login flow (request → sign → submit → session)
4. Edge case: Replay attack (same nonce rejected)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** Phase 3 (Authentication + Security)
