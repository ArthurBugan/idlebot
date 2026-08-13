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
- [ ] **T2.8** Validate server-only execution

## Phase 4: Voice Cleanup Scheduler
- [x] **T3.1** voice_cleanup_scheduler — 60s interval
- [x] **T3.2** Empty-channel check — scheduled_voice_cleanup
- [x] **T3.3** 300s idleness destroy — scheduled_voice_cleanup
- [ ] **T3.4** Notify affected players

## Phase 5: Listing Cleanup Scheduler
- [ ] **T3.5** Implement listing_cleanup_scheduler() — runs every 1 hour
- [ ] **T3.6** Find expired listings (published > 30 days ago)
- [ ] **T3.7** Delete expired listings
- [ ] **T3.8** Notify sellers

## Phase 6: Audit Logging
- [ ] **T4.1** Create ScheduledActionLog struct
- [x] **T4.2** log_scheduled_action — scheduler::audit appends scheduled_log rows
- [x] **T4.3** All schedulers audited — audit() called in every tick handler

## Phase 7: Testing
- [ ] **T5.1** All 4 schedulers run on schedule
- [x] **T5.2** Server-authoritative — scheduled tables only tick module-side
- [ ] **T5.3** Idle gains calculate correctly
- [ ] **T5.4** Plants update growth status
- [ ] **T5.5** Voice channels clean up after 5 min emptiness
- [ ] **T5.6** Expired listings removed
- [ ] **T5.7** Audit logs recorded
- [ ] **T5.8** Error during scheduler doesn't crash

## Verification
- [✓] 4 scheduled functions defined with correct intervals
- [✓] Server-only validation exists
