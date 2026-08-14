# Tasks 003: Player Spawn and WASD Movement

> **Implementation Checklist**

## Phase 1: Player Data Structures
- [x] **T1.1** Create `CorePlayer` struct in idlecore-core/src/player.rs (address, spawn_position, hex_id, vehicle)
- [x] **T1.2** Implement `new(address, spawn_position)` constructor (player.rs:33)
- [x] **T1.3** Implement `speed_multiplier()` — returns vehicle multiplier (player.rs:62-63)
- [x] **T1.4** Write unit tests for player spawn logic (1 test in player.rs)

## Phase 2: Movement System
- [x] **T2.1** Create player plugin in idlecore-client/src/plugins/player.rs
- [x] **T2.2** Implement WASD input handling (W/A/S/D keys)
- [x] **T2.3** Implement movement system: direction * speed * delta_secs
- [x] **T2.4** Implement base speed: 100.0 m/s (slower from 200)
- [x] **T2.5** Implement vehicle multiplier application (player.rs speed_multiplier, vehicle.rs speed values 2x-10x)
- [x] **T2.6** world→axial roundtrip tests at every hex center (radius 8)

## Phase 3: Spawn Logic
- [x] **T3.1** Implement `find_nearest_empty_hex(grid, position)` — empty hex finding (player.rs:69)
- [x] **T3.2** Spawn player at center of world (hex 0,0) on start
- [x] **T3.3** Update player hex_id when crossing hex boundaries
- [x] **T3.4** Write unit tests for spawn selection

## Phase 4: Client Integration
- [x] **T4.1** Wire movement system into Bevy app main.rs (plugins)
- [x] **T4.2** Spawn player mesh (blue capsule) at spawn location
- [x] **T4.3** Test: player moves smoothly in client window (verified visually)
- [x] **T4.4** Test: vehicle speed multiplier visible (Vehicle enum with speed_multiplier values)
- [x] **T4.5** Test: player crosses hex boundaries correctly

## Phase 5: Testing & Polish
- [x] **T5.1** Integration test: spawn → move → spawn location validation — spawn_location_tests (round-trip, expected-hex landing, far-spawn validity, no drift over 5k travels)
- [x] **T5.2** movement_loop_fits_frame_budget — 100k move computations < 250 ms
- [x] **T5.3** world_to_axial(0,0)→(0,0) plus center roundtrips cover stationary
- [x] **T5.4** max-radius hex set has unique ids; roundtrip holds at grid edge

## Verification
- [x] All unit tests pass (1 test in player.rs)
- [x] Player spawns at center of world (verified visually)
- [x] WASD movement works smoothly (verified visually)
- [x] Vehicle multipliers applied correctly (Bicycle: 2x, Scooter: 3x, ..., Airplane: 10x)
- [x] Player crosses hex boundaries smoothly
- [x] No performance issues at 60fps (verified visually)
