# Tasks 018: Multiplayer Architecture

> **Implementation Checklist**

## Phase 1: Connection Flow
- [ ] **T1.1** Implement connect_to_spacetimedb() — wallet auth → JWT → connection
- [ ] **T1.2** Subscribe to player_state table
- [ ] **T1.3** Subscribe to hex_tiles table
- [ ] **T1.4** Subscribe to voice_channels table
- [ ] **T1.5** Subscribe to market_listings table

## Phase 2: Player State Sync
- [ ] **T2.1** Define PlayerState struct (player_id, hex_id, position_x, position_y, velocity, vehicle_id, status, connected_at)
- [ ] **T2.2** Implement handle_player_state_update() — sync to local state
- [ ] **T2.3** Broadcast player position changes to other clients
- [ ] **T2.4** Implement view radius filtering (3-hex radius, online only)

## Phase 3: Movement Prediction
- [ ] **T3.1** Define PredictedMovement struct (local_position, local_hex)
- [ ] **T3.2** Implement movement prediction loop (every 100ms)
- [ ] **T3.3** Send predicted position to server
- [ ] **T3.4** Handle server correction (snap to authoritative position)

## Phase 4: Conflict Resolution
- [ ] **T4.1** Implement check_conflict() — distance check (hex_radius = 10.0)
- [ ] **T4.2** Implement proximity rule (closer to hex center wins)
- [ ] **T4.3** If equal distance, earlier connection wins

## Phase 5: Disconnect Handling
- [ ] **T5.1** Implement handle_player_disconnect() — mark Disconnecting
- [ ] **T5.2** Schedule cleanup in 5 seconds
- [ ] **T5.3** Close voice channel if in one
- [ ] **T5.4** Implement handle_player_reconnect() — validate JWT, restore position

## Phase 6: Testing
- [ ] **T6.1** Player connects via wallet auth → JWT → SpacetimeDB
- [ ] **T6.2** Position updates arrive at server within 100ms
- [ ] **T6.3** Server correction happens on divergence
- [ ] **T6.4** Two players on same hex resolved via proximity rule
- [ ] **T6.5** Voice channels auto-destroy on disconnect
- [ ] **T6.6** Reconnect restores last known position
- [ ] **T6.7** Client only receives nearby player updates (≤3 hex radius)

## Verification
- [✓] PlayerState struct matches spec
- [✓] Conflict resolution resolves with proximity rule
