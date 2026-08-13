# Tasks 016: Assets (Placeholder → Low-Poly → Polish)

> **Implementation Checklist**

## Phase 1: Procedural Placeholders (MVP)
- [x] **T1.1** Hex material colors — terrain tint per biome in world_mesh/chunk mesh colors
- [ ] **T1.2** Verify player mesh (tetrahedron placeholder) exists
- [x] **T1.3** Plant meshes — cone/tall-cone primitives spawned per planted hex
- [ ] **T1.4** Verify terrain colors match spec (#7EC850, #228B22, etc.)
- [x] **T1.5** Pollution visible — dark disc marker on polluted hexes

## Phase 2: Asset Loading Infrastructure
- [ ] **T2.1** Define AssetHandle type for glTF models
- [ ] **T2.2** Implement load_vehicle_assets() — spawn meshes from asset server
- [ ] **T2.3** Implement load_cosmetic_assets() — hats, auras, trails
- [x] **T2.4** load_plant_assets — shared cone meshes + young/mature materials (FloorPlantAssets)
- [x] **T2.5** Terrain materials — biome-colored chunk meshes (world_floor)
- [x] **T2.6** Asset loading — FloorPlantAssets ensured lazily in update_plant_visuals

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
