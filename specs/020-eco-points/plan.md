# Plan 020: Eco Points & Hex Rating System

> **Implementation Plan**

## Architecture

### Eco Points Earned
- Clean: +10 EP
- Plant tree: +5 EP
- Harvest tree: +2 EP

### Eco Rating Calculation
- Eco rating: sum of all adjacent hex eco_ratings
- Display: 0-100 scale (0 = polluted, 100 = pristine)
- Visual: hex tint (darker = lower rating, greener = higher)

### Eco Point Spending
- Unlock special cosmetics at thresholds (500 EP = "Eco Warrior" hat)
- Mark hex as "Eco-Friendly" (visual marker)

## Files to Create/Modify

### Core (idlecore-core)
- `src/eco_points.rs` — EcoPoints struct, calculation logic

### Server (idlecore-server)
- `src/main.rs` — Register eco point reducers
- `src/types.rs` — EcoPoints table schema

### Client (idlecore-client)
- `src/eco_points_ui.rs` — Display eco points, eco rating

## Testing Strategy
1. Unit test: Eco points calculated correctly for actions
2. Unit test: Hex eco rating calculation
3. Unit test: Cosmetic unlocks at thresholds

## Dependencies
- Depends on 004-interactions (clean, plant, harvest actions)
- Depends on 010-economy (currency management)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** Post-MVP Balance
