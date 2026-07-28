# Tasks 013: Wallet Authentication

> **Implementation Checklist**

## Phase 1: Authentication Flow
- [ ] **T1.1** Client requests login from server
- [ ] **T1.2** Server verifies wallet signature
- [ ] **T1.3** Generate JWT token for authenticated session
- [ ] **T1.4** Client sends JWT with each request

## Phase 2: Wallet Connection
- [ ] **T1.5** Connect to wallet provider (MetaMask, Phantom, etc.)
- [ ] **T1.6** Request wallet address from client
- [ ] **T1.7** Verify wallet signature against message
- [ ] **T1.8** Store wallet state in client

## Phase 3: Session Management
- [ ] **T1.9** Validate JWT on each request
- [ ] **T1.10** Refresh JWT when expired
- [ ] **T1.11** Handle session expiry gracefully
- [ ] **T1.12** Logout and invalidate token

## Phase 4: Security
- [ ] **T1.13** Store JWT in httpOnly cookie
- [ ] **T1.14** Implement token rotation
- [ ] **T1.15** Detect and prevent token theft

## Phase 5: Testing
- [✓] **T1.16** Test login with valid signature
- [✓] **T1.17** Test login with invalid signature
- [✓] **T1.18** Test JWT generation
- [✓] **T1.19** Test session expiry
- [✓] **T1.20** Test logout
