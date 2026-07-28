# Plan 010: Economy System

> **Implementation Plan**

## Architecture

### Currency Definitions
- Gold: Earned via idle gains, harvesting. Spent on planting, vehicles, cosmetics, teleport.
- USDT: Premium currency for marketplace purchases.
- Eco Points: Earned by cleaning pollution, planting trees. Affects eco rating.

### Economy Actions
- Plant: cost 10G, gain 5 XP
- Harvest: free, gain 15G + 10 XP
- Clean: cost 20G, gain 20G + 15 XP + 10 EP
- Teleport: cost 100G
- Publish template: cost 50G

### Transaction Ledger
- Server-authoritative transaction log
- Tracks gold changes, eco point changes per action

## Files to Create/Modify

### Core (idlecore-core)
- `src/economy.rs` — PlayerEconomy struct, currency management, action execution

### Server (idlecore-server)
- `src/progression.rs` — Wire economy to progression (level unlocks based on gold spent)
- `src/scheduler/idle.rs` — Gold earned via idle gains

### Client (idlecore-client)
- `src/economy_ui.rs` — Display gold, USDT, eco points in UI
- `src/main.rs` — Wire economy UI systems

## Testing Strategy
1. Unit test: Gold earned/spent correctly on all actions
2. Unit test: Eco points calculated correctly
3. Unit test: No negative balances
4. Integration test: Full economy flow (plant → idle → harvest)

## Dependencies
- Depends on 001-idle-gains (idle gold earning)
- Depends on 004-interactions (action costs)
- Depends on 006-vehicles (vehicle costs)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
