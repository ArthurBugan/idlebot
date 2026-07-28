# Plan 015: Scheduler Security

> **Implementation Plan**

## Architecture

### Scheduled Function Architecture
- SpacetimeDB scheduled functions with strict validation
- Server-authoritative calculations (no client input)
- Rate limiting and error handling
- Audit logging for all scheduled actions

### Security Measures
- Validate scheduled function is server-only
- Atomic updates (all-or-nothing)
- Error handling without player impact
- Performance: < 100ms per scheduled function

### Audit Logging
- Track all scheduled actions (function name, timestamp, player_id, action_type, data)
- Log to dedicated table for debugging and monitoring

## Files to Create/Modify

### Server (idlecore-server)
- `src/scheduler/mod.rs` — Scheduler module with all scheduled functions
- `src/main.rs` — Register scheduled functions
- `src/scheduler/idle.rs` — Idle gains calculation (every 5 minutes)
- `src/scheduler/plant.rs` — Plant growth updates (every 10 seconds)
- `src/scheduler/voice.rs` — Voice channel cleanup (every 1 minute)
- `src/scheduler/listing.rs` — Marketplace listing cleanup (every 1 hour)

### Core (idlecore-core)
- `src/scheduler.rs` — Scheduler configuration (intervals, enabled/disabled flags)

## Testing Strategy
1. Unit test: Idle gains calculation
2. Unit test: Plant growth update
3. Unit test: Voice channel cleanup
4. Integration test: All scheduled functions run on schedule
5. Edge case: Scheduler crash recovery

## Dependencies
- Depends on 001-idle-gains (idle gains calculation)
- Depends on 004-interactions (action execution)
- Depends on 005-voice-chat (voice channel management)
- Depends on 011-marketplace (listing cleanup)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
