# IdleBot — KANBAN

> Task tracking board for IdleBot development
> **Last Updated:** 2026-07-26

---

## 📊 Overall Status

```
Total Specs: 21
Backlog: 16
In Progress: 3 (with code but no review)
Tests: 0 (no passing tests yet)
Done: 0 (no spec fully reviewed)
```

**Current State:**
- ✅ Substantial code exists in all 4 crates (idlecore-core, idlecore-chain, idlecore-server, idlecore-client)
- ⚠️ 4 specs have complete documentation (spec.md + plan.md + tasks.md): 001, 002, 003, 004
- ⚠️ 17 specs have only spec.md (no implementation plan yet)
- ⚠️ No tasks.md has checked boxes (implementation not formally tracked)
- ⚠️ No review.md files exist (no specs formally completed)
- ⚠️ Build works for individual crates but times out on full workspace (Pi performance)
- ⚠️ 6 test files exist but no test status verified

---

## 📋 BACKLOG

### Specs without full documentation (spec.md only)

**World & Gameplay**
- [ ] **005-voice-chat** — Voice chat architecture (str0m 0.21)
- [ ] **006-vehicles** — Vehicle system (5 types, speed multipliers)
- [ ] **007-cosmetics** — Cosmetic system for players
- [ ] **008-teleport** — Teleport mechanic (100G cost)
- [ ] **009-minimap** — Minimap and global map display

**Economy & Marketplace**
- [ ] **010-economy** — Gold/Eco Points/USDT economy system
- [ ] **011-marketplace** — Marketplace listing & purchase
- [ ] **012-smart-contracts** — Polygon smart contracts (Alloy)
- [ ] **020-eco-points** — Eco Points specific mechanics

**Authentication & Identity**
- [ ] **013-wallet-auth** — Polygon wallet login
- [ ] **014-player-identity** — Player identity management
- [ ] **015-scheduler-security** — Scheduled function security
- [ ] **017-level-progression** — Level progression (100*level² XP formula)
- [ ] **018-multiplayer** — Multiplayer synchronization
- [ ] **019-database-schema** — SpacetimeDB schema design
- [ ] **021-server-protocol** — Server-client protocol

**Assets**
- [ ] **016-assets** — Procedural → Low-poly → Polish pipeline

---

## 🚀 IN PROGRESS

### Specs with complete documentation (spec.md + plan.md + tasks.md)

**001-idle-gains** — Idle gains calculation
- Status: Code exists, tasks unchecked
- Files: idle_config.rs (6531 LOC in workspace), scheduler/idle.rs (stub)
- Next: Complete server integration (scheduled function), client UI

**002-hex-grid** — Hex grid generation and rendering
- Status: Code exists, tasks unchecked
- Files: hex.rs, grid.rs, hex_tile.rs, world/map_generator.rs, world/hex_renderer.rs
- Next: Complete grid generation tests, render verification

**003-player-spawn** — Player spawn and WASD movement
- Status: Code exists, tasks unchecked
- Files: player.rs, input.rs, player/player_system.rs
- Next: Movement system integration, spawn logic tests

**004-interactions** — Plant, harvest, clean actions
- Status: Code exists, tasks unchecked
- Files: plant.rs, actions.rs, farming.rs (server)
- Next: Action validation, server reducer integration

---

## 🔬 TESTS / TDD

### Current Test Files (6 files with #[cfg(test)])

| File | Lines | Status |
|------|-------|--------|
| idlecore-core/src/plant.rs | 487 | Tests exist (unchecked) |
| idlecore-core/src/progression.rs | 61 | Tests exist (unchecked) |
| idlecore-core/src/economy.rs | 510 | Tests exist (unchecked) |
| idlecore-core/src/lib.rs | 543 | Tests exist (unchecked) |
| idlecore-core/src/idle_config.rs | 312 | Tests exist (unchecked) |
| idlecore-server/src/scheduler/idle.rs | - | Stub implementation |

**Test Status:** ⚠️ No verification run completed (build timeout on Pi)

---

## ✅ DONE

### Infrastructure (Completed)
- [x] Project structure initialization
- [x] SDD template setup (specs, constitution)
- [x] Cargo.toml workspace configuration
- [x] First commit to GitHub

### Substantial Code Written (Not Formally "Done")
- [ ] idlecore-core: 16 source files (~6,500 LOC)
- [ ] idlecore-chain: 3 source files (~200 LOC)
- [ ] idlecore-server: 7 source files (stubs + partial)
- [ ] idlecore-client: 9 source files (partial implementation)

**Note:** Code exists but no specs formally reviewed, no tests verified, no integration completed.

---

## 📈 Progress Metrics

### By Spec Documentation
- Complete (spec + plan + tasks): 4/21 (19%)
- Partial (spec only): 17/21 (81%)

### By Code Coverage
- idlecore-core: 16 files, ~6,500 LOC — Most complete
- idlecore-chain: 3 files, ~200 LOC — Functional
- idlecore-server: 7 files — Partial (stubs in scheduler)
- idlecore-client: 9 files — Partial (systems wired but incomplete)

### By Tasks Completed
- Checked tasks across all specs: 0%
- Total task items: ~80+
- Completed: 0

---

## 🎯 Recommended Next Steps

### Immediate (This Week)
1. **Run full test suite**: `cargo test` to verify what actually passes
2. **Verify build**: `cargo check` with longer timeout or on better hardware
3. **Complete 001-idle-gains**: Server integration is the blocker

### Short-term (This Sprint)
4. **Formalize 001-004**: Check off tasks in tasks.md as code is verified
5. **Create plans**: Write plan.md for specs 005-021
6. **Integration testing**: Get 001-idle-gains working end-to-end

### Medium-term
7. **Complete documentation**: All 21 specs need spec.md + plan.md + tasks.md
8. **Code review**: Review existing code against specs
9. **MVP scope**: Focus on Core Loop (001-004, 010-economy, 013-wallet-auth)

---

## 📝 Notes

- **MVP Goal:** Core idle loop with voice chat and basic interactions
- **Tech Stack:** Bevy 0.15, SpacetimeDB 2.7, Alloy, Polygon, str0m 0.21
- **Build Performance:** Pi is too slow for full workspace builds (>60s timeout)
- **Critical Blocker:** No verified passing tests — assume nothing works until proven
- **Next Milestone:** Get 001-idle-gains fully integrated (server + client)

---

## 🔗 Related Files

- `specs/constitution.md` — Unnegotiable principles
- `feature_list.json` — Feature tracking (currently empty)
- `PROPOSAL.md` — Full design specification
- `specs/001-idle-gains/status.md` — Per-feature status script
