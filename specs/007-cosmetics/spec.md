# Spec 007: Cosmetics System

> **Objective:** Implement cosmetic items (hats, auras, trails) purchasable with gold or USDT

## Problem Statement

Players want to customize their avatar appearance. Cosmetics should be tiered by currency type (gold for basic, USDT for premium).

## Proposed Solution

- Three cosmetic categories: Hat, Aura, Trail
- Gold for basic cosmetics, USDT for premium
- Cosmetics are visual only, no gameplay advantage
- Persistent across sessions

## Requirements

### Functional Requirements
1. FR1: Purchase hat with gold
2. FR2: Purchase aura with gold or USDT
3. FR3: Purchase trail with gold or USDT
4. FR4: Equip/unequip cosmetics
5. FR5: Visual rendering of equipped cosmetics
6. FR6: Cosmetic inventory management

### Non-Functional Requirements
1. NFR1: Cosmetics synced to server
2. NFR2: No competitive advantage from cosmetics

## Design

### Cosmetic Types
| Category | Gold Cost | USDT Cost | Effect |
|----------|-----------|-----------|--------|
| Hat (Basic) | 200 | — | Simple hat model |
| Hat (Premium) | — | 1.0 | Animated hat |
| Aura (Basic) | 500 | — | Static glow |
| Aura (Premium) | — | 2.5 | Particle effects |
| Trail (Basic) | 300 | — | Simple color trail |
| Trail (Premium) | — | 1.5 | Animated trail |

### Cosmetic Inventory
```rust
struct CosmeticItem {
    category: CosmeticCategory,
    cosmetic_type: CosmeticType,
    purchased: bool,
    equipped: bool,
}

enum CosmeticCategory {
    Hat,
    Aura,
    Trail,
}

enum CosmeticType {
    Basic,
    Premium,
}

struct Player {
    cosmetics: Vec<CosmeticItem>,
    equipped_hat: Option<CosmeticItem>,
    equipped_aura: Option<CosmeticItem>,
    equipped_trail: Option<CosmeticItem>,
}
```

### Cosmetic System
```rust
fn equip_cosmetic(player: &mut Player, category: CosmeticCategory, index: usize) {
    if let Some(cosmetic) = player.cosmetics.iter_mut().find(|c| {
        c.category == category && c.equipped
    }) {
        cosmetic.equipped = false;
    }
    
    if let Some(cosmetic) = player.cosmetics.get_mut(index) {
        if cosmetic.category == category {
            cosmetic.equipped = true;
            match category {
                CosmeticCategory::Hat => player.equipped_hat = Some(cosmetic.clone()),
                CosmeticCategory::Aura => player.equipped_aura = Some(cosmetic.clone()),
                CosmeticCategory::Trail => player.equipped_trail = Some(cosmetic.clone()),
            }
        }
    }
}
```

## Acceptance Criteria
- [ ] All cosmetic categories purchasable
- [ ] Equipped cosmetics display correctly
- [ ] Can equip/unequip without losing purchase
- [ ] Gold/USDT deductions work correctly
- [ ] Cosmetics persist across sessions

## Risks
- R1: Asset creation for each cosmetic
- R2: Performance with multiple particles

## Open Questions
- Q1: Should cosmetics have rare/legendary tiers?
yes
- Q2: Can players gift cosmetics to others?
yes