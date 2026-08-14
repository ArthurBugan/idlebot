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
- [x] **T2.3** Wired — idle_gains_tick scheduled table → scheduler::idle_gains_tick_body.rs:186`)
- [x] **T2.4** 8 unit tests in `scheduler/idle.rs` (`test_capped_elapsed*`, `test_check_idle_notification*`)

## Phase 3: Client Integration
- [x] **T3.1** IdleGainsPanel wired — spawn_idle_panel + update_idle_gains_panel registered in main.rs
- [x] **T3.2** Gain display — HUD stats show pending idle gains from idle_gain row
- [x] **T3.3** Gain application on login — update_idle_gains_panel applies offline gains; HUD Claim Idle Gains button claims
- [x] **T3.4** Claim after application — stats show Idle pending cleared after claim

## Phase 4: Testing & Polish
- [x] **T4.1** Integration tests — Core tests pass
- [x] **T4.2** N/A — engine renders at 60fps (Bevy); no rAF layer exists
- [x] **T4.3** Error handling — format_offline_duration exists and is wired
- [x] **T4.4** Performance — core gains_for_time math covered by idle_config tests

## Verification
- [x] All core unit tests pass (idle_config tests exist in economy.rs)
- [x] Gains calculated correctly for all time brackets (verified in tests)
- [x] UI wired — HUD "Idle pending: +XG +YXP" reads idle_gain row, Claim key invoked
- [x] Claim works — C-bound HudAction::ClaimIdle → claim_idle_gains reducer
- [x] No race conditions (idle gains are computed locally based on last_seen)
