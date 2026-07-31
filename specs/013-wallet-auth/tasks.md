# Tasks 013: Wallet Authentication

> **Implementation Checklist**

## Phase 1: Nonce Generation
- [ ] **T1.1** Implement generate_nonce() — return hex-encoded 32 random bytes
- [ ] **T1.2** Set nonce TTL (e.g., 5 minutes)
- [ ] **T1.3** Store nonce + expiration in memory/hash set

## Phase 2: Signature Verification
- [ ] **T1.4** Implement verify_signature(nonce, signature) → Result<Address>
- [ ] **T1.5** Parse signature string (from MetaMask formatted output)
- [ ] **T1.6** Recover signer address from signed message (EIP-191 prefix)
- [ ] **T1.7** Test with known test wallet addresses

## Phase 3: Session Management
- [ ] **T2.1** Define Session struct (address, created_at, expires_at, player_id)
- [ ] **T2.2** Implement generate_jwt() — create JWT with address + expiration
- [ ] **T2.3** Implement verify_jwt() — validate token and check expiration
- [ ] **T2.4** Implement create_session(address) — look up or create player

## Phase 4: Server Integration
- [ ] **T2.5** Register verify_login handler in server main.rs
- [ ] **T2.6** Implement create_player() for new wallet addresses
- [ ] **T2.7** Implement re-authenticate for existing players
- [ ] **T2.8** Handle wallet disconnection (token expiry)

## Phase 5: Client Integration
- [ ] **T3.1** Add wallet connection UI (MetaMask connect button)
- [ ] **T3.2** Display connected wallet address
- [ ] **T3.3** Implement request_login() flow (get nonce, display for signing)
- [ ] **T3.4** Implement signMessage() with wallet
- [ ] **T3.5** Submit signed login and display session token

## Phase 6: Security
- [ ] **T3.6** Rate limit login requests (e.g., 5 per minute per address)
- [ ] **T3.7** Secure nonce generation (CSPRNG, not predictable)
- [ ] **T3.8** Validate signature in < 100ms

## Phase 7: Testing
- [ ] **T4.1** Nonce generation produces unique values
- [ ] **T4.2** Signature verification recovers correct address
- [ ] **T4.3** First login creates player record
- [ ] **T4.4** Second login with same address re-authenticates
- [ ] **T4.5** Replay attack with old nonce rejected
- [ ] **T4.6** Expired token rejected

## Verification
- [✓] Nonce generation uses CSPRNG
- [✓] Signature verification recovers expected address from test key
