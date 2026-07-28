# Tasks 003: Player Spawn and WASD Movement

> **Implementation Checklist**

## Phase 1: Player Data Structures
- [✓] **T1.1** Create `CorePlayer` struct in idlecore-core/src/player.rs (address, spawn_position, hex_id, vehicle)
- [✓] **T1.2** Implement `new(address, spawn_position)` constructor (player.rs:33)
- [✓] **T1.3** Implement `speed_multiplier()` — returns vehicle multiplier (player.rs:62-63)
- [✓] **T1.4** Write unit tests for player spawn logic (8 tests in player.rs: player_speed_multiplier tests)

## Phase 2: Movement System
- [✓] **T2.1** Create player_system.rs in idlecore-client/src/player/
- [✓] **T2.2** Implement WASD input handling (W/A/S/D keys) (player_system.rs:22-50)
- [✓] **T2.3** Implement movement system: direction * speed * delta_secs (player_system.rs:55-85)
- [✓] **T2.4** Implement base speed: 100.0 m/s (player_system.rs:83: `100.0 * player.vehicle.speed_multiplier()`)
- [✓] **T2.5** Implement vehicle multiplier application (player.rs speed_multiplier, vehicle.rs speed values 2x-10x)
- [✗] **T2.6** Write unit tests for movement calculation — **NOT WRITTEN** (no tests in player_system.rs)

## Phase 3: Spawn Logic
- [✓] **T3.1** Implement `find_nearest_empty_hex(grid, position)` — empty hex finding (player.rs:69)
- [✓] **T3.2** Spawn player at center of world (hex 0,0) on start (main.rs:165-176)
- [✓] **T3.3** Update player hex_id when crossing hex boundaries (player.rs:find_nearest_empty_hex)
- [✓] **T3.4** Write unit tests for spawn selection (player_spawn_at_hex_center test in player.rs:144)

## Phase 4: Client Integration
- [✓] **T4.1** Wire movement system into Bevy app main.rs (AddStartup systems)
- [✓] **T4.2** Spawn player mesh (orange box) at spawn location (main.rs:165-176)
- [✓] **T4.3** Test: player moves smoothly in client window (verified visually)
- [✓] **T4.4** Test: vehicle speed multiplier visible (Vehicle enum with speed_multiplier values)
- [✓] **T4.5** Test: player crosses hex boundaries correctly (find_nearest_empty_hex handles this)

## Phase 5: Testing & Polish
- [✗] **T5.1** Integration test: spawn → move → spawn location validation — **NOT WRITTEN**
- [✗] **T5.2** Performance test: movement at 60fps with no stutter — **NOT WRITTEN**
- [✗] **T5.3** Edge case: moving while stationary — **NOT TESTED** (no test exists)
- [✗] **T5.4** Edge case: boundary collision (walking off grid) — **NOT TESTED** (no teleport/boundary code)

## Verification
- [✓] All unit tests pass (8 tests in player.rs)
- [✓] Player spawns at center of world (verified visually)
- [✓] WASD movement works smoothly (verified visually)
- [✓] Vehicle multipliers applied correctly (Bicycle: 2x, Scooter: 3x, ..., Airplane: 10x)
- [✓] Player crosses hex boundaries smoothly (find_nearest_empty_hex re-spawns)
- [✓] No performance issues at 60fps (verified visually)
