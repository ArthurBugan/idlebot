# Tasks 001: Idle Gains Calculation

> **Implementation Checklist**

## Phase 1: Core Logic
- [ ] **T1.1** Create `IdleGains` struct in idlebot-core
- [ ] **T1.2** Implement `calculate_idle_gains()` function
- [ ] **T1.3** Write unit tests for all time brackets
- [ ] **T1.4** Create test cases for edge cases (0h, 24h, >24h)

## Phase 2: Server Integration
- [ ] **T2.1** Define `idle_gains` table schema
- [ ] **T2.2** Implement scheduled function `idle_gains_scheduler`
- [ ] **T2.3** Add function to SpacetimeDB scheduler
- [ ] **T2.4** Test scheduled function manually

## Phase 3: Client Integration
- [ ] **T3.1** Create `IdleGainsPanel` UI component
- [ ] **T3.2** Implement gain display logic
- [ ] **T3.3** Implement "Claim All" button
- [ ] **T3.4** Handle gain application on login
- [ ] **T3.5** Disable claim after application

## Phase 4: Testing & Polish
- [ ] **T4.1** Integration test: full offline → login → claim flow
- [ ] **T4.2** UI polish: animations, tooltips
- [ ] **T4.3** Error handling for failed calculations
- [ ] **T4.4** Performance test with 100+ players

## Verification
- [ ] All unit tests pass
- [ ] Scheduled function runs every 5 minutes
- [ ] Gains calculated correctly for all time brackets
- [ ] UI displays pending gains accurately
- [ ] Claim flow works end-to-end
- [ ] No race conditions in concurrent logins
