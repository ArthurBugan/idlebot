# Tasks 015: Scheduler Security

> **Implementation Checklist**

## Phase 1: Scheduler Infrastructure
- [ ] **T1.1** Define ScheduledFunction struct (name, interval, is_running, last_run, error_count)
- [ ] **T1.2** Create Scheduler struct with 4 scheduled functions
- [ ] **T1.3** Implement validate_scheduled_context() — check server-only
- [ ] **T1.4** Implement atomic_scheduled_update() — all-or-nothing

## Phase 2: Idle Gains Scheduler
- [ ] **T2.1** Implement idle_gains_scheduler() — runs every 5 min
- [ ] **T2.2** Calculate elapsed offline time per player
- [ ] **T2.3** Look up idle gains table (level bracket → XP/gold)
- [ ] **T2.4** Atomic update: deduct pending, add earned
- [ ] **T2.5** Log scheduled action

## Phase 3: Plant Updates Scheduler
- [ ] **T2.6** Implement plant_updates_scheduler() — runs every 10 sec
- [ ] **T2.7** Check plant maturity: if mature and not harvested, set Ready
- [ ] **T2.8** Validate server-only execution

## Phase 4: Voice Cleanup Scheduler
- [ ] **T3.1** Implement voice_cleanup_scheduler() — runs every 1 min
- [ ] **T3.2** Check each channel for emptiness
- [ ] **T3.3** If empty > 300 seconds, destroy channel
- [ ] **T3.4** Notify affected players

## Phase 5: Listing Cleanup Scheduler
- [ ] **T3.5** Implement listing_cleanup_scheduler() — runs every 1 hour
- [ ] **T3.6** Find expired listings (published > 30 days ago)
- [ ] **T3.7** Delete expired listings
- [ ] **T3.8** Notify sellers

## Phase 6: Audit Logging
- [ ] **T4.1** Create ScheduledActionLog struct
- [ ] **T4.2** Implement log_scheduled_action() — insert log entry
- [ ] **T4.3** Log all 4 scheduler functions on each run

## Phase 7: Testing
- [ ] **T5.1** All 4 schedulers run on schedule
- [ ] **T5.2** Functions are server-authoritative (reject client calls)
- [ ] **T5.3** Idle gains calculate correctly
- [ ] **T5.4** Plants update growth status
- [ ] **T5.5** Voice channels clean up after 5 min emptiness
- [ ] **T5.6** Expired listings removed
- [ ] **T5.7** Audit logs recorded
- [ ] **T5.8** Error during scheduler doesn't crash

## Verification
- [✓] 4 scheduled functions defined with correct intervals
- [✓] Server-only validation exists
