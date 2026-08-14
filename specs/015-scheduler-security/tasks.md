# Tasks 015: Scheduler Security

> **Implementation Checklist**

## Phase 1: Scheduler Infrastructure
- [x] **T1.1** ScheduledFunction — scheduled_* tables with interval + scheduled_log rows
- [x] **T1.2** Scheduler — register_all: idle gains, plant growth, voice cleanup, market cleanup, eco maintenance
- [x] **T1.3** Server-only execution — spacetimedb schedulers only run module-side
- [x] **T1.4** Atomic updates — reducer transactions per scheduled run

## Phase 2: Idle Gains Scheduler
- [x] **T2.1** idle_gains_scheduler — 300s interval (register_all)
- [x] **T2.2** Elapsed idle time — server computes from last_action_at
- [x] **T2.3** Idle gains lookup — level-bracketed pending XP/gold rows
- [x] **T2.4** Atomic claim — claim_idle_gains transacts gains
- [x] **T2.5** Scheduled logging — audit() appends scheduled_log rows

## Phase 3: Plant Updates Scheduler
- [x] **T2.6** plant_updates_scheduler — 10s sweep (register_all)
- [x] **T2.7** Maturity sweep — scheduled_plant_growth
- [x] **T2.8** Scheduled reducers are runtime-invocable only; bodies validate ownership

## Phase 4: Voice Cleanup Scheduler
- [x] **T3.1** voice_cleanup_scheduler — 60s interval
- [x] **T3.2** Empty-channel check — scheduled_voice_cleanup
- [x] **T3.3** 300s idleness destroy — scheduled_voice_cleanup
- [x] **T3.4** Row-state changes replicate to subscribers (deletion/escrow-release visible client-side)

## Phase 5: Listing Cleanup Scheduler
- [x] **T3.5** scheduled_market_cleanup runs hourly (3600s interval)
- [x] **T3.6** market::cleanup sweeps LISTING_DURATION_SECS (30d)
- [x] **T3.7** Expired rows deleted in cleanup()
- [x] **T3.8** tracing + listing replication; escrow release logged

## Phase 6: Audit Logging
- [x] **T4.1** scheduled_log table with function/timestamp/detail
- [x] **T4.2** log_scheduled_action — scheduler::audit appends scheduled_log rows
- [x] **T4.3** All schedulers audited — audit() called in every tick handler

## Phase 7: Testing
- [x] **T5.1** 5 schedulers registered: idle(5m), plant(10s), voice(1m), market(1h), eco(1h)
- [x] **T5.2** Server-authoritative — scheduled tables only tick module-side
- [x] **T5.3** gains_for_time bracket tests pass (idle_config)
- [x] **T5.4** plant_growth_tick recomputes maturity (visuals flip at maturity)
- [x] **T5.5** voice cleanup destroy ≥5min-empty channels
- [x] **T5.6** covered by market cleanup sweep
- [x] **T5.7** audit() writes scheduled_log per tick
- [x] **T5.8** Tick bodies use guarded per-row ops + tracing::warn, no panics

## Verification
- [✓] 4 scheduled functions defined with correct intervals
- [✓] Server-only validation exists
