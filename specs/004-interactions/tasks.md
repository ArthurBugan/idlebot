# Tasks 004: Basic Interactions (Plant, Harvest, Clean)

> **Implementation Checklist**

## Phase 1: Plant System
- [x] **T1.1** Create `PlantType` enum in idlecore-core/src/plant.rs (Wheat, Corn, Tree, RareHerb)
- [x] **T1.2** Create `Plant` struct with planted_at, growth_duration
- [x] **T1.3** Implement `Plant::is_mature(now)` — check if grown
- [x] **T1.4** Implement `Plant::time_to_maturity(now)` — seconds to maturity
- [x] **T1.5** Write unit tests for plant maturity checks (3 tests in plant.rs)

## Phase 2: Action Validation
- [x] **T2.1** Create actions.rs in idlecore-core/src/
- [x] **T2.2** Implement `validate_plant(player, hex)` — check gold, hex state
- [x] **T2.3** Implement `validate_harvest(hex, now)` — check plant exists and mature
- [x] **T2.4** Implement `validate_clean(player, hex)` — check gold, pollution
- [x] **T2.5** Write unit tests for each validation function (13 tests in actions.rs)

## Phase 3: Action Execution
- [x] **T3.1** Implement `execute_plant(player, hex, plant_type, now)` — spend gold, plant
- [x] **T3.2** Implement `execute_harvest(player, hex, now)` — collect gold + XP, remove plant
- [x] **T3.3** Implement `execute_clean(player, hex)` — spend gold, remove pollution
- [x] **T3.4** Plant growth — maturity computed from planted_at + growth_time; scheduled_plant_growth sweep
- [x] **T3.5** Write unit tests for action execution (2 tests in actions.rs)

## Phase 4: Server Integration
- [x] **T4.1** Add `interact_hex` reducer to server main.rs
- [x] **T4.2** Implement `interact_hex` logic: validate → execute → update state
- [x] **T4.3** Register interact_hex in server modules
- [x] **T4.4** interaction_checks pure rules + mock-player tests (adjacent/cooldown/lock/full)

## Phase 5: Client Integration
- [x] **T5.1** Add interaction key binding (E key) in input.rs
- [x] **T5.2** Implement interaction system in interaction.rs (execute_interaction fn)
- [x] **T5.3** Update player gold/XP UI on action — HUD stats sync from authoritative player row
- [x] **T5.4** E-key actions wired to reducers; results visible in the HUD log
- [x] **T5.5** Plant lifecycle unit-tested (is_mature/time_remaining roundtrip in types.rs tests)

## Phase 6: Testing & Polish
- [x] **T6.1** Cycle logic covered by Plant is_mature/mock_plant_wait_harvest_flow tests (E2E needs live server)
- [ ] **T6.2** Performance test: multiple players interacting simultaneously — **NOT WRITTEN**
- [x] **T6.3** Verified — hex_distance check rejects beyond range 1
- [x] **T6.4** Verified — hex.plant guard + hex lock reject double harvest
- [x] **T6.5** Visual feedback — planted hexes show cones that turn golden at maturity; harvest/clean removes them

## Verification
- [x] All core unit tests pass (13 tests in actions.rs + plant.rs)
- [x] Plant action validates correctly
- [x] Harvest action works on mature plants only
- [x] Clean action works on polluted hexes
- [x] Gold/XP updated correctly after each action
- [x] Plant growth progresses over time
- [x] Visual feedback — plant cones / pollution discs / eco tints update in-world; log lines report results
