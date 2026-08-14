# Tasks 002: Hex Grid Generation and Rendering

> **Implementation Checklist**

## Phase 1: Core Hex Coordinate System
- [x] **T1.1** Create `HexCoord` struct in idlecore-core/src/hex.rs
- [x] **T1.2** Implement `to_pixel()` — convert axial to world coordinates
- [x] **T1.3** Implement `to_id()` — serialize hex to u64 (q<<32|r)
- [x] **T1.4** Implement `neighbors()` — get 6 adjacent hex coordinates
- [x] **T1.5** Write unit tests for coordinate conversions (15 tests in hex.rs)

## Phase 2: Grid Generation
- [x] **T2.1** Create `HexGrid` struct in idlecore-core/src/hex_grid.rs
- [x] **T2.2** Implement `generate_hex_grid(seed, radius)` — exists as `EarthWorld::generate()` in world.rs
- [x] **T2.3** Implement terrain assignment with probability distribution (Earth-like biomes based on latitude/elevation)
- [x] **T2.4** Write tests: correct hex count for radius 100 (tests in hex_grid.rs)
- [x] **T2.5** Write tests: terrain distribution (test_ocean_land_ratio in world.rs)
- [x] **T2.6** Write tests: deterministic generation (seed-based, tests in world.rs)

## Phase 3: Server Grid State
- [x] **T3.1** HexTileState in server types.rs (primary storage, grid type exists in core)
- [x] **T3.2** Grid generation in server `world.rs` (84600 hexes initialized in world gen)
- [x] **T3.3** Queries — fully covered by hex_tile table subscriptions (no reducer needed)

## Phase 4: Client Rendering
- [x] **T4.1** Hex mesh generator in client `plugins/world.rs` (`spawn_world` fn)
- [x] **T4.2** Bevy system to render hexes (flat-top hex mesh)
- [x] **T4.3** Wire terrain colors to material colors (biome colors)
- [x] **T4.4** Test: grid renders in client window (verified visually, 7,651 tiles at radius 50)
- [x] **T4.5** world_pos_to_hex drives click selection; roundtrip tests cover cursor→hex

## Phase 5: Testing & Polish
- [x] **T5.1** Perf smoke test — radius-64 (12,481 hexes) traversal < 500 ms
- [x] **T5.2** Visual test: grid looks correct (verified visually)
- [x] **T5.3** radius-0 test returns only the center hex
- [x] **T5.4** radius-64 count and unique-id tests in hex_grid.rs

## Verification
- [x] All core unit tests pass (135 tests in idlecore-core)
- [x] Grid generates hexes for radius 50 (~7,651 tiles)
- [x] Earth-like biome distribution (ocean/land ratio ~71%)
- [x] Client renders grid without performance issues (verified)
- [x] Deterministic: same seed produces identical grid (seed-based RNG used)
