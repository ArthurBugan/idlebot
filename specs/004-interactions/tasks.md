# Tasks 004: Basic Interactions (Plant, Harvest, Clean)

> **Implementation Checklist**

## Phase 1: Plant System
- [ ] **T1.1** Create PlantType enum in idlecore-core/src/plant.rs
- [ ] **T1.2** Create Plant struct with planted_at, growth_duration
- [ ] **T1.3** Implement Plant::is_mature() — check if grown
- [ ] **T1.4** Implement Plant::new(plant_type) — set growth duration
- [ ] **T1.5** Write unit tests for plant maturity checks

## Phase 2: Action Validation
- [ ] **T2.1** Create actions.rs in idlecore-core/src/
- [ ] **T2.2** Implement validate_plant(player, hex) — check gold, hex state
- [ ] **T2.3** Implement validate_harvest(hex) — check plant exists and mature
- [ ] **T2.4** Implement validate_clean(player, hex) — check gold, pollution
- [ ] **T2.5** Write unit tests for each validation function

## Phase 3: Action Execution
- [ ] **T3.1** Implement execute_plant(player, hex, plant_type) — spend gold, plant
- [ ] **T3.2** Implement execute_harvest(player, hex) — collect gold + XP, remove plant
- [ ] **T3.3** Implement execute_clean(player, hex) — spend gold, remove pollution
- [ ] **T3.4** Implement plant growth system (server scheduler or time check)
- [ ] **T3.5** Write unit tests for action execution

## Phase 4: Server Integration
- [ ] **T4.1** Add interact_hex reducer to server main.rs
- [ ] **T4.2** Implement interact_hex logic: validate → execute → update state
- [ ] **T4.3** Register interact_hex in server modules
- [ ] **T4.4** Test reducer with mock player and hex

## Phase 5: Client Integration
- [ ] **T5.1** Add interaction key binding (E key or click) in input.rs
- [ ] **T5.2** Implement interaction system in main.rs
- [ ] **T5.3** Update player gold/XP UI on action
- [ ] **T5.4** Test interaction in client window
- [ ] **T5.5** Test: plant → wait → harvest flow

## Phase 6: Testing & Polish
- [ ] **T6.1** Integration test: full plant → grow → harvest cycle
- [ ] **T6.2** Performance test: multiple players interacting simultaneously
- [ ] **T6.3** Edge case: interacting with non-adjacent hex
- [ ] **T6.4** Edge case: double harvest on same plant
- [ ] **T6.5** Visual feedback: plant grows, harvest animation

## Verification
- [ ] All unit tests pass
- [ ] Plant action validates correctly
- [ ] Harvest action works on mature plants only
- [ ] Clean action works on polluted hexes
- [ ] Gold/XP updated correctly after each action
- [ ] Plant growth progresses over time
- [ ] Client shows visual feedback for actions
