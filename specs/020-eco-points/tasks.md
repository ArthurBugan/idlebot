# Tasks 020: Eco Points & Hex Rating System

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define EcoPointSource enum (CleanPollution, PlantTree, HarvestTree, DailyBonus)
- [ ] **T1.2** Define EcoPoints struct (total_earned, total_spent, current)
- [ ] **T1.3** Implement add_points() — route to correct source
- [ ] **T1.4** Implement spend_points() — prevent negative

## Phase 2: Hex Eco Rating
- [ ] **T2.1** Define HexEcoRating struct (rating 0-100, last_updated, decay_rate, eco_actions)
- [ ] **T2.2** Implement apply_action() — handle clean/plant/harvest/decay
- [ ] **T2.3** Implement decay_daily() — check elapsed ≥ 86400s, apply decay
- [ ] **T2.4** Implement get_eco_tint() — HSL green color based on rating
- [ ] **T2.5** Implement get_eco_title() — Eco Enthusiast (100+), Eco Warrior (500+), Eco Legend (1000+)

## Phase 3: Cosmetic Unlocks
- [ ] **T3.1** Define EcoCosmeticUnlock enum (EcoWarriorHat, EcoWarriorAura, EcoWarriorTrail)
- [ ] **T3.2** Implement check_eco_unlock() — verify EP threshold met

## Phase 4: Scheduler Integration
- [ ] **T4.1** Create eco_decay_scheduler() — runs daily, decays all hexes
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
