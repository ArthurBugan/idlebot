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

## Acceptance Criteria
- [ ] All 5 vehicles purchasable with gold
- [ ] Equipped vehicle displays on player
- [ ] Speed multiplier applied to movement
- [ ] Vehicle persists after logout
- [ ] UI shows vehicle inventory

## Risks
- R1: Vehicle visual complexity
for now its just simple shapes that represents the vehicles

## Open Questions
- Q1: Should vehicles have durability?
yes
- Q2: Boats work on water tiles?
yes
