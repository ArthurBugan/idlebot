# Tasks 020: Eco Points & Hex Rating System

> **Implementation Checklist**

## Phase 1: Data Model
- [x] **T1.1** Sources — add_eco_points(ctx, p, amt, action) categorizes clean/plant/harvest
- [x] **T1.2** Balance — player.eco_points u32, clamped atomic updates
- [x] **T1.3** add_points — economy::add_eco_points with action routing
- [x] **T1.4** spend_points — economy::spend_eco_points rejects negative

## Phase 2: Hex Eco Rating
- [x] **T2.1** HexEcoRating — hex_tile.eco_rating (0-100 int on the row)
- [x] **T2.2** apply_action — clean/plant/harvest update hex state; hourly_eco_tick adjusts ratings
- [x] **T2.3** decay — hourly_eco_tick decays ratings on schedule
- [x] **T2.4** get_eco_tint — HUD shows hex eco rating (Lush/Healthy/Strained/Degraded)
- [x] **T2.5** get_eco_title — HUD eco_rank(): Scout/Enthusiast/Warrior/Legend thresholds

## Phase 3: Cosmetic Unlocks
- [x] **T3.1** Unlocks — cosmetics.rs gates Hat tier at ECO_WARRIOR_UNLOCK_EP=500
- [x] **T3.2** check_eco_unlock — equip_owned checks EP; HUD shows next-unlock hint

## Phase 4: Scheduler Integration
- [x] **T4.1** eco scheduler — hourly_eco_tick + weekly_audit in scheduler.rs
- [x] **T4.2** Atomic — clamp + single row update in add_eco_points/spend_eco_points
- [x] **T4.3** eco_maintenance_tick audited via scheduler::audit (scheduled_log)

## Phase 5: Transaction Logging
- [x] **T4.4** eco_transaction table + record_eco_tx on plant/harvest/clean
- [x] **T4.5** Ledger — record() audit entry per eco change (seen in game log)

## Phase 6: Client Display
- [x] **T5.1** HUD stats line shows eco points
- [x] **T5.2** HUD shows Eco rank by EP
- [x] **T5.3** Tint discs (lush/degraded bands) on world hexes
- [x] **T5.4** "Eco-Friendly" flag in HUD hex line

## Phase 7: Testing
- [x] **T6.1** Awarded per action (ECO_FOR_* in interactions.rs)
- [x] **T6.2** hex.eco_rating += RATING_FOR_* capped at 100
- [x] **T6.3** eco.rs hourly tick decays −1/day, floor 0
- [x] **T6.4** eco_band tint discs spawned in world_floor
- [x] **T6.5** HUD Eco-Friendly marker on 100+
- [x] **T6.6** ECO_WARRIOR_UNLOCK_EP gate in cosmetics.rs
- [x] **T6.7** record() ledger entries per eco transaction

## Verification
- [✓] EcoPoints struct has add_points/spend_points
- [✓] HexEcoRating decays correctly
- [✓] get_eco_tint returns HSL green color
