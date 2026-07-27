# Plan 002: Hex Grid Generation and Rendering

> **Implementation Plan**

## Architecture

### Grid Generation
- Hex coordinate system using axial coordinates (q, r, s where q + r + s = 0)
- Grid bounded to radius 100 (12,480+ hexes)
- Seed-based deterministic generation

### Terrain Assignment
```rust
enum TerrainType {
    Grass,      // 50% - eco_rating: 50, color: #7EC850
    Forest,     // 20% - eco_rating: 50, color: #228B22
    Water,      // 8%  - eco_rating: 20, color: #4169E1
    City,       // 10% - eco_rating: 20, color: #808080
    Desert,     // 7%  - eco_rating: 20, color: #F4A460
    Polluted,   // 5%  - eco_rating: 10, color: #4B0082
}
```

### Rendering Strategy
- Bevy instanced meshes for all hexes (single draw call)
- Flat-top hex geometry with terrain-based vertex colors
- Grid queries: hex_at(position), neighbors(hex_id)

## Files to Create/Modify

### Core (idlecore-core)
- `src/hex.rs` — HexCoord struct with to_pixel(), to_id(), neighbor calculation
- `src/grid.rs` — HexGrid struct with generation, queries, updates
- `src/hex_tile.rs` — HexTile data structure
- `src/lib.rs` — Export new modules

### Server (idlecore-server)
- `src/world.rs` — World state with hex grid data
- Add hex_grid to types.rs table schema (already has HexTileDbEntry)

### Client (idlecore-client)
- `src/world/map_generator.rs` — Replace with seeded generation
- `src/world/hex_renderer.rs` — Bevy instanced rendering system
- `src/main.rs` — Wire up map generation + rendering systems

## Testing Strategy
1. Unit test: grid generates correct hex count (12,480 for radius 100)
2. Unit test: terrain distribution matches probabilities (50% ±5% for Grass)
3. Unit test: deterministic generation (same seed = same grid)
4. Unit test: hex queries return correct hexes
5. Visual test: grid renders in Bevy client

## Dependencies
- Depends on 003-player-spawn (player needs grid for spawn location)
- Depends on 004-interactions (interactions need grid for hex selection)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** MVP Core Loop
