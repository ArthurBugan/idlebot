# Plan 020: Eco Points & Hex Rating System

> **Implementation Plan**

## Architecture

### Eco Points Economy
- Earn: Clean (+10 EP), Plant Tree (+5 EP), Harvest Tree (+2 EP)
- Spend: Unlock cosmetics at thresholds (500 EP = Eco Warrior hat)
- No negative eco points allowed

### Hex Eco Rating
- Scale: 0 (polluted) to 100 (eco-friendly)
- +10 from cleaning, +5 from planting, +2 from harvesting
- Decays -1 per day
- Visual tint: HSL green, lightness scales with rating
- Title unlocks: Eco Enthusiast (100+), Eco Warrior (500+), Eco Legend (1000+)

### Scheduled Decay
- Runs once per day per hex
- Only on hexes with changes
- No gameplay advantage — cosmetic/UI only

## Files to Create/Modify

### Core (idlecore-core)
- Modify `src/economy.rs` — Add EcoPoints struct, HexEcoRating struct
- Modify `src/plant.rs` — Add eco point reward to clean action

### Server (idlecore-server)
- Modify `src/progression.rs` — Add eco rating update
- Create `src/scheduler/eco.rs` — Daily eco decay scheduler

### Client (idlecore-client)
- Modify `src/player.rs` — Display eco points, eco title
- Modify `src/world/hex_renderer.rs` — Apply eco tint to hex colors

## Dependencies
- Requires 010-economy (eco points earning)
- Requires 014-player-identity (player state)
- Requires 019-database-schema (table definitions)

## Testing Strategy
1. Unit test: EcoPoints.add_points() calculates correctly
2. Unit test: EcoPoints.spend_points() prevents negative
3. Unit test: HexEcoRating decays correctly
4. Integration test: Clean action awards EP, updates hex rating
5. Edge case: Rating caps at 100, floors at 0

## Timeline
- **Estimate:** 2 days
- **Phase:** Phase 3 (Eco System)
- **Blocked Until:** 010-economy (eco point earning mechanism)
