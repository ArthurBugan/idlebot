# Tasks 002: Hex Grid Generation and Rendering

> **Implementation Checklist**

## Phase 1: Core Hex Coordinate System
- [ ] **T1.1** Create HexCoord struct in idlecore-core/src/hex.rs
- [ ] **T1.2** Implement to_pixel() — convert axial to world coordinates
- [ ] **T1.3** Implement to_id() — serialize hex to u64 (q<<32|r)
- [ ] **T1.4** Implement neighbors() — get 6 adjacent hex coordinates
- [ ] **T1.5** Write unit tests for coordinate conversions

## Phase 2: Grid Generation
- [ ] **T2.1** Create HexGrid struct in idlecore-core/src/grid.rs
- [ ] **T2.2** Implement generate_hex_grid(seed, radius) function
- [ ] **T2.3** Implement terrain assignment with probability distribution
- [ ] **T2.4** Write tests: correct hex count for radius 100
- [ ] **T2.5** Write tests: terrain distribution within acceptable range
- [ ] **T2.6** Write tests: deterministic generation (same seed = same grid)

## Phase 3: Server Grid State
- [ ] **T3.1** Update HexTileDbEntry schema in server types.rs if needed
- [ ] **T3.2** Add grid generation to server world initialization
- [ ] **T3.3** Add grid query reducers (get_hex, get_neighbors)

## Phase 4: Client Rendering
- [ ] **T4.1** Create hex mesh generator (flat-top geometry)
- [ ] **T4.2** Create Bevy system to render hexes with instancing
- [ ] **T4.3** Wire terrain colors to material colors
- [ ] **T4.4** Test: grid renders in client window
- [ ] **T4.5** Test: click/hover returns correct hex under cursor

## Phase 5: Testing & Polish
- [ ] **T5.1** Performance test: 12,480 hexes at 60fps
- [ ] **T5.2** Visual test: grid looks correct (colors, shape, size)
- [ ] **T5.3** Edge case: empty grid (radius 0)
- [ ] **T5.4** Edge case: maximum grid size

## Verification
- [ ] All unit tests pass
- [ ] Grid generates 12,480 hexes for radius 100
- [ ] Terrain distribution matches spec probabilities
- [ ] Client renders grid without performance issues
- [ ] Deterministic: same seed produces identical grid
