# Spec 006: Vehicle System

> **Objective:** Implement vehicle purchase, equip, and speed modification system

## Problem Statement

Players need vehicles to traverse the hex grid faster. Vehicles provide speed multipliers and should be purchasable with gold.

## Proposed Solution

- 5 vehicle types with different speed multipliers and costs
- Equippable via UI inventory
- Speed affects WASD movement rate
- Visual representation on player character

## Requirements

### Functional Requirements
1. FR1: Purchase vehicle with gold
2. FR2: Equip/unequip vehicle from inventory
3. FR3: Apply speed multiplier to movement
4. FR4: Visual vehicle display on player
5. FR5: Vehicle persists across sessions

### Non-Functional Requirements
1. NFR1: Vehicle state synced to server
2. NFR2: No PvP vehicle advantages (cosmetic only)

## Design

### Vehicle Data
| Vehicle | Speed Multiplier | Gold Cost | Description |
|---------|-----------------|-----------|-------------|
| None | 1.0x | 0 | Base speed |
| Bicycle | 2.0x | 500 | Fast, eco-friendly |
| Scooter | 3.0x | 1,000 | Quick commuting |
| Motorcycle | 5.0x | 2,500 | Street racer |
| Boat | 4.0x | 2,000 | Water traversal |
| Airplane | 10.0x | 10,000 | Ultimate speed |

### Vehicle System
```rust
struct Vehicle {
    vehicle_type: VehicleType,
    equipped: bool,
    purchased: bool,
    speed_multiplier: f32,
}

enum VehicleType {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

impl Vehicle {
    fn speed_multiplier(&self) -> f32 {
        match self.vehicle_type {
            VehicleType::None => 1.0,
            VehicleType::Bicycle => 2.0,
            VehicleType::Scooter => 3.0,
            VehicleType::Motorcycle => 5.0,
            VehicleType::Boat => 4.0,
            VehicleType::Airplane => 10.0,
        }
    }
}
```

### Player Vehicle Inventory
```rust
struct Player {
    vehicles: Vec<Vehicle>,
    equipped_vehicle: Option<Vehicle>,
}

fn equip_vehicle(player: &mut Player, vehicle_index: usize) {
    if let Some(vehicle) = player.vehicles.get(vehicle_index) {
        player.equipped_vehicle = Some(vehicle.clone());
        vehicle.equipped = true;
    }
}

fn purchase_vehicle(player: &mut Player, vehicle_type: VehicleType) {
    let cost = vehicle_type.gold_cost();
    if player.gold >= cost && !player.has_vehicle(vehicle_type) {
        player.gold -= cost;
        let vehicle = Vehicle::new(vehicle_type);
        player.vehicles.push(vehicle);
    }
}
```

## Acceptance Criteria
- [ ] All 5 vehicles purchasable with gold
- [ ] Equipped vehicle displays on player
- [ ] Speed multiplier applied to movement
- [ ] Vehicle persists after logout
- [ ] UI shows vehicle inventory

## Risks
- R1: Vehicle visual complexity
- R2: Balance issues (Airplane too fast?)

## Open Questions
- Q1: Should vehicles have durability?
- Q2: Boats work on water tiles?
