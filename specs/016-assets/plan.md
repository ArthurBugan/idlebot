# Plan 016: Assets (Placeholder → Low-Poly → Polish)

> **Implementation Plan**

## Architecture

### Three-Phase Strategy
**Phase 1 (MVP):** Bevy primitive meshes (cubes, cones, tetrahedrons) for gameplay
**Phase 2:** Import low-poly glTF assets (vehicles, cosmetics, plants, terrain)
**Phase 3:** Add animations and particle effects (VFX)

### Asset Pipeline
- Procedural generation → glTF conversion → Bevy asset server
- Texture optimization: 256x256 or 512x512 max
- Mesh optimization: < 500 triangles per model
- PBR materials (metallic, roughness)

## Files to Create/Modify

### Client (idlecore-client)
- `src/assets/procedural.rs` — Already exists, procedural placeholder assets
- `src/world/hex_renderer.rs` — Add terrain material colors
- `src/world/map_generator.rs` — Modify to use low-poly assets

### New Files
- `src/assets/models.rs` — Asset loading for glTF models
- `src/assets/animations.rs` — Vehicle animations, particle effects

## Dependencies
- Requires Phase 1 (MVP) to be complete and verified
- Low-poly assets to be created externally (Blender, etc.)

## Testing Strategy
1. Unit test: Procedural placeholder meshes render
2. Integration test: Terrain colors display correctly
3. Unit test: glTF loading (once assets exist)
4. Performance test: Asset loading < 100ms per asset

## Timeline
- **Phase 1:** Already partially done (procedural.rs exists)
- **Phase 2:** 2-3 days (asset creation + import)
- **Phase 3:** 3-5 days (animations + VFX)

## Ponytail Note
ponytail: Skipping low-poly asset creation (external Blender work). Focus on procedural placeholders that work for MVP.
