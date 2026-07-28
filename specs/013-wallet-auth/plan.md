# Plan 013: Wallet Authentication

> **Implementation Plan**

## Architecture

### Authentication Flow
1. Client requests login from server
2. Server generates random nonce (32 bytes)
3. Client signs nonce with wallet
4. Client submits nonce + signature to server
5. Server verifies signature, recovers wallet address
6. Server creates/looks up player, generates JWT/session token
7. Client stores token for authenticated requests

### Signature Verification
- Ethereum-style signature recovery (EIP-191 typed messages)
- Server uses EIP-712 struct hashing for domain separation
- Domain separator: `{name: "idlebot", version: 1}`

### Nonce Generation
- Server-side CSPRNG (SpacetimeDB hash, not `time::rng` which is not secure)
- Nonce must be unique per login session

## Files to Create/Modify

### Core (idlecore-core)
- `src/auth.rs` — Signature verification, EIP-712 domain separator

### Chain (idlecore-chain)
- `src/auth.rs` — Wallet address type, session management

### Server (idlecore-server)
- `src/main.rs` — Register login, logout, wallet_change reducers
- `src/types.rs` — WalletAuthDbEntry table

### Client (idlecore-client)
- `src/wallet_auth.rs` — Wallet connection UI, nonce signing
- `src/main.rs` — Wire authentication flow

## Testing Strategy
1. Unit test: EIP-712 signature verification
2. Unit test: Nonce generation uniqueness
3. Unit test: Session token generation
4. Integration test: Full login flow (wallet sign → server verify → JWT)
5. Edge case: Replay attack (same nonce twice)

## Dependencies
- Requires 014-player-identity (player creation on first login)
- Requires 019-database-schema (player table schema)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
