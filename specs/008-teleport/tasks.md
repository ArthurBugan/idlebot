# Tasks 008: Teleport Mechanic

> **Implementation Checklist**

## Phase 1: Core Teleport System
- [ ] **T1.1** Create TeleportSystem struct in idlecore-core/src/teleport.rs
- [ ] **T1.2** Implement teleport cost calculation (100G base, level-dependent)
- [ ] **T1.3** Implement cooldown logic (60 seconds)
- [ ] **T1.4** Implement teleport execution (teleport to target hex)
- [ ] **T1.5** Write unit tests for teleport cost calculation
- [ ] **T1.6** Write unit tests for cooldown logic

## Phase 2: Server Integration
- [ ] **T2.1** Add teleport reducer to server main.rs
- [ ] **T2.2** Implement teleport validation (gold check, cooldown check)
- [ ] **T2.3** Implement teleport execution (position update, gold deduction)
- [ ] **T2.4** Register teleport in server modules

## Phase 3: Client Integration
- [ ] **T3.1** Wire minimap hex selection for teleport
- [ ] **T3.2** Implement teleport UI (confirm button, cooldown display)
- [ ] **T3.3** Handle teleport animation/particle effect
- [ ] **T3.4** Test teleport in client window
- [ ] **T3.5** Test teleport cooldown timer display

## Phase 4: Testing & Polish
- [ ] **T4.1** Integration test: select hex → confirm → teleport
- [ ] **T4.2** Edge case: insufficient gold
- [ ] **T4.3** Edge case: teleport on cooldown
- [ ] **T4.4** Edge case: teleport to occupied hex
- [ ] **T4.5** Visual test: teleport animation plays

## Verification
- [ ] All unit tests pass
- [ ] Teleport cost calculated correctly (100G base, scales with level)
- [ ] Cooldown timer works (60 seconds)
- [ ] Teleport executes instantly
- [ ] Gold deducted correctly
- [ ] Player position updates after teleport
