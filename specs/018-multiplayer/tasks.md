# Tasks 018: Multiplayer Architecture

> **Implementation Checklist**

## Phase 1: Connection Flow
- [✓] **T1.1** Wallet signature → JWT token exchange
- [✓] **T1.2** JWT → SpacetimeDB authenticated client connection
- [✓] **T1.3** Subscribe to relevant tables (players, hex_tiles, voice, market)
- [✓] **T1.4** Player state sync at 100ms intervals

## Phase 2: State Replication
- [✓] **T1.5** Define PlayerState struct in SpacetimeDB (position, hex, vehicle, cosmetics)
- [✓] **T1.6** Replicate only players within view radius (≤3 hexes)
- [✓] **T1.7** Hex occupancy tracking (server tracks which player is at which hex)
- [✓] **T1.8** Conflict resolution (proximity rule for 2 players on same hex)

## Phase 3: Movement & Prediction
- [✓] **T1.9** Client-side movement prediction (100ms interval)
- [✓] **T1.10** Server-side validation (within grid, no conflict, speed limit)
- [✓] **T1.11** Server correction when client diverges
- [✓] **T1.12** ServerConfirmation tracking for ordering

## Phase 4: Disconnect & Reconnect
- [✓] **T1.13** Server marks player offline on disconnect
- [✓] **T1.14** Wait 5 seconds before cleaning up
- [✓] **T1.15** Auto-destroy voice channel on disconnect
- [✓] **T1.16** Client reconnect with JWT, restore last position

## Phase 5: Voice Channel on Disconnect
- [✓] **T1.17** Remove player from voice channel on disconnect
- [✓] **T1.18** Destroy empty voice channels
