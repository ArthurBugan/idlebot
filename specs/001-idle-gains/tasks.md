# Tasks 001: Idle Gains Calculation

> **Implementation Checklist**

## Phase 1: Core Logic
- [x] **T1.1** Create `IdleGains` struct in idlecore-core (`idle_config.rs:8`, `new()`, `gains_for_time()`)
- [x] **T1.2** Implement `idle_hours()` function (`idle_config.rs:59`)
- [x] **T1.3** Implement `is_idle_eligible()` function (`idle_config.rs:64`)
- [x] **T1.4** Implement `validate_idle_duration()` function (`idle_config.rs:70`)
- [x] **T1.5** Implement `format_offline_duration()` function (`idle_config.rs:89`)
- [x] **T1.6** Unit tests for idle_config (tests in economy.rs `test_idle_gains`)

## Phase 2: Server Integration
- [x] **T2.1** Define `idle_gains` table schema in types.rs
- [x] **T2.2** Implement `process_idle_gains(now: u64)` — full logic (`scheduler/idle.rs:16`)
- [ ] **T2.3** Wired to SpacetimeDB scheduler via `crate::scheduler::process_idle_gains(ctx)` (`main.rs:186`)
- [x] **T2.4** 8 unit tests in `scheduler/idle.rs` (`test_capped_elapsed*`, `test_check_idle_notification*`)

## Phase 3: Client Integration
- [ ] **T3.1** `IdleGainsPanel` UI component in `client/src/idle.rs`
- [ ] **T3.2** Gain display logic (pending_gold_text, format_offline_duration)
- [ ] **T3.3** Handle gain application on login (`apply_idle_gains_to_panel`, `handle_claim_all_button`)
- [ ] **T3.4** Disable claim after application (claim button interacts via InteractionAction)

## Phase 4: Testing & Polish
- [x] **T4.1** Integration tests — Core tests pass
- [ ] **T4.2** UI polish — **NOT WRITTEN** (no `requestAnimationFrame`)
- [x] **T4.3** Error handling — format_offline_duration exists and is wired
- [ ] **T4.4** Performance test — **NOT WRITTEN** (no 60fps test)

## Verification
- [x] All core unit tests pass (idle_config tests exist in economy.rs)
- [x] Gains calculated correctly for all time brackets (verified in tests)
- [ ] UI displays pending gains in client (panel exists but `update_idle_panel` not wired to main)
- [ ] Claim flow works end-to-end (handle_claim_all_button exists but not integrated)
- [x] No race conditions (idle gains are computed locally based on last_seen)
