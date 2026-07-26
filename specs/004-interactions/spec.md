# Spec 004: Basic Interactions (Plant, Harvest, Clean)

> **Objective:** Implement plant, harvest, and clean pollution interactions

## Problem Statement

Players need to interact with the world by planting seeds, harvesting mature plants, and cleaning polluted hexes. Each action has costs and rewards.

## Proposed Solution

- Click or press key to interact with hex in range
- Plant: costs 10G, requires empty hex, gives 5 XP
- Harvest: free, requires mature plant, gives 15G + 10 XP
- Clean: costs 20G, requires polluted hex, gives 20G + 15 XP

## Requirements

### Functional Requirements
1. FR1: Detect hex in interaction range
2. FR2: Plant action with validation
3. FR3: Harvest action with validation
4. FR4: Clean action with validation
5. FR5: Update player gold/XP on action
6. FR6: Plant growth progression

### Non-Functional Requirements
1. NFR1: Action cooldown < 100ms
2. NFR2: Server-authoritative validation
3. NFR3: Visual feedback on action

## Design

### Interaction Range
- Range: 1 hex (10 meters)
- Detect hex under player cursor or in front of player

### Action Logic
```rust
fn plant(&mut self, player: &Player, hex: &Hex) -> Result<ActionResult> {
    if player.gold < 10 {
        return Err(ActionError::InsufficientGold);
    }
    if !hex.is_empty() {
        return Err(ActionError::HexOccupied);
    }
    
    player.gold -= 10;
    player.xp += 5;
    hex.plant = Some(Plant::new(PlantType::Wheat));
    
    Ok(ActionResult {
        success: true,
        gold_change: -10,
        xp_change: 5,
        message: "Planted seed".to_string(),
    })
}

fn harvest(&mut self, player: &Player, hex: &Hex) -> Result<ActionResult> {
    let plant = hex.plant.as_ref()
        .ok_or(ActionError::NoPlant)?;
    
    if !plant.is_mature() {
        return Err(ActionError::PlantNotMature);
    }
    
    player.gold += 15;
    player.xp += 10;
    hex.plant = None;
    
    Ok(ActionResult {
        success: true,
        gold_change: 15,
        xp_change: 10,
        message: "Harvested plant".to_string(),
    })
}

fn clean_pollution(&mut self, player: &Player, hex: &Hex) -> Result<ActionResult> {
    if player.gold < 20 {
        return Err(ActionError::InsufficientGold);
    }
    if !hex.is_polluted() {
        return Err(ActionError::NotPolluted);
    }
    
    player.gold -= 20;
    player.gold += 20;
    player.xp += 15;
    hex.pollution = None;
    hex.eco_rating += 10;
    
    Ok(ActionResult {
        success: true,
        gold_change: 0,
        xp_change: 15,
        message: "Cleaned pollution".to_string(),
    })
}
```

### Plant Growth
```rust
struct Plant {
    plant_type: PlantType,
    planted_at: Instant,
    growth_time: Duration,
}

impl Plant {
    fn is_mature(&self) -> bool {
        self.planted_at.elapsed() >= self.growth_time
    }
}

enum PlantType {
    Wheat(Duration::from_secs(30)),
    Tree(Duration::from_secs(120)),
    RareHerb(Duration::from_secs(300)),
}
```

## Acceptance Criteria
- [ ] Plant action validates gold and hex state
- [ ] Harvest action validates plant maturity
- [ ] Clean action validates pollution state
- [ ] Gold/XP updated correctly
- [ ] Plant growth progresses over time
- [ ] Visual feedback for each action

## Risks
- R1: Concurrent plant/harvest on same hex
- R2: Plant growth timing accuracy

## Open Questions
- Q1: Should plants have animations?
- Q2: Different plant types with different rewards?
