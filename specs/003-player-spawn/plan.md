# Plan 003: Player Spawn and WASD Movement

> **Implementation Plan**

## Architecture

### Player Spawn
- Find nearest empty grass hex to spawn location
- Player spawns at hex center
- Set player hex_id and position

### Movement System
- WASD input handling in Bevy Update systems
- Base speed: 10 m/s
- Vehicle multipliers: 2x (Bicycle) to 10x (Airplane)
- Smooth movement interpolation (no teleportation)
- Boundary validation (don't walk off grid)

### Data Structures
```rust
struct Player {
    address: WalletAddress,
    position: Vec3,          // World position
    hex_id: u64,             // Current hex
    speed: f32,              // Current speed (base * multiplier)
    is_online: bool,
    owned_vehicle: Option<Vehicle>,
}
```

## Files to Create/Modify

### Client (idlecore-client)
- `src/player/player_system.rs` — Movement system, spawn logic
- `src/player.rs` — Player data structures, speed calculation
- `src/input.rs` — WASD input handling
- `src/main.rs` — Wire up movement system, spawn player at grid center

### Core (idlecore-core)
- `src/player.rs` — Player struct with position, hex_id, speed
- `src/lib.rs` — Export player module

## Testing Strategy
1. Unit test: player spawns at valid hex
2. Unit test: WASD movement changes position correctly
3. Unit test: vehicle multiplier applied to speed
4. Integration test: player moves across multiple hexes
5. Visual test: movement smooth in Bevy window

## Dependencies
- Depends on 002-hex-grid (needs grid for spawn location)
- Used by 004-interactions (player needs to be at hex to interact)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
