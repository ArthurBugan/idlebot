# Tasks 010: Economy System

> **Implementation Checklist**

## Phase 1: Currency Display
- [ ] **T1.1** Create EconomyPanel component showing all 3 currencies
- [ ] **T1.2** Display Gold balance
- [ ] **T1.3** Display USDT balance  
- [ ] **T1.4** Display Eco Points

## Phase 2: Gold Economy
- [ ] **T1.5** Gold earned via idle gains (spawn threshold)
- [ ] **T1.6** Gold earned via actions (planting, harvesting, selling)
- [ ] **T1.7** Gold spent on planting (10G per action)
- [ ] **T1.8** Gold spent on vehicle purchase
- [ ] **T1.9** Gold spent on cosmetic purchase
- [ ] **T1.10** Gold spent on teleport (100G)

## Phase 3: USDT Economy
- [ ] **T2.1** USDT price ratio fixed (1 USDT = 2.0281G)
- [ ] **T2.2** USDT balance tracked in DB
- [ ] **T2.3** USDT deducted for template purchase
- [ ] **T2.4** Cooldown check (6 hours) before withdrawing

## Phase 4: Eco Points
- [ ] **T3.1** Eco Points earned: Clean (+10), Plant tree (+5), Harvest tree (+2)
- [ ] **T3.2** Eco Points affect hex eco_rating
- [ ] **T3.3** Eco rating decreases slowly (max 11, target 0)

## Phase 5: Economy Ledger
- [ ] **T4.1** Transaction record: player_id, from/to, amount, currency, timestamp
