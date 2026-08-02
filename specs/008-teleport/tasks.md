# Tasks 008: Teleport Mechanic

> **Implementation Checklist**

## Phase 1: Server Teleport Logic
- [x] **T1.1** Define TeleportSystem struct (TeleportState) in teleport.rs
- [x] **T1.2** Implement can_teleport() method (check cooldown)
- [x] **T1.3** Implement execute_teleport() (deduct gold, update position, set cooldown)
- [x] **T1.4** Define TeleportError enum (InsufficientGold, OnCooldown, InvalidTarget, OutOfRange)

## Phase 2: Client Teleport UI
- [ ] **T1.5** Define TeleportUI struct (selected_hex, destination_hex, cooldown_timer)
- [ ] **T1.6** Implement click hex → select destination logic
- [ ] **T1.7** Implement confirm teleport button
- [ ] **T1.8** Calculate teleport cost (100G * sqrt(level), capped at level^2)

## Phase 3: Teleport Animation
- [ ] **T1.9** Create teleport particle effect (simple Bevy sprite particles)
- [ ] **T1.10** Animate player movement to target hex
- [ ] **T1.11** Display cooldown timer in UI

## Phase 4: State Sync
- [ ] **T1.12** Server broadcasts new player position after teleport
- [ ] **T1.13** Update other players' view to show teleported player

## Phase 5: Testing
- [x] **T1.14** Test teleport with sufficient gold (test_execute_teleport_success)
- [x] **T1.15** Test teleport on cooldown (test_execute_teleport_insufficient_gold, test_teleport_state_cooldown)
- [ ] **T1.16** Test teleport animates correctly
- [x] **T1.17** Test server-authoritative teleport (prevents cheating)
