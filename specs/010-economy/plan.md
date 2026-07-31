# Plan 010: Economy System

> **Implementation Plan**

## Architecture

### Currency System
- Gold: Primary currency (idle gains, harvesting, actions)
- USDT: Premium currency (marketplace purchases)
- Eco Points: Environmental currency (cleaning, planting)
- Server-authoritative all calculations

### Economy Ledger
- Transaction table for audit trail
- Player economy state in database
- No negative balance enforcement

### Economy Actions
- Plant: spend 10G, gain 5XP
- Harvest: earn 15G + 10XP
- Clean: spend 20G, earn 15G + 15XP + 10EcoPoints
- Teleport: spend 100G
- Purchase vehicle/cosmetic: spend gold or USDT

## Files to Create/Modify

### Core (idlecore-core)
- `src/economy.rs` — Full rewrite: PlayerEconomy struct, EconomyAction enum, execute_action, EconomyLedger

### Server (idlecore-server)
- Modify `src/types.rs` — Add CurrencyType enum, EconomyAction
- Modify `src/main.rs` — Wire economy actions to reducers

### Client (idlecore-client)
- Modify `src/player.rs` — Display all three currencies

## Dependencies
- Requires 001-idle-gains (gold earning)
- Requires 004-interactions (action gold costs)
- Requires 014-player-identity (player state)

## Testing Strategy
1. Unit test: Gold add/spend correctly
2. Unit test: No negative balance allowed
3. Integration test: All action types cost/earn correctly
4. Edge case: Insufficient funds handled gracefully

## Timeline
- **Estimate:** 2-3 days
- **Phase:** Core loop (FR1 is blocked until 004)

## Blocked Until
- 004-interactions must be complete first (actions need economy validation)
