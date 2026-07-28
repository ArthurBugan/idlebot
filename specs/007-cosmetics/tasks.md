# Tasks 007: Cosmetics System

> **Implementation Checklist**

## Phase 1: Purchase Cosmetic
- [ ] **T1.1** Define CosmeticItem struct (category, cosmetic_type, purchased, equipped)
- [ ] **T1.2** Define CosmeticCategory enum (Hat, Aura, Trail)
- [ ] **T1.3** Define CosmeticType enum (Basic, Premium)
- [ ] **T1.4** Create CosmeticInventory struct (vec of CosmeticItem)

## Phase 2: Server Logic
- [ ] **T1.5** Purchase hat with gold (200G)
- [ ] **T1.6** Purchase aura with gold or USDT (500G or 1.0 USDT)
- [ ] **T1.7** Purchase trail with gold or USDT (300G or 1.0 USDT)
- [ ] **T1.8** Validate player has enough currency

## Phase 3: Equipment System
- [ ] **T1.9** Implement equip_cosmetic() function
- [ ] **T1.10** Implement unequip_cosmetic() function
- [ ] **T1.11** Track equipped cosmetics per category

## Phase 4: Visual Rendering
- [ ] **T1.12** Render equipped hat on player avatar
- [ ] **T1.13** Render equipped aura around player
- [ ] **T1.14** Render equipped trail behind player

## Phase 5: Inventory Management
- [ ] **T1.15** Display cosmetic inventory in UI
- [ ] **T1.16** Show purchased vs equipped status

## Phase 6: Testing
- [ ] **T1.17** Test purchase with sufficient gold
- [ ] **T1.18** Test purchase with insufficient gold
- [ ] **T1.19** Test equip/unequip cycle
- [ ] **T1.20** Test cosmetics persist across sessions
- [ ] **T1.21** Test no gameplay advantage from cosmetics
