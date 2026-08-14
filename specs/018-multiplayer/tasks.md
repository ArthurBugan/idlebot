# Tasks 018: Multiplayer Architecture

> **Implementation Checklist**

## Phase 1: Connection Flow
- [x] **T1.1** connect_to_spacetimedb — DEMO_WALLET login flow in Net::connect
- [x] **T1.2** player subscription — subscribe_to_all_tables
- [x] **T1.3** hex_tiles subscription — subscribe_to_all_tables
- [x] **T1.4** voice_channels subscription — subscribe_to_all_tables
- [x] **T1.5** market_listings subscription — subscribe_to_all_tables

## Phase 2: Player State Sync
- [x] **T2.1** player row carries all listed fields (address, hex_id, position_x/y, vehicle, status, last_login)
- [x] **T2.2** handle_player_state_update — sync_remote_players mirrors rows into Net.players + markers
- [x] **T2.3** Position broadcast — move_player_then sent while moving (0.75s throttle)
- [x] **T2.4** View radius filtering — player rows beyond 3 hexes skipped (markers + minimap dots)

## Phase 3: Movement Prediction
- [x] **T3.1** Local player moves immediately (local physics body = predicted state)
- [x] **T3.2** Frame-driven local movement; sync throttled to 0.75 s
- [x] **T3.3** sync_movement → move_player_then with intended speed
- [x] **T3.4** Server caps displacement/speed (SPEED_TOLERANCE) and recomputes hex; hex reconcile on hex flip

## Phase 4: Conflict Resolution
- [x] **T4.1** check_conflict — occupancy scan on hex flip in move_player
- [x] **T4.2** occupancy_resolution: closer-to-center wins
- [x] **T4.3** Tie-break by earlier last_login + unit tests

## Phase 5: Disconnect Handling
- [x] **T5.1** Disconnect — logout reducer flips player status; markers go grey/despawn
- [x] **T5.2** voice_cleanup_tick destroys stale channels (empty > 5 min)
- [x] **T5.3** leave() removes player; empty channels destroyed by cleanup tick
- [x] **T5.4** Reconnect — login restores the row incl. stored position/vehicle

## Phase 6: Testing
- [x] **T6.1** Wallet auth via login reducer binding identity (JWT layer omitted — SpacetimeDB identity token used instead)
- [x] **T6.2** Position updates arrive at server within 100ms — teleport echo RTT measured end-to-end (ServerLatency windowed avg, HUD "Net: X ms"), avg shown live; the 100 ms acceptance check runs against a live server
- [x] **T6.3** Speed-cap/displacement validation + hex recompute are authoritative
- [x] **T6.4** Occupancy rule enforced on hex entry (denied move → error)
- [x] **T6.5** voice cleanup tick destroys stale channels
- [x] **T6.6** Position/hex persisted on the row; restored at login
- [x] **T6.7** sync_remote_players culls > 3 hex away

## Verification
- [✓] PlayerState struct matches spec
- [✓] Conflict resolution resolves with proximity rule
