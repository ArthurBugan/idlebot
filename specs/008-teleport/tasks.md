# Tasks 008: Teleport Mechanic

> **Implementation Checklist**

## Phase 1: Server Teleport Logic
- [x] **T1.1** Define TeleportSystem struct (TeleportState) in teleport.rs
- [x] **T1.2** Implement can_teleport() method (check cooldown)
- [x] **T1.3** Implement execute_teleport() (deduct gold, update position, set cooldown)
- [x] **T1.4** Define TeleportError enum (InsufficientGold, OnCooldown, InvalidTarget, OutOfRange)

## Phase 2: Client Teleport UI
- [x] **T1.5** Define TeleportUI struct (selected_hex, destination_hex, cooldown_timer) - TeleportUi in ui.rs
- [x] **T1.6** Implement click hex → select destination logic - TeleportUi::populate()
- [x] **T1.7** Implement confirm teleport button - InteractionUi handler
- [x] **T1.8** Calculate teleport cost (100G * sqrt(level), capped at level^2) - teleport_cost()

## Phase 3: Teleport Animation
- [x] **T1.9** Create teleport particle effect (TeleportAnimation with ease-in-out)
- [x] **T1.10** Animate player movement to target hex - TeleportAnimation::tick()
- [x] **T1.11** Display cooldown timer in UI - cooldown_remaining()

## Phase 4: State Sync
- [x] **T1.12** Server broadcasts new player position after teleport - server_teleport() returns TeleportEvent
- [x] **T1.13** Update other players' view to show teleported player - TeleportEvent struct

## Phase 5: Testing
- [x] **T1.14** Test teleport with sufficient gold (test_execute_teleport_success)
- [x] **T1.15** Test teleport on cooldown (test_execute_teleport_insufficient_gold, test_teleport_state_cooldown)
- [x] **T1.16** Test teleport animates correctly (test_teleport_animation_full_cycle, test_teleport_animation_alpha_bounds)
- [x] **T1.17** Test server-authoritative teleport (prevents cheating) - server_teleport validates gold/cooldown
