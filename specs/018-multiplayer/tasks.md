# Tasks 018: Multiplayer Architecture

> **Implementation Checklist**

## Phase 1: Connection Flow
- [x] **T1.1** connect_to_spacetimedb — DEMO_WALLET login flow in Net::connect
- [x] **T1.2** player subscription — subscribe_to_all_tables
- [x] **T1.3** hex_tiles subscription — subscribe_to_all_tables
- [x] **T1.4** voice_channels subscription — subscribe_to_all_tables
- [x] **T1.5** market_listings subscription — subscribe_to_all_tables

## Phase 2: Player State Sync
- [ ] **T2.1** Define PlayerState struct (player_id, hex_id, position_x, position_y, velocity, vehicle_id, status, connected_at)
- [x] **T2.2** handle_player_state_update — sync_remote_players mirrors rows into Net.players + markers
- [x] **T2.3** Position broadcast — move_player_then sent while moving (0.75s throttle)
- [x] **T2.4** View radius filtering — player rows beyond 3 hexes skipped (markers + minimap dots)

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
