# Spec 010: Economy System

> **Objective:** Implement complete economy with Gold, Eco Points, and USDT

## Problem Statement

Players need a balanced economy with multiple currencies for different purposes. Gold is the primary currency, USDT is premium, and Eco Points track environmental impact.

## Proposed Solution

- Gold: Earned via idle gains, harvesting. Spent on planting, vehicles, cosmetics, teleport.
- USDT: Premium currency for marketplace purchases.
- Eco Points: Earned by cleaning pollution and planting. Affects eco rating.

## Requirements

### Functional Requirements
1. FR1: Display all three currencies to player
2. FR2: Gold earned via idle gains and actions
3. FR3: Gold spent on actions (plant, clean, teleport, etc.)
4. FR4: USDT balance tracked and displayed
5. FR5: Eco Points earned/calculation
6. FR6: Eco Points affect hex eco rating
7. FR7: Economy ledger for audit

### Non-Functional Requirements
1. NFR1: Server-authoritative economy
2. NFR2: No negative currency balances
3. NFR3: Transaction history available to player

## Design

### Currency Definitions
```rust
struct PlayerEconomy {
    gold: u64,
    usdt: u64,
    eco_points: u32,
    lifetime_gold_earned: u64,
    lifetime_gold_spent: u64,
}

impl PlayerEconomy {
    fn add_gold(&mut self, amount: u64) {
        self.gold += amount;
        self.lifetime_gold_earned += amount;
    }
    
    fn spend_gold(&mut self, amount: u64) -> Result<()> {
        if self.gold < amount {
            return Err(EconomyError::InsufficientGold);
        }
        self.gold -= amount;
        self.lifetime_gold_spent += amount;
        Ok(())
    }
    
    fn add_eco_points(&mut self, amount: u32) {
        self.eco_points += amount;
    }
}
```

### Economy Actions
```rust
enum EconomyAction {
    Plant { cost: u64, xp: u32 },
    Harvest { reward_gold: u64, reward_xp: u32 },
    Clean { cost: u64, reward_gold: u64, reward_xp: u32 },
    Teleport { cost: u64 },
    PublishTemplate { cost: u64 },
    PurchaseVehicle { cost: u64 },
    PurchaseCosmetic { cost: u64, currency: CurrencyType },
}

enum CurrencyType {
    Gold,
    USDT,
}

fn execute_action(action: EconomyAction, player: &mut PlayerEconomy) -> Result<EconomyResult> {
    match action {
        EconomyAction::Plant { cost, xp } => {
            player.spend_gold(cost)?;
            player.add_xp(xp);
            Ok(EconomyResult::Success)
        }
        EconomyAction::Harvest { reward_gold, reward_xp } => {
            player.add_gold(reward_gold);
            player.add_xp(reward_xp);
            Ok(EconomyResult::Success)
        }
        EconomyAction::Clean { cost, reward_gold, reward_xp } => {
            player.spend_gold(cost)?;
            player.add_gold(reward_gold);
            player.add_xp(reward_xp);
            player.add_eco_points(10);
            Ok(EconomyResult::Success)
        }
        // ... etc
    }
}
```

### Economy Ledger
```rust
struct Transaction {
    id: Uuid,
    player_id: UUID,
    timestamp: Instant,
    action: EconomyAction,
    gold_change: i64,
    eco_points_change: i32,
    balance_after: u64,
}
```

## Acceptance Criteria
- [ ] All three currencies display correctly
- [ ] Gold earned/spent correctly on all actions
- [ ] Eco Points earned on clean actions
- [ ] No negative balances allowed
- [ ] Transaction history accessible
- [ ] Server-authoritative calculations

## Risks
- R1: Inflation from idle gains (need to monitor)
- R2: Economy imbalance between actions

## Open Questions
- Q1: Should there be daily gold caps?
- Q2: How to obtain USDT (real money, marketplace)?
