# Plan 004: Basic Interactions (Plant, Harvest, Clean)

> **Implementation Plan**

## Architecture

### Interaction Range
- 1 hex radius (10 meters)
- Detect hex in front of or under player
- Click or key press to interact

### Actions
```rust
enum HexAction {
    Plant(PlantType),   // Cost: 10G, Gain: 5 XP
    Harvest,            // Free, Gain: 15G + 10 XP
    CleanPollution,     // Cost: 20G, Gain: 20G + 15 XP
}
```

### Validation
- Plant: player has gold, hex is empty
- Harvest: hex has mature plant
- Clean: hex is polluted, player has gold

### Plant Growth
```rust
struct Plant {
    plant_type: PlantType,
    planted_at: u64,    // Unix timestamp
    growth_duration: Duration,  // Time to mature
}

enum PlantType {
    Wheat(Duration::from_secs(3600)),   // 1 hour
    Corn(Duration::from_secs(5400)),    // 1.5 hours
    Tree(Duration::from_secs(21600)),   // 6 hours
    RareHerb(Duration::from_secs(43200)), // 12 hours
}
```

## Files to Create/Modify

### Core (idlecore-core)
- `src/plant.rs` — Plant struct, PlantType enum, growth logic
- `src/actions.rs` — Action validation, execution
- `src/economy.rs` — Gold spending, XP gaining
- `src/lib.rs` — Export new modules

### Server (idlecore-server)
- `src/world.rs` — interact_hex reducer
- `src/main.rs` — Register interact_hex reducer

### Client (idlecore-client)
- `src/input.rs` — Interaction key binding (E or click)
- `src/player.rs` — Update player state on action
- `src/main.rs` — Wire interaction system

## Testing Strategy
1. Unit test: plant validation (no gold = fail)
2. Unit test: harvest validation (no plant = fail)
3. Unit test: clean validation (not polluted = fail)
4. Unit test: plant growth timing
5. Integration test: full plant → grow → harvest flow
6. Server test: interaction reducer updates state

## Dependencies
- Depends on 003-player-spawn (player needs to be at hex)
- Depends on 002-hex-grid (needs grid for hex location)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
