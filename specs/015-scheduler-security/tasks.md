# Tasks 015: Scheduler Security

> **Implementation Checklist**

## Phase 1: Scheduler Functions
- [ ] **T1.1** Scheduled function: calculate_idle_gains — **PARTIALLY DONE** (idle.rs exists)
- [ ] **T1.2** Scheduled function: update_plants — **NOT IMPLEMENTED**
- [ ] **T1.3** Scheduled function: cleanup_voice_channels — **PARTIALLY DONE** (registered)
- [ ] **T1.4** Scheduled function: cleanup_expired_listings — **NOT IMPLEMENTED**

## Phase 2: Security Validation
- [ ] **T2.1** Server-authoritative calculations — **NOT IMPLEMENTED**
- [ ] **T2.2** Player seed cannot be modified client-side — **NOT IMPLEMENTED**
- [ ] **T2.3** Time boundaries checked against server clock — **NOT IMPLEMENTED**
- [ ] **T2.4** Input validation on all parameters — **PARTIALLY DONE** (some validation)
- [ ] **T2.5** Reentrancy protection — **NOT IMPLEMENTED**
- [ ] **T2.6** Resource usage limits per function — **NOT IMPLEMENTED**

## Phase 3: Testing
- [ ] **T3.1** Test: idle gains calculation — **NOT TESTED**
- [ ] **T3.2** Test: time manipulation attempts — **NOT TESTED**
- [ ] **T3.3** Test: empty schedule execution — **NOT TESTED**
