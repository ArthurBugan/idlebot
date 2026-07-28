# Tasks 015: Scheduler Security

> **Implementation Checklist**

## Phase 1: Scheduler Functions
- [✓] **T1.1** Scheduled function: calculate_idle_gains (5 min interval)
- [✓] **T1.2** Scheduled function: update_plants (1 min interval)
- [✓] **T1.3** Scheduled function: cleanup_voice_channels (every 10 min)
- [✓] **T1.4** Scheduled function: cleanup_old_listings (every hour)

## Phase 2: Security
- [ ] **T1.5** Only server can trigger scheduled functions
- [ ] **T1.6** Validate player data before processing
- [ ] **T1.7** Log all scheduled function executions
- [ ] **T1.8** Rate limit scheduled functions

## Phase 3: Error Handling
- [ ] **T1.9** Handle DB errors gracefully
- [ ] **T1.10** Retry failed executions
- [ ] **T1.11** Alert on repeated failures

## Phase 4: Testing
- [✓] **T1.12** Test idle gains calculation
- [✓] **T1.13** Test plant growth update
- [✓] **T1.14** Test voice channel cleanup
- [✓] **T1.15** Test listing cleanup
