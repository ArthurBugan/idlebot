# Tasks 013: Wallet Authentication

> **Implementation Checklist**

## Phase 1: Authentication Flow
- [ ] **T1.1** Client requests login from server — **PARTIALLY DONE** (login reducer exists)
- [ ] **T1.2** Server generates random nonce (32 bytes) — **NOT IMPLEMENTED**
- [ ] **T1.3** Client signs nonce with wallet (secp256k1) — **NOT IMPLEMENTED**
- [ ] **T1.4** Server verifies signature — **NOT IMPLEMENTED**
- [ ] **T1.5** Server validates wallet address matches on-chain — **NOT IMPLEMENTED**
- [ ] **T1.6** Server stores player info in SpacetimeDB — **PARTIALLY DONE** (PlayerDbEntry exists)
- [ ] **T1.7** Return encrypted session token to client — **NOT IMPLEMENTED**

## Phase 2: Token Management
- [ ] **T2.1** Store session token server-side — **NOT IMPLEMENTED**
- [ ] **T2.2** Client sends token with every request — **NOT IMPLEMENTED**
- [ ] **T2.3** Token expiry check (24-hour limit) — **NOT IMPLEMENTED**
- [ ] **T2.4** Logout / token revocation — **PARTIALLY DONE** (logout reducer exists)

## Phase 3: Security
- [ ] **T3.1** Nonce cannot be predicted/reused — **NOT TESTED**
- [ ] **T3.2** Signature must be valid secp256k1 — **NOT TESTED**
- [ ] **T3.3** Brute force protection (rate limiting) — **NOT IMPLEMENTED**
- [ ] **T3.4** Graceful handling of invalid signature — **NOT IMPLEMENTED**

## Phase 4: Wallet Types
- [ ] **T4.1** Support Phantom wallet (browser extension) — **NOT IMPLEMENTED**
- [ ] **T4.2** Support Solana mobile wallet — **NOT IMPLEMENTED**
- [ ] **T4.3** Support CLI wallet — **NOT IMPLEMENTED**
