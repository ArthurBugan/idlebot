# Tasks 006: Vehicles

> **Implementation Checklist**

## Phase 1: Vehicle Data & Types
- [ ] **T1.1** Define VehicleType enum (None, Bicycle, Scooter, Motorcycle, Boat, Airplane)
- [ ] **T1.2** Define speed multipliers for each vehicle type
- [ ] **T1.3** Define gold cost for each vehicle
- [ ] **T1.4** Create Vehicle struct with type, speed_multiplier, gold_cost

## Phase 2: Vehicle Purchase (FR1)
- [ ] **T2.1** Implement purchase_vehicle() - deduct gold, add to player inventory
- [ ] **T2.2** Validate player has enough gold
- [ ] **T2.3** Mark vehicle as purchased
- [ ] **T2.4** Return success/failure to client

## Phase 3: Vehicle Equipment (FR2)
- [ ] **T3.1** Implement equip_vehicle() - set equipped flag
- [ ] **T3.2** Implement unequip_vehicle() - set equipped=false
- [ ] **T3.3** Track equipped vehicle per player
- [ ] **T3.4** UI shows vehicle inventory

## Phase 4: Speed Application (FR3)
- [ ] **T4.1** Apply speed multiplier to player movement
- [ ] **T4.2** Multiply base_speed by vehicle speed_multiplier
- [ ] **T4.3** Enforce maximum speed cap
- [ ] **T4.4** Server validates vehicle state before applying speed

## Phase 5: Vehicle Display (FR4)
- [ ] **T5.1** Render vehicle on player character (top-down)
- [ ] **T5.2** Show vehicle icon/type indicator
- [ ] **T5.3** Update visual when player equips/un equips

## Phase 6: Persistence (FR5)
- [ ] **T6.1** Store purchased vehicles in DB
- [ ] **T6.2** Store equipped vehicle in DB
- [ ] **T6.3** Restore vehicle state on reconnect

## Phase 7: Testing
- [ ] **T7.1** Test purchase with sufficient gold
- [ ] **T7.2** Test purchase with insufficient gold
- [ ] **T7.3** Test equip/un equip cycle
- [ ] **T7.4** Test speed multiplier applied correctly
- [ ] **T7.5** Test vehicle persists across sessions
