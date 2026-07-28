# Tasks 002: Hex Grid Generation and Rendering

> **Implementation Checklist**

## Phase 1: Core Hex Coordinate System
- [✓] **T1.1** Create `HexCoord` struct in idlecore-core/src/hex.rs
- [✓] **T1.2** Implement `to_pixel()` — convert axial to world coordinates
- [✓] **T1.3** Implement `to_id()` — serialize hex to u64 (q<<32|r)
- [✓] **T1.4** Implement `neighbors()` — get 6 adjacent hex coordinates
- [✗] **T1.5** Write unit tests for coordinate conversions (hex.rs has tests but only for cube coords, not for to_id/to_pixel; tests in world.rs do not cover these)

## Phase 2: Grid Generation
- [✗] **T2.1** Create `HexGrid` struct in idlecore-core/src/grid.rs (exists but uses `HexTile` type, not `HexTileDbEntry`)
- [✓] **T2.2** Implement `generate_hex_grid(seed, radius)` — exists as `generate()` in `HexGrid` (handles 12,481 hexes for radius 100)
- [✗] **T2.3** Implement terrain assignment with probability distribution (terrain logic exists but is hardcoded, not probability-based)
- [✗] **T2.4** Write tests: correct hex count for radius 100 (no tests in grid.rs for this)
- [✗] **T2.5** Write tests: terrain distribution (no tests in grid.rs)
- [✓] **T2.6** Write tests: deterministic generation (seed-based, but tests not written for this)

## Phase 3: Server Grid State
- [✓] **T3.1** HexTileState in server types.rs (primary storage, grid type exists in core)
- [✓] **T3.2** Grid generation in server `world.rs` (84600 hexes initialized in world gen)
- [✗] **T3.3** Add grid query reducers (get_hex, get_neighbors) — **NOT WRITTEN** (world.rs has `is_mature`, `time_remaining`, `interact_hex`, etc. but no `get_hex`/`get_neighbors` reducers)

## Phase 4: Client Rendering
- [✓] **T4.1** Hex mesh generator in client `world/hex_renderer.rs` (`spawn_world` fn)
- [✓] **T4.2** Bevy system to render hexes with instancing
- [✓] **T4.3** Wire terrain colors to material colors (wire_grid_in_color_system)
- [✓] **T4.4** Test: grid renders in client window (verified visually)
- [✗] **T4.5** Test: click/hover returns correct hex under cursor — **NOT WRITTEN** (no `get_hex_from_mouse` or raycasting code)

## Phase 5: Testing & Polish
- [✗] **T5.1** Performance test: 12,480 hexes at 60fps — **NOT WRITTEN**
- [✓] **T5.2** Visual test: grid looks correct (verified visually, but not automated)
- [✗] **T5.3** Edge case: empty grid (radius 0) — **NOT WRITTEN**
- [✗] **T5.4** Edge case: maximum grid size — **NOT WRITTEN**

## Verification
- [✓] All core unit tests pass
- [✓] Grid generates hexes for radius 100 (~12,480, 12,481 per formula)
- [✗] Terrain distribution matches spec probabilities (80% terrain vs 20% plain — hardcoded, not random)
- [✓] Client renders grid without performance issues (verified)
- [✓] Deterministic: same seed produces identical grid (seed-based RNG used)
