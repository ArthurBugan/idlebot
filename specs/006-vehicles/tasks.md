/usr/bin/bash: warning: setlocale: LC_ALL: cannot change locale (en_US.UTF-8): No such file or directory
# Tasks 006: Vehicles

> **Implementation Checklist**

## Phase 1: Vehicle Data & Types
- [x] **T1.1** Define VehicleType enum (None, Bicycle, Scooter, Motorcycle, Boat, Airplane) in vehicle.rs
- [x] **T1.2** Define speed multipliers (Bicycle: 2x, Scooter: 3x, Motorcycle: 5x, Boat: 4x, Airplane: 10x)
- [x] **T1.3** Define gold cost for each vehicle
- [x] **T1.4** Create Vehicle struct with type, speed_multiplier, gold_cost

## Phase 2: Vehicle Purchase (FR1)
- [x] **T2.1** purchase_vehicle() — buy_vehicle reducer deducts gold, inserts player_vehicle
- [x] **T2.2** Validate gold — spend_gold rejects insufficient balance
- [x] **T2.3** Mark purchased — player_vehicle row insert
- [x] **T2.4** Return success/failure — Result<String, String> → HUD log

## Phase 3: Vehicle Equipment (FR2)
- [x] **T3.1** Implement equip_vehicle() - set equipped flag
- [x] **T3.2** Implement unequip_vehicle() - set equipped=false
- [x] **T3.3** Track equipped vehicle per player (Player.vehicle field)
- [x] **T3.4** Inventory UI — HUD stats lists owned vehicles from player_vehicle cache

## Phase 4: Speed Application (FR3)
- [x] **T4.1** Apply speed multiplier to player movement (speed_multiplier() method)
- [x] **T4.2** Multiply base_speed by vehicle speed_multiplier (100.0 * multiplier)
- [x] **T4.3** Enforce maximum speed cap
- [x] **T4.4** Server validates vehicle state before applying speed

## Phase 5: Vehicle Display (FR4)
- [x] **T5.1** Vehicle rendered — colored ground plate under the player
- [x] **T5.2** Type indicator — floating label above the player (e.g. "Bicycle")
- [x] **T5.3** Equip state update — HUD vehicle stat mirrors authoritative row; speed multiplier applied

## Phase 6: Persistence (FR5)
- [x] **T6.1** Persist purchases — player_vehicle table
- [x] **T6.2** Persist equipped — player.vehicle + equipped flag
- [x] **T6.3** Restore on reconnect — sync_remote_players mirrors row.vehicle into ClientPlayer

## Phase 7: Testing
- [ ] **T7.1** Test purchase with sufficient gold
- [ ] **T7.2** Test purchase with insufficient gold
- [ ] **T7.3** Test equip/un equip cycle
- [ ] **T7.4** Test speed multiplier applied correctly
- [ ] **T7.5** Test vehicle persists across sessions
