/usr/bin/bash: warning: setlocale: LC_ALL: cannot change locale (en_US.UTF-8): No such file or directory
# Tasks 007: Cosmetics System

> **Implementation Checklist**

## Phase 1: Purchase Cosmetic
- [x] **T1.1** Define CosmeticItem struct (category, cosmetic_type, purchased, equipped) in cosmetic.rs
- [x] **T1.2** Define CosmeticCategory enum (Hat, Aura, Trail)
- [x] **T1.3** Define CosmeticType enum (Basic, Premium)
- [x] **T1.4** Create CosmeticInventory struct (vec of CosmeticItem)

## Phase 2: Server Logic
- [x] **T1.5** Purchase hat with gold (200G) — can_purchase_gold, purchase_with_gold functions
- [x] **T1.6** Purchase aura with gold or USDT (500G or 1.0 USDT)
- [x] **T1.7** Purchase trail with gold or USDT (300G or 1.0 USDT)
- [x] **T1.8** Validate player has enough currency

## Phase 3: Equipment System
- [x] **T1.9** Implement equip_cosmetic() function
- [x] **T1.10** Implement unequip_cosmetic() function
- [x] **T1.11** Track equipped cosmetics per category (equipped_hat, equipped_aura, equipped_trail)

## Phase 4: Visual Rendering
- [x] **T1.12** Render equipped hat on player avatar (render_hat())
- [x] **T1.13** Render equipped aura around player (render_aura())
- [x] **T1.14** Render equipped trail behind player (render_trail())

## Phase 5: Inventory Management
- [x] **T1.15** Display cosmetic inventory in UI (CosmeticInventory struct with items vec)
- [x] **T1.16** Show purchased vs equipped status (purchased and equipped fields)

## Phase 6: Testing
- [x] **T1.17** Test purchase with sufficient gold (test_can_purchase_gold)
- [x] **T1.18** Test purchase with insufficient gold (test_can_purchase_gold)
- [x] **T1.19** Test equip/unequip cycle (test_equip_unequip_cycle, test_equip_all_categories, test_unequip_all_categories)
- [x] **T1.20** Test cosmetics persist across sessions (test_purchase_persists)
- [x] **T1.21** Test no gameplay advantage from cosmetics (test_no_gameplay_advantage)
