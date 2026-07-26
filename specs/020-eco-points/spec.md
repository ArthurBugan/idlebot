# Spec 020: Eco Points & Hex Rating System

> **Objective:** Define how Eco Points are earned/spent and how they affect hex eco rating

## Problem Statement

Players earn Eco Points by cleaning pollution and planting trees. These points must affect hex eco rating, but the exact gameplay impact is unspecified. This spec defines the mechanism, effects, and balance.

## Proposed Solution

- Eco Points earned: Clean (+10), Plant Tree (+5), Harvest Tree (+2)
- Eco Points spent: Unlock special cosmetics (500 EP), Mark hex as "Eco-Friendly"
- Hex eco rating changes: +10 on clean, +5 on plant tree, decays -1/day
- Eco rating affects: Visual tint (greener = higher), unlocks "Eco Warrior" title at 100+
- No gameplay advantage (cosmetic/UI only)

## Requirements

### Functional Requirements
1. FR1: Calculate eco points for actions (clean, plant, harvest)
2. FR2: Update hex eco rating when eco points awarded
3. FR3: Track eco rating decay over time (-1 per day)
4. FR4: Display eco rating on hex (color tint, UI label)
5. FR5: Unlock cosmetics at eco point thresholds (500 EP = "Eco Warrior" hat)
6. FR6: Transaction log for eco point changes
7. FR7: Eco-friendly hex marker (visual + UI)

### Non-Functional Requirements
1. NFR1: Eco point calculations server-authoritative
2. NFR2: Eco rating decays atomically (scheduled function)
3. NFR3: No negative eco points or rating

## Design

### Eco Point Earnings
```rust
enum EcoPointSource {
    CleanPollution { points: u32 },
    PlantTree { points: u32 },
    HarvestTree { points: u32 },
    DailyBonus { points: u32 },  // Future: daily login reward
}

struct EcoPoints {
    total_earned: u32,
    total_spent: u32,
    current: u32,
}

impl EcoPoints {
    fn add_points(&mut self, source: EcoPointSource) {
        let points = match source {
            EcoPointSource::CleanPollution { points } => points,
            EcoPointSource::PlantTree { points } => points,
            EcoPointSource::HarvestTree { points } => points,
            EcoPointSource::DailyBonus { points } => points,
        };
        self.current += points;
        self.total_earned += points;
    }
    
    fn spend_points(&mut self, amount: u32) -> Result<()> {
        if self.current < amount {
            return Err(EcoError::InsufficientPoints);
        }
        self.current -= amount;
        self.total_spent += amount;
        Ok(())
    }
}
```

### Hex Eco Rating
```rust
struct HexEcoRating {
    rating: i32,        // 0-100 scale
    last_updated: Instant,
    decay_rate: i32,    // -1 per day
    eco_actions: Vec<EcoAction>,  // audit log
}

enum EcoAction {
    Cleaned { hex_id: u64, points: u32 },
    Planted { hex_id: u64, points: u32 },
    Harvested { hex_id: u64, points: u32 },
    Decayed { hex_id: u64, change: i32 },
}

impl HexEcoRating {
    fn apply_action(&mut self, action: EcoAction) {
        match action {
            EcoAction::Cleaned { points, .. } => {
                self.rating += 10;
                if self.rating > 100 { self.rating = 100; }
            }
            EcoAction::Planted { points, .. } => {
                self.rating += 5;
                if self.rating > 100 { self.rating = 100; }
            }
            EcoAction::Harvested { points, .. } => {
                self.rating += 2;
                if self.rating > 100 { self.rating = 100; }
            }
            EcoAction::Decayed { change, .. } => {
                self.rating += change;  // change is negative
                if self.rating < 0 { self.rating = 0; }
            }
        }
        self.eco_actions.push(action);
        self.last_updated = Instant::now();
    }
    
    fn decay_daily(&mut self) {
        if self.last_updated.elapsed() >= Duration::from_secs(86400) {
            self.apply_action(EcoAction::Decayed { change: -1, hex_id: self.hex_id });
        }
    }
}
```

### Eco Rating Effects
```rust
fn get_eco_tint(rating: i32) -> Color {
    // 0 (polluted) = dark gray
    // 50 (normal) = grass green
    // 100 (eco-friendly) = bright green
    let t = (rating as f32) / 100.0;
    Color::hsl(
        120.0,    // green hue
        0.8,      // saturation
        0.3 + t * 0.3,  // lightness: 0.3 (dark) → 0.6 (bright)
    )
}

fn is_eco_friendly(rating: i32) -> bool {
    rating >= 100
}

fn get_eco_title(player_eco_points: u32) -> Option<&'static str> {
    if player_eco_points >= 1000 {
        Some("Eco Legend")
    } else if player_eco_points >= 500 {
        Some("Eco Warrior")
    } else if player_eco_points >= 100 {
        Some("Eco Enthusiast")
    } else {
        None
    }
}
```

### Cosmetic Unlocks
```rust
enum EcoCosmeticUnlock {
    EcoWarriorHat { requires: u32, cosmetic_id: u32 },
    EcoWarriorAura { requires: u32, cosmetic_id: u32 },
    EcoWarriorTrail { requires: u32, cosmetic_id: u32 },
}

fn check_eco_unlock(player_eco_points: u32, unlock: &EcoCosmeticUnlock) -> bool {
    player_eco_points >= unlock.requires
}
```

### Scheduled Decay Function
```rust
fn eco_decay_scheduler(db: &spacetimedb::DatabaseIndex) {
    let hexes = db.hex_tiles().collect::<Vec<_>>();
    
    for hex in hexes {
        let mut eco_rating = HexEcoRating::from_db(hex);
        eco_rating.decay_daily();
        db.update_hex_rating(hex.hex_id, eco_rating.rating);
    }
}
```

## Acceptance Criteria
- [ ] Eco points awarded correctly on clean/plant/harvest actions
- [ ] Hex eco rating updates on eco actions
- [ ] Rating decays -1 per day for inactive hexes
- [ ] Eco rating displays as color tint on hexes
- [ ] Eco-friendly hexes (100+) unlock title
- [ ] Eco cosmetics unlock at 500 EP
- [ ] Eco transaction log recorded

## Risks
- R1: Eco decay could make hexes feel "punished" for inactivity
- R2. Balance: eco points vs gold — should eco be harder to earn?
- R3: No gameplay advantage means less motivation to earn eco points

## Open Questions
- Q1: Should eco rating affect harvest yield (more eco = more gold)?
- Q2: Should eco-friendly hexes spawn rarer plants?
- Q3: Is daily decay too aggressive or too slow?
- Q4: Should there be a "pollution spread" mechanic (uncontaminated hexes slowly pollute neighbors)?
