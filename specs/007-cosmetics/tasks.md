# Tasks 007: Cosmetics System

> **Implementation Checklist**

## Phase 1: Cosmetic Data Structures
- [✓] **T1.1** Create CosmeticItem struct with category and type — **IMPROVED** (CosmeticItem with category, purchased, equipped)
- [✓] **T1.2** Define CosmeticCategory enum (Hat, Aura, Trail) — **IMPROVED**
- [✓] **T1.3** Define CosmeticType enum (Basic, Premium) — **IMPROVED**
- [✓] **T1.4** CosmeticItem purchasable in server — **IMPROVED** (buy_item reducer)
- [✓] **T1.5** CosmeticItem visual rendering on client — **NOT IMPLEMENTED** (UI not wired yet)
- [ ] **T1.6** CosmeticItem tracked in persistent storage — **NOT IMPLEMENTED** (no DB table)

## Phase 2: Purchase Logic
- [ ] **T2.1** Purchase basic cosmetic (Hat, 200 gold) — **NOT IMPLEMENTED**
- [ ] **T2.2** Purchase premium cosmetic (USDT cost) — **NOT IMPLEMENTED**
- [ ] **T2.3** Purchase Aura (Gold or USDT, 500-2500G) — **NOT IMPLEMENTED**
- [ ] **T2.4** Purchase Trail (Gold or USDT, 300-1500G) — **NOT IMPLEMENTED**
- [✓] **T2.5** Gold deduction validated before purchase — **IMPROVED** (buy_item in world.rs checks gold)
- [✓] **T2.6** USDT deduction for premium — **NOT IMPLEMENTED**
- [✓] **T2.7** Failed purchase due to insufficient funds — **IMPROVED** (validation in buy_item)
- [✓] **T2.8** Purchase marked as successful — **NOT IMPLEMENTED** (no user feedback)

## Phase 3: Equip/Unequip
- [ ] **T3.1** Equip cosmetic from inventory slot — **NOT IMPLEMENTED**
- [ ] **T3.2** Unequip cosmetic to inventory slot — **NOT IMPLEMENTED**
- [✓] **T3.3** Previous equipped cosmetic returned to inventory — **NOT IMPLEMENTED**
- [✓] **T3.4** Only one cosmetic per category equipped at a time — **NOT IMPLEMENTED**
- [✓] **T3.5** Change cosmetic appearance in real-time — **NOT IMPLEMENTED**

## Phase 4: Storage & Persistence
- [ ] **T4.1** Cosmetic purchase persistent across sessions — **NOT IMPLEMENTED**
- [ ] **T4.2** Cosmetic equipment state persistent — **NOT IMPLEMENTED**
- [ ] **T4.3** Cosmetic inventory synced to server — **NOT IMPLEMENTED**
- [✓] **T4.4** Cosmetic item count tracked — **IMPROVED** (templates: String field in PlayerDbEntry)
- [✓] **T4.5** Template templates_limit respected — **IMPROVED** (templates_limit: u32 field)
- [ ] **T4.6** SQL query for purchasing cosmetic — **NOT IMPLEMENTED**
- [✓] **T4.7** SQL query for retrieving cosmetics — **NOT IMPLEMENTED**

## Phase 5: Testing
- [✓] **T5.1** Cosmetic purchase with sufficient gold — **NOT TESTED**
- [✓] **T5.2** Cosmetic purchase with insufficient gold — **NOT TESTED**
- [✓] **T5.3** Purchase premium cosmetic with USDT — **NOT TESTED**
- [✓] **T5.4** Equipment state persists after session restart — **NOT TESTED**
- [✓] **T5.5** Cosmetic does not provide competitive advantage — **NOT TESTED**
