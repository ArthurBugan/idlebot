# Plan 015: Scheduler Security

> **Implementation Plan**

## Architecture

### Scheduled Functions (SpacetimeDB)
1. **idle_gains** — Every 5 min: calculate idle XP/gold for offline players
2. **plant_updates** — Every 10 sec: check plant maturity, update status
3. **voice_cleanup** — Every 1 min: destroy empty voice channels
4. **listing_cleanup** — Every 1 hour: mark expired marketplace listings

### Security Measures
- Server-authoritative only (no client-triggered execution)
- Atomic updates (all-or-nothing transactions)
- Audit logging for all scheduled actions
- Error handling without player impact

## Files to Create/Modify

### Server (idlecore-server)
- Create `src/scheduler/idle.rs` — Idle gains calculation
- Create `src/scheduler/plant.rs` — Plant growth updates
- Create `src/scheduler/voice.rs` — Voice channel cleanup
- Create `src/scheduler/listing.rs` — Listing cleanup
- Modify `src/scheduler/mod.rs` — Register all schedulers
- Modify `src/main.rs` — Wire scheduler functions

## Dependencies
- Requires 001-idle-gains (idle gains calculation)
- Requires 004-interactions (plant state)
- Requires 005-voice-chat (voice channels)
- Requires 011-marketplace (marketplace listings)

## Testing Strategy
1. Unit test: Idle gains calculation for various time ranges
2. Unit test: Plant maturity check
3. Integration test: Voice channel destruction after 5 min emptiness
4. Integration test: Expired listing cleanup
5. Edge case: Error during scheduled function doesn't crash

## Timeline
- **Estimate:** 2-3 days
- **Phase:** Phase 3 (Security)
- **Blocked Until:** All target systems (idle gains, plants, voice, marketplace) must be complete
