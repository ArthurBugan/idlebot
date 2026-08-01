# Tasks 008: Teleport Mechanic

> **Implementation Checklist**

## Phase 1: Server Teleport Logic
- [x] **T1.1** Define TeleportSystem struct (last_teleport, cooldown: Duration)
- [x] **T1.2** Implement can_teleport() method (check cooldown)
- [x] **T1.3** Implement execute_teleport() (deduct 100G, update position, set cooldown)
- [x] **T1.4** Define TeleportError enum (InsufficientGold, OnCooldown, InvalidTarget)

## Phase 2: Client Teleport UI
- [x] **T1.5** Define TeleportUI struct (selected_hex, destination_hex, cooldown_timer)
- [x] **T1.6** Implement click hex → select destination logic
- [x] **T1.7** Implement confirm teleport button
- [x] **T1.8** Calculate teleport cost (100G)

## Phase 3: Teleport Animation
- [x] **T1.9** Create teleport particle effect (simple Bevy sprite particles)
- [x] **T1.10** Animate player movement to target hex
- [x] **T1.11** Display cooldown timer in UI

## Phase 4: State Sync
- [x] **T1.12** Server broadcasts new player position after teleport
- [x] **T1.13** Update other players' view to show teleported player

## Phase 5: Testing
- [x] **T1.14** Test teleport with sufficient gold
- [x] **T1.15** Test teleport on cooldown
- [x] **T1.16** Test teleport animates correctly
- [x] **T1.17** Test server-authoritative teleport (prevents cheating)
