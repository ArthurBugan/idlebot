# Plan 007: Cosmetics System

> **Implementation Plan**

## Architecture

### Cosmetic Types
- Three categories: Hat, Aura, Trail
- Basic cosmetics: gold only (200-500G)
- Premium cosmetics: USDT only (1.0-2.5 USDT)
- Cosmetics are visual only, no gameplay advantage

### Inventory Management
- Player has 3 equipped slots: hat, aura, trail
- Cosmetic inventory persists across sessions
- Equip/unequip UI with category tabs

## Files to Create/Modify

### Core (idlecore-core)
- `src/cosmetics.rs` — CosmeticItem struct, CosmeticCategory enum, purchase logic, equip logic

### Server (idlecore-server)
- `src/types.rs` — CosmeticItem DB table schema
- `src/main.rs` — Register purchase/equip reducers

### Client (idlecore-client)
- `src/cosmetics.rs` — Cosmetic inventory UI
- `src/main.rs` — Wire cosmetic purchase/equip systems

## Testing Strategy
1. Unit test: Purchase deducts gold/USDT correctly
2. Unit test: Equip/unequip toggles equipment
3. Integration test: Purchase → equip → visual display
4. Edge case: Insufficient balance

## Dependencies
- Depends on 010-economy (gold/USDT management)
- Depends on 013-wallet-auth (player creation for first-time purchase)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
