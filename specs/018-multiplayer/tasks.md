# Tasks 018: Multiplayer Architecture

> **Implementation Checklist**

## Phase 1: Connection Flow
- [ ] **T1.1** Wallet signature → JWT token exchange
- [ ] **T1.2** JWT → SpacetimeDB authenticated client connection
- [ ] **T1.3** Subscribe to relevant tables (players, hex_tiles, voice, market)
- [ ] **T1.4** Player state sync at 100ms intervals

## Phase 2: State Replication
- [ ] **T2.1** Define PlayerState struct in SpacetimeDB (position, hex, vehicle, cosmetics)
- [ ] **T2.2** Replicate only players within view radius (≤3 hexes)
- [ ] **T2.3** Hex occupancy tracking (server tracks which player is at which hex)
- [ ] **T2.4** Conflict resolution (proximity rule for 2 players on same hex)

## Phase 3: Movement & Prediction
- [ ] **T3.1** Client-side movement prediction (100ms interval)
- [ ] **T3.2** Server-side validation (within grid, no conflict, speed limit)
- [ ] **T3.3** Server correction when client diverges
- [ ] **T3.4** ServerConfirmation tracking for ordering

## Phase 4: Disconnect & Reconnect
- [ ] **T4.1** Server marks player offline on disconnect
- [ ] **T4.2** Wait 5 seconds before cleaning up
- [ ] **T4.3** Auto-destroy voice channel on disconnect
- [ ] **T4.4** Client reconnect with JWT, restore last position

## Phase 5: Voice Channel on Disconnect
- [ ] **T5.1** Remove player from voice channel on disconnect
- [ ] **T5.2** Destroy empty voice channels
