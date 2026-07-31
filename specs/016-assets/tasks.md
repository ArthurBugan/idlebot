# Tasks 016: Assets (Placeholder → Low-Poly → Polish)

> **Implementation Checklist**

## Phase 1: Procedural Placeholders (MVP)
- [ ] **T1.1** Verify hex material colors defined (Grass, Forest, Water, City, Desert, Polluted)
- [ ] **T1.2** Verify player mesh (tetrahedron placeholder) exists
- [ ] **T1.3** Verify plant meshes (simple cones) exist
- [ ] **T1.4** Verify terrain colors match spec (#7EC850, #228B22, etc.)
- [ ] **T1.5** Verify pollution visible (dark markers)

## Phase 2: Asset Loading Infrastructure
- [ ] **T2.1** Define AssetHandle type for glTF models
- [ ] **T2.2** Implement load_vehicle_assets() — spawn meshes from asset server
- [ ] **T2.3** Implement load_cosmetic_assets() — hats, auras, trails
- [ ] **T2.4** Implement load_plant_assets() — Wheat, Tree, RareHerb
- [ ] **T2.5** Implement load_terrain_materials() — 6 terrain colors
- [ ] **T2.6** Add asset loading to Bevy app initialization

## Phase 3: Vehicle Models (5 types)
- [ ] **T3.1** Create vehicle model paths (vehicles/bicycle.glb, etc.)
- [ ] **T3.2** Load each vehicle type
- [ ] **T3.3** Apply material based on vehicle type (metallic, roughness)
- [ ] **T3.4** Verify < 500 triangles per model

## Phase 4: Cosmetic Models
- [ ] **T4.1** Create cosmetic model paths (cosmetics/hat_basic.glb, etc.)
- [ ] **T4.2** Load and display on player avatar
- [ ] **T4.3** Layer cosmetics on top of player mesh

## Phase 5: Plant Models (3 types)
- [ ] **T4.4** Create plant model paths (plants/wheat.glb, tree.glb, rare_herb.glb)
- [ ] **T4.5** Spawn plants on hex tiles
- [ ] **T4.6** Different colors/shapes per plant type

## Phase 6: Animation System (Phase 3)
- [ ] **T5.1** Define animation clip names per vehicle (pedal, idle, ride, float, fly)
- [ ] **T5.2** Implement play_vehicle_animation()
- [ ] **T5.3** Add vehicle animation query system

## Phase 7: Particle Effects (Phase 3)
- [ ] **T5.4** Create aura VFX (point light around player)
- [ ] **T5.5** Create trail VFX (line of particles behind player)
- [ ] **T5.6** Create explosion VFX (optional)

## Phase 8: Testing
- [ ] **T6.1** Hexes render with terrain colors
- [ ] **T6.2** Player visible as tetrahedron
- [ ] **T6.3** Plants visible (simple cones)
- [ ] **T6.4** Polluted hexes visible (dark markers)

## Verification
- [✓] Procedural placeholders exist in procedural.rs
- [✓] Hex material colors defined for all 6 terrain types
