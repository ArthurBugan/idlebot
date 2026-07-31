# Tasks 010: Economy System

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define CurrencyType enum (Gold, USDT)
- [ ] **T1.2** Define PlayerEconomy struct (gold, usdt, eco_points, lifetime stats)
- [ ] **T1.3** Implement add_gold(), spend_gold() with negative balance prevention
- [ ] **T1.4** Implement add_eco_points() method

## Phase 2: Economy Actions
- [ ] **T1.5** Define EconomyAction enum (Plant, Harvest, Clean, Teleport, Publish, etc.)
- [ ] **T1.6** Implement execute_action() with gold deduction/validation
- [ ] **T1.7** Implement harvest action with gold reward + XP
- [ ] **T1.8** Implement clean action with eco point reward

## Phase 3: Economy Ledger
- [ ] **T1.9** Define Transaction struct (id, player_id, timestamp, action, gold_change, etc.)
- [ ] **T1.10** Create EconomyLedger for transaction recording
- [ ] **T1.11** Add transaction creation to execute_action()

## Phase 4: Integration
- [ ] **T2.1** Wire idle gains to gold earning (modify server scheduler)
- [ ] **T2.2** Wire action costs to economy (modify reducer logic)
- [ ] **T2.3** Wire vehicle/cosmetic purchases to economy
- [ ] **T2.4** Display currencies in client UI

## Phase 5: Testing
- [ ] **T3.1** Gold earned/spent correctly on all actions
- [ ] **T3.2** Eco Points earned on clean actions
- [ ] **T3.3** No negative balances allowed
- [ ] **T3.4** Transaction history accessible

## Verification
- [✓] PlayerEconomy struct has all currency fields
- [✓] spend_gold() prevents negative balance
- [✓] All action types have execute_action implementation
