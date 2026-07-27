# Tasks 003: Player Spawn and WASD Movement

> **Implementation Checklist**

## Phase 1: Player Data Structures
- [ ] **T1.1** Create Player struct in idlecore-core/src/player.rs
- [ ] **T1.2** Implement Player::spawn_at(hex_id, position)
- [ ] **T1.3** Implement Player::speed_multiplier(owned_vehicle)
- [ ] **T1.4** Write unit tests for player spawn logic

## Phase 2: Movement System
- [ ] **T2.1** Create player_system.rs in idlecore-client/src/player/
- [ ] **T2.2** Implement WASD input handling (W/A/S/D keys)
- [ ] **T2.3** Implement movement system: position += direction * speed * dt
- [ ] **T2.4** Implement base speed: 10.0 m/s
- [ ] **T2.5** Implement vehicle multiplier application
- [ ] **T2.6** Write unit tests for movement calculation

## Phase 3: Spawn Logic
- [ ] **T3.1** Implement find_nearest_empty_grass_hex(grid, position)
- [ ] **T3.2** Spawn player at center of world (hex 0,0) on start
- [ ] **T3.3** Update player hex_id when crossing hex boundaries
- [ ] **T3.4** Write unit tests for spawn selection

## Phase 4: Client Integration
- [ ] **T4.1** Wire movement system into Bevy app main.rs
- [ ] **T4.2** Spawn player mesh (orange box) at spawn location
- [ ] **T4.3** Test: player moves smoothly in client window
- [ ] **T4.4** Test: vehicle speed multiplier visible
- [ ] **T4.5** Test: player crosses hex boundaries correctly

## Phase 5: Testing & Polish
- [ ] **T5.1** Integration test: spawn → move → spawn location validation
- [ ] **T5.2** Performance test: movement at 60fps with no stutter
- [ ] **T5.3** Edge case: moving while stationary
- [ ] **T5.4** Edge case: boundary collision (walking off grid)

## Verification
- [ ] All unit tests pass
- [ ] Player spawns at center of world
- [ ] WASD movement works smoothly
- [ ] Vehicle multipliers applied correctly
- [ ] Player crosses hex boundaries smoothly
- [ ] No performance issues at 60fps
