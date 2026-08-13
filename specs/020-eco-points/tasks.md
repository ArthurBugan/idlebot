# Tasks 020: Eco Points & Hex Rating System

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define EcoPointSource enum (CleanPollution, PlantTree, HarvestTree, DailyBonus)
- [ ] **T1.2** Define EcoPoints struct (total_earned, total_spent, current)
- [x] **T1.3** add_points — economy::add_eco_points with action routing
- [x] **T1.4** spend_points — economy::spend_eco_points rejects negative

## Phase 2: Hex Eco Rating
- [x] **T2.1** HexEcoRating — hex_tile.eco_rating (0-100 int on the row)
- [x] **T2.2** apply_action — clean/plant/harvest update hex state; hourly_eco_tick adjusts ratings
- [x] **T2.3** decay — hourly_eco_tick decays ratings on schedule
- [x] **T2.4** get_eco_tint — HUD shows hex eco rating (Lush/Healthy/Strained/Degraded)
- [ ] **T2.5** Implement get_eco_title() — Eco Enthusiast (100+), Eco Warrior (500+), Eco Legend (1000+)

## Phase 3: Cosmetic Unlocks
- [ ] **T3.1** Define EcoCosmeticUnlock enum (EcoWarriorHat, EcoWarriorAura, EcoWarriorTrail)
- [ ] **T3.2** Implement check_eco_unlock() — verify EP threshold met

## Phase 4: Scheduler Integration
- [x] **T4.1** eco scheduler — hourly_eco_tick + weekly_audit in scheduler.rs
- [ ] **T4.2** Implement atomic update (all-or-nothing)
- [ ] **T4.3** Log scheduled action

## Phase 5: Transaction Logging
- [ ] **T4.4** Create EcoTransaction struct (player_id, hex_id, action, points_earned, rating_before, rating_after)
- [ ] **T4.5** Log eco point changes to transaction table

## Phase 6: Client Display
- [ ] **T5.1** Display current eco points in UI
- [ ] **T5.2** Display eco title if unlocked
- [ ] **T5.3** Display eco rating on hex (color tint)
- [ ] **T5.4** Display "Eco-Friendly" marker on 100+ rating

## Phase 7: Testing
- [ ] **T6.1** Eco points awarded correctly on clean/plant/harvest
- [ ] **T6.2** Hex eco rating updates on eco actions
- [ ] **T6.3** Rating decays -1 per day for inactive hexes
- [ ] **T6.4** Eco rating displays as color tint on hexes
- [ ] **T6.5** Eco-friendly hexes (100+) unlock title
- [ ] **T6.6** Eco cosmetics unlock at 500 EP
- [ ] **T6.7** Eco transaction log recorded

## Verification
- [✓] EcoPoints struct has add_points/spend_points
- [✓] HexEcoRating decays correctly
- [✓] get_eco_tint returns HSL green color
