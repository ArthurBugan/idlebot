# Plan 016: Assets (Placeholder → Low-Poly → Polish)

> **Implementation Plan**

## Architecture

### Phase 1: Procedural Placeholders (MVP)
- Hexes: colored planes with terrain colors
- Player: tetrahedron mesh (4 vertices, 4 faces)
- Plants: cone meshes (3 vertices each)
- Pollution: dark colored sprites

### Phase 2: Low-Poly Assets
- 5 vehicle models (GLTF format)
- Cosmetic models (hats, auras, trails)
- Plant models (Wheat, Tree, RareHerb)
- Terrain materials (6 types, PBR)
- All assets < 500 triangles, textures < 512x512

### Phase 3: Animations and VFX
- Vehicle animations (5 types)
- Cosmetic animations (optional)
- Plant growth animations
- Particle effects (aura, trails)

## Files to Create/Modify

### Client (idlecore-client)
- `src/assets/procedural.rs` — Placeholder mesh generation
- `src/assets/low_poly.rs` — GLTF asset loading (Phase 2)
- `src/animation.rs` — Animation system (Phase 3)
- `src/particles.rs` — Particle effects (Phase 3)

### Core (idlecore-core)
- `src/terrain.rs` — Terrain material definitions

## Testing Strategy
1. Visual test: Placeholder assets render correctly
2. Visual test: Low-poly assets load and display
3. Performance test: Asset loading < 100ms per asset
4. Animation test: Animations play at 30fps

## Dependencies
- Depends on 002-hex-grid (terrain rendering)
- Depends on 006-vehicles (vehicle models)
- Depends on 007-cosmetics (cosmetic models)
- Requires Blender/asset pipeline (external)

## Timeline
- **Phase 1 (MVP):** 1 day (procedural placeholders)
- **Phase 2:** 3-5 days (low-poly assets)
- **Phase 3:** 2-3 days (animations, VFX)
