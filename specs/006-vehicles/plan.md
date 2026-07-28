# Plan 006: Vehicle System

> **Implementation Plan**

## Architecture

### Vehicle Data Structures
- 6 vehicle types: None (1x), Bicycle (2x), Scooter (3x), Motorcycle (5x), Boat (4x), Airplane (10x)
- Gold cost: 0 / 500 / 1000 / 2500 / 2000 / 10000
- Speed multiplier applied to player movement

### Purchase Logic
- Deduct gold from player
- Add vehicle to inventory
- Persist vehicle in server DB
- Display in UI inventory

### Level Unlocks
- Level 2: Bicycle unlock
- Level 3: Scooter unlock
- Level 5: Motorcycle unlock
- Level 7: Airplane unlock

## Files to Create/Modify

### Core (idlecore-core)
- `src/vehicle.rs` — Vehicle struct, VehicleType enum, purchase logic, speed_multiplier

### Server (idlecore-server)
- `src/progression.rs` — Vehicle unlock at level 2+ (linked to progression)
- `src/economy.rs` — Gold deduction for purchase

### Client (idlecore-client)
- `src/vehicle.rs` — Vehicle inventory UI, purchase button, equip logic
- `src/player/player_system.rs` — Apply speed_multiplier to movement
- `src/main.rs` — Wire vehicle UI systems

## Testing Strategy
1. Unit test: Vehicle speed_multiplier for all types
2. Unit test: Purchase deducts gold correctly
3. Unit test: Equip/unequip vehicle
4. Integration test: Purchase → equip → movement speed change
5. Edge case: Insufficient gold

## Dependencies
- Depends on 003-player-spawn (player struct needs vehicle field)
- Depends on 010-economy (gold management)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
