# Spec 015: Scheduler Security

> **Objective:** Implement secure scheduled functions for idle gains, plant updates, voice cleanup, and listing cleanup

## Problem Statement

Server-side scheduled functions handle critical game mechanics (idle gains, plant growth, voice cleanup). These must be secure, reliable, and prevent exploitation.

## Proposed Solution

- SpacetimeDB scheduled functions with strict validation
- Server-authoritative calculations (no client input)
- Rate limiting and error handling
- Audit logging for all scheduled actions

## Requirements

### Functional Requirements
1. FR1: Idle gains calculation runs every 5 minutes
2. FR2: Plant growth updates run every 10 seconds
3. FR3: Voice channel cleanup runs every 1 minute
4. FR4: Marketplace listing cleanup runs every 1 hour
5. FR5: All scheduled functions server-authoritative
6. FR6: Audit log for all scheduled actions

### Non-Functional Requirements
1. NFR1: Scheduled functions can't be called externally
2. NFR2: Atomic updates (all-or-nothing)
3. NFR3: Error handling without player impact
4. NFR4: Performance: < 100ms per scheduled function

## Design

### Scheduled Function Architecture
```rust
struct Scheduler {
    idle_gains: ScheduledFunction,
    plant_updates: ScheduledFunction,
    voice_cleanup: ScheduledFunction,
    listing_cleanup: ScheduledFunction,
}

struct ScheduledFunction {
    name: String,
    interval: Duration,
    is_running: bool,
    last_run: Option<Instant>,
    error_count: u32,
}
```

### 1. Idle Gains Scheduler
```rust
fn idle_gains_scheduler(db: &Database) {
    let players = db.get_all_players();
    
    for player in players {
        let elapsed = chrono::Utc::now() - player.last_login;
        let gains = calculate_idle_gains(elapsed);
        
        // Atomic update
        db.update_player_gains(
            player.id,
            gains.xp,
            gains.gold,
        );
        
        // Log action
        db.log_scheduled_action(
            "idle_gains",
            player.id,
            gains.xp,
            gains.gold,
        );
    }
}
```

### 2. Plant Growth Scheduler
```rust
fn plant_updates_scheduler(db: &Database) {
    let plants = db.get_all_plants();
    
    for plant in plants {
        if plant.is_mature() && !plant.is_harvested() {
            // Mark as ready for harvest
            plant.status = PlantStatus::Ready;
            db.update_plant(plant);
        }
    }
}
```

### 3. Voice Cleanup Scheduler
```rust
fn voice_cleanup_scheduler(db: &Database) {
    let channels = db.get_all_voice_channels();
    
    for channel in channels {
        if channel.is_empty() {
            let empty_duration = chrono::Utc::now() - channel.last_occupied;
            
            if empty_duration > Duration::from_secs(300) {
                // Destroy channel
                db.destroy_voice_channel(channel.id);
                
                // Notify players
                for player in channel.players {
                    db.notify_player(player, "Voice channel destroyed");
                }
            }
        }
    }
}
```

### 4. Listing Cleanup Scheduler
```rust
fn listing_cleanup_scheduler(db: &Database) {
    let now = chrono::Utc::now();
    let expired = db.get_expired_listings(now);
    
    for listing in expired {
        // Refund gold to seller? (No, already spent)
        db.delete_listing(listing.id);
        
        // Notify seller
        db.notify_player(listing.seller_id, "Listing expired");
    }
}
```

### Security Measures
```rust
// Validate scheduled function is server-only
fn validate_scheduled_context(ctx: &SchedulerContext) -> Result<()> {
    if ctx.is_client_call {
        return Err(SchedulerError::Unauthorized);
    }
    
    if !ctx.is_scheduled_call {
        return Err(SchedulerError::NotScheduled);
    }
    
    Ok(())
}

// Atomic transaction for scheduled updates
fn atomic_scheduled_update<F, T>(db: &Database, f: F) -> Result<T>
where
    F: FnOnce(&Database) -> Result<T>,
{
    let result = f(db);
    
    match result {
        Ok(val) => {
            db.commit_transaction()?;
            Ok(val)
        }
        Err(e) => {
            db.rollback_transaction()?;
            Err(e)
        }
    }
}
```

### Audit Logging
```rust
struct ScheduledActionLog {
    id: Uuid,
    function_name: String,
    timestamp: Instant,
    player_id: Option<UUID>,
    action_type: String,
    data: serde_json::Value,
}

fn log_scheduled_action(
    db: &Database,
    function_name: &str,
    player_id: Option<UUID>,
    action_type: &str,
    data: &serde_json::Value,
) {
    let log = ScheduledActionLog {
        id: Uuid::new_v4(),
        function_name: function_name.to_string(),
        timestamp: Instant::now(),
        player_id,
        action_type: action_type.to_string(),
        data: data.clone(),
    };
    
    db.insert_scheduled_log(log);
}
```

## Acceptance Criteria
- [ ] All 4 scheduled functions run on schedule
- [ ] Functions are server-authoritative
- [ ] Idle gains calculate correctly
- [ ] Plants update growth status
- [ ] Voice channels clean up after 5 min
- [ ] Expired listings removed
- [ ] Audit logs recorded
- [ ] Atomic updates (no partial state)
- [ ] Error handling doesn't crash scheduler

## Risks
- R1: Scheduler crash (need restart mechanism)
- R2: Large player count performance
- R3: Clock skew between servers

## Open Questions
- Q1: Should scheduler run on separate thread?
- Q2: Distributed scheduler for multiple servers?
- Q3: Configurable intervals per deployment?
