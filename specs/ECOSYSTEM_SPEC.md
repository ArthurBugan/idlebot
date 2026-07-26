# IdleBot — Ecosystem Mechanics Specification

**Version:** 1.0  
**Last Updated:** 2026-07-26  
**Status:** Active

---

## 1. Overview

This document defines the complete ecosystem mechanics for IdleBot's economy, progression, and player engagement loops. It supplements the main PROPOSAL.md with specific formulas, sink mechanisms, and pacing requirements.

---

## 2. Economy Sinks (Detailed)

### 2.1 Vehicle Maintenance Cost

**Formula:** `5 Gold/hour` (applied every 24 hours at server midnight)

| Vehicle Type | Hourly Cost | Daily Cost | Weekly Cost | Monthly Cost |
|--------------|-------------|------------|-------------|--------------|
| Bicycle      | 5G          | 120G       | 840G        | 3,600G       |
| Scooter      | 5G          | 120G       | 840G        | 3,600G       |
| Motorcycle   | 5G          | 120G       | 840G        | 3,600G       |
| Boat         | 5G          | 120G       | 840G        | 3,600G       |
| Airplane     | 5G          | 120G       | 840G        | 3,600G       |

**Note:** Cost is identical across all vehicles (thematic: electric vehicles cost the same to maintain).

### 2.2 Idle Gain Decay

**Trigger:** 7 consecutive days without any Gold spending activity

**Formula:** 
- Day 8+: `-10%` per day
- Day 15+: `-25%` per day
- Day 30+: `-50%` per day

**Recovery:** Spending at least 1 Gold resets decay counter.

### 2.3 Teleport Cost Scaling

**Formula:** `Cost = 100 * level^0.5` (rounded down)

| Level | Teleport Cost |
|-------|---------------|
| 1     | 100G          |
| 5     | 223G          |
| 10    | 316G          |
| 25    | 500G          |
| 50    | 707G          |
| 100   | 1,000G        |
| 200   | 1,414G        |

### 2.4 Listing Renewal Cost

**Formula:** `10 Gold per 7 days` (auto-renewal or manual renewal)

**Grace period:** 24 hours after expiration before listing goes inactive.

### 2.5 Planting Costs

| Plant Type    | Cost (Gold) | Growth Time |
|---------------|-------------|-------------|
| Wheat         | 10G         | 1 hour      |
| Corn          | 15G         | 1.5 hours   |
| Sunflower     | 20G         | 2 hours     |
| Tree          | 50G         | 6 hours     |
| RareHerb      | 100G        | 12 hours    |

---

## 3. Progression Pacing

### 3.1 Level Formula

```rust
pub fn xp_for_next_level(level: u32) -> u64 {
    100 * (level as u64).pow(2)
}

pub fn calculate_level(total_xp: u64) -> u32 {
    let mut level = 1u32;
    let mut xp_needed = 100u64;
    let mut remaining = total_xp;
    while remaining >= xp_needed {
        remaining -= xp_needed;
        level += 1;
        xp_needed = Self::xp_for_next_level(level);
    }
    level
}
```

### 3.2 XP Gains (Active Gameplay)

| Action            | XP | Gold |
|-------------------|----|------|
| Plant             | 5  | -10  |
| Harvest           | 10 | +15  |
| Clean Pollution   | 15 | +20  |
| Clear Terrain     | 5  | -15  |
| Publish Template  | 0  | -50  |

**Net per plant-harvest cycle:** +15 XP, +5 Gold

### 3.3 Idle Gains (Offline)

| Offline Duration | XP Gained | Gold Gained |
|-----------------|-----------|-------------|
| < 1 hour        | 10        | 5           |
| 1–6 hours       | 60        | 30          |
| 6–12 hours      | 100       | 50          |
| 12–24 hours     | 150       | 75          |
| Max: 24 hours   | 150       | 75          |

**Anti-Cheat:** Server validates with ±2s tolerance. Repeated rapid logins (within 5min of last logout) trigger 90-day "new player" state (no idle gains).

### 3.4 Level Milestones

| Level | Total XP  | Time to Reach (Active Play) | Time to Reach (Idle Play) |
|-------|-----------|-----------------------------|---------------------------|
| 5     | 1,000     | 67 actions                  | 13 days                   |
| 10    | 5,500     | 367 actions                 | 73 days                   |
| 25    | 40,625    | 2,708 actions               | ~5 years                  |
| 50    | 151,250   | 10,083 actions              | ~13 years                 |

**Note:** Level 100+ requires active engagement + marketplace income.

### 3.5 Diminishing Returns

**Formula:** XP gains decrease by 5% per 10 levels above level 50.

- Level 51-60: 5% reduction
- Level 61-70: 10% reduction
- Level 71-80: 15% reduction
- ...continues linearly

**Max reduction:** 50% at level 200+ (ensures progression continues but slower)

---

## 4. Inflation Control

### 4.1 Economy Flow

```
Faucet (Sources):
├── Idle gains (max 75G/24h)
├── Harvesting (+15G per cycle, 5min cooldown)
├── Cleaning (+20G per hex)
└── Marketplace sales (variable, seller price)

Sinks (Removal):
├── Vehicle maintenance (5G/h)
├── Listing renewals (10G/7d)
├── Teleport costs (100-500G+ depending on level)
├── Planting costs (10G per plant)
└── Cosmetic purchases (50-500G)
```

### 4.2 Inflation Target

- **Annual growth rate:** < 5%
- **Audits:** Quarterly
- **Adjustment mechanism:** Increase sink costs by 10% if growth exceeds 7% for 2 consecutive quarters

### 4.3 Gold Value Anchors

| Item/Service      | Cost (Gold) | Time Required (Active) | Time Required (Idle) |
|-------------------|-------------|------------------------|----------------------|
| Plant (Wheat)     | 10G         | 1 hour                 | 8 hours              |
| Harvest           | 0G          | 0 minutes              | 0 minutes            |
| Vehicle (Bicycle) | 500G        | 33 days                | 6.7 years            |
| Vehicle (Airplane)| 10,000G     | 667 days               | 133 years            |
| Teleport (Level 1)| 100G        | 6.7 days               | 1.3 years            |

---

## 5. Player Engagement Loops

### 5.1 Short-Term Loop (Minutes to Hours)

1. Enter hex with others
2. Interact with plants (plant/harvest)
3. Chat via voice
4. Move to next hex
5. Repeat

**Cycle time:** 5-15 minutes

### 5.2 Medium-Term Loop (Hours to Days)

1. Farm plants for gold
2. Buy vehicle
3. Use vehicle to travel faster
4. Visit high-traffic areas for voice chat
5. Publish templates to marketplace

**Cycle time:** 1-7 days

### 5.3 Long-Term Loop (Weeks to Months)

1. Build wealth through idle gains + marketplace
2. Upgrade cosmetics/vehicles
3. Grow level for lower teleport costs
4. Participate in seasonal events (future)
5. Build reputation in community

**Cycle time:** 1-6 months

---

## 6. Balance Testing Requirements

### 6.1 Simulation Model

**Tool:** Python simulation engine (see `scripts/sim_economy.py`)

**Inputs:**
- Player count (100, 500, 1000, 5000)
- Activity rates (active players, idle players)
- Marketplace activity (listings, sales)

**Outputs:**
- Gold in circulation over time
- Player wealth distribution (Gini coefficient)
- Time to reach level milestones
- Inflation rate

### 6.2 Acceptance Criteria

- **Stability:** Gold in circulation grows < 5% annually
- **Fairness:** Gini coefficient < 0.6 at year 1
- **Progression:** Level 10 reachable in < 3 months for active player
- **Engagement:** Average session length > 15 minutes

---

## 7. Future Adjustments

### 7.1 Dynamic Sink Adjustment

**Trigger:** Quarterly economy audit

**Adjustments:**
- Vehicle maintenance costs: +5-15%
- Listing renewal costs: +5-10%
- Teleport costs: +5-20%
- Planting costs: +5-10%

**Consent mechanism:** 7-day announcement window before changes take effect. Players can vote on adjustments via in-game referendum (future feature).

### 7.2 Seasonal Events

**Spring Cleanup Event:**
- Double XP for 1 week
- Planting costs reduced by 50%
- Pollution spread paused for 1 week

**Summer Sale Event:**
- Cosmetics 30% discount
- Marketplace fees reduced by 50%
- Vehicle maintenance paused for 3 days

---

**Last Updated:** 2026-07-26  
**Next Audit:** 2026-10-26
