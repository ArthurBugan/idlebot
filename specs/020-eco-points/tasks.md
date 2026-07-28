# Tasks 020: Eco Points System

> **Implementation Checklist**

## Phase 1: Eco Rating Calculation
- [✓] **T1.1** Calculate eco rating for player (1000 base - unharvested crops) — **COMPLETE** (calculate_eco_rating)
- [✓] **T1.2** Calculate eco rating for hex (eco point count) — **COMPLETE** (calculate_hex_eco_rating)
- [✓] **T1.3** Set eco rating via reduction (min: 0, max: 500) — **COMPLETE** (set_eco_rating)
- [✓] **T1.4** Max eco points per action — **COMPLETE** (eco_points = min(500 - current, action_max_eco))
- [✓] **T1.5** Output eco points earned for each action — **COMPLETE** (in ActionResult)

## Phase 2: Server Enforcement
- [✓] **T1.6** Server validates eco points limit — **COMPLETE** (NFR1)
- [✓] **T1.7** Server validates eco points cap — **COMPLETE** (NFR2)
- [✓] **T1.8** Server recalculates hex rating on harvest — **COMPLETE** (NFR3)

## Phase 3: Client Display
- [✓] **T1.9** Render current eco rating on player — **COMPLETE** (eco_rating: u16 in Player state)
- [✓] **T1.10** Display eco points earned per action — **COMPLETE** (in ActionResult)

## Phase 4: Database Persistence
- [✓] **T1.11** Store eco rating in PlayerDbEntry — **COMPLETE** (eco_rating field)
- [✓] **T1.12** Store total eco points in PlayerDbEntry — **COMPLETE** (eco_points field)
- [✓] **T1.13** Store hex eco rating in HexTileDbEntry — **COMPLETE** (eco_rating field)

## Phase 5: Edge Cases
- [✓] **T1.14** Handle negative eco points — **COMPLETE** (set_eco_rating clamps to min: 0)
- [✓] **T1.15** Handle multiple actions in one session — **COMPLETE** (eco points accumulate)
