# Spec 001: Idle Gains Calculation

> **Objective:** Implement offline XP and gold accumulation system

## Problem Statement

Players need to earn resources while offline to maintain engagement and reward returning players. The system must calculate gains based on elapsed time and accumulate them when the player logs back in.

## Proposed Solution

- Server-side scheduled function running every 5 minutes
- Calculate gains based on time intervals (1h, 6h, 12h, 24h)
- Store pending gains in player state
- Apply gains on next login or manual claim
- Cap at 24 hours maximum

## Requirements

### Functional Requirements
1. FR1: Calculate XP and Gold based on offline duration
2. FR2: Apply gains automatically on login
3. FR3: Display pending gains to player
4. FR4: Allow manual claim of pending gains
5. FR5: Cap gains at 24-hour maximum

### Non-Functional Requirements
1. NFR1: Server-side calculation (no client-side manipulation)
2. NFR2: Run every 5 minutes via SpacetimeDB scheduled function
3. NFR3: Atomic update of player state
4. NFR4: Handle concurrent logins gracefully

## Design

### Data Model
```rust
struct PlayerState {
    total_xp: u64,
    gold: u64,
    pending_xp: u64,
    pending_gold: u64,
    last_login: Instant,
    claimed_at: Option<Instant>,
}
```

### Gains Table
| Offline Duration | XP Gained | Gold Gained |
|-----------------|-----------|-------------|
| < 1 hour        | 10        | 5           |
| 1–6 hours       | 60        | 30          |
| 6–12 hours      | 100       | 50          |
| 12–24 hours     | 150       | 75          |
| Max: 24 hours   | 150       | 75          |

### Algorithm
```rust
fn calculate_idle_gains(last_login: Instant) -> (u64, u64) {
    let elapsed = chrono::Utc::now() - last_login;
    let hours = elapsed.num_hours();
    
    match hours {
        0..1 => (10, 5),
        1..6 => (60, 30),
        6..12 => (100, 50),
        12..24 => (150, 75),
        _ => (150, 75), // Cap at 24h
    }
}
```

## Acceptance Criteria
- [ ] Gains calculated correctly based on elapsed time
- [ ] Gains applied on login
- [ ] Pending gains UI displayed
- [ ] Manual claim functionality works
- [ ] 24-hour cap enforced
- [ ] Server-side calculation verified

## Risks
- R1: Concurrent login updates could cause race conditions
- R2: Large time gaps (>24h) need proper capping
- R3: Scheduled function reliability

## Open Questions
- Q1: Should players be able to speed up idle gains with premium currency?
- Q2: How to display gains progress (e.g., "12.5h / 24h")?
