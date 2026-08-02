# Plan 001: Idle Gains Calculation

> **Implementation Plan**

## Architecture

### SpacetimeDB Tables
```sql
CREATE TABLE idle_gains (
    player_id UUID PRIMARY KEY,
    pending_xp BIGINT DEFAULT 0,
    pending_gold BIGINT DEFAULT 0,
    last_calculated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Scheduled Function
- **Name:** `idle_gains_scheduler`
- **Interval:** Every 5 minutes
- **Logic:** Iterate all players, calculate gains based on last login time, update pending_gains table

### Client Integration
- Show pending gains modal on login
- "Claim All" button to apply gains
- Disable claim if gains already applied this session

## Testing Strategy
1. Unit tests for gain calculation logic
2. Integration test for scheduled function
3. UI test for claim flow
4. Edge case: login immediately after offline (0 time)
5. Edge case: login after exactly 24h

## Dependencies
- Requires player table to exist
- Requires server scheduled function support

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
