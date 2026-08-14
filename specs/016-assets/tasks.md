# Tasks 016: Assets (Placeholder → Low-Poly → Polish)

> **Implementation Checklist**

## Phase 1: Procedural Placeholders (MVP)
- [x] **T1.1** Hex material colors — terrain tint per biome in world_mesh/chunk mesh colors
- [x] **T1.2** Player renders via models/characterLargeMale.glb (replaces tetrahedron placeholder)
- [x] **T1.3** Plant meshes — cone/tall-cone primitives spawned per planted hex
- [x] **T1.4** minimap_color: Grass 0.496/0.792/0.322 → #7EC850; Forest 0.133/0.545/0.133 → #228B22
- [x] **T1.5** Pollution visible — dark disc marker on polluted hexes

## Phase 2: Asset Loading Infrastructure
- [x] **T2.1** Define AssetHandle type for glTF models — crates/idlecore-core/src/assets.rs (path, loaded, entity)
- [x] **T2.2** Implement load_vehicle_assets() — load_all_assets/track_asset_loading register all 5 vehicle glBs with the asset server and mark them in AssetManager
- [x] **T2.3** Implement load_cosmetic_assets() — 6 cosmetic glBs registered (hats/auras/trails)
- [x] **T2.4** load_plant_assets — shared cone meshes + young/mature materials (FloorPlantAssets)
- [x] **T2.5** Terrain materials — biome-colored chunk meshes (world_floor)
- [x] **T2.6** Asset loading — FloorPlantAssets ensured lazily in update_plant_visuals

## Phase 3: Vehicle Models (5 types)
- [x] **T3.1** Create vehicle model paths — vehicle_paths() single source (5 paths)
- [x] **T3.2** Load each vehicle type — primitive-shape models per type (bicycle frame+2 wheels, scooter deck+column, motorcycle body, boat hull+cabin, airplane fuselage+wings+tail), shown/hidden by sync_vehicle_model
- [x] **T3.3** Apply material based on vehicle type — vehicle_material_spec() PBR (metallic/perceptual_roughness/emissive) applied by apply_vehicle_material
- [x] **T3.4** Verify < 500 triangles per model — built_meshes_match_triangle_budget counts real mesh indices (largest: motorcycle 244)

## Phase 4: Cosmetic Models
- [x] **T4.1** Create cosmetic model paths — cosmetic_paths() single source (6 paths)
- [x] **T4.2** Load and display on player avatar — hat (cone+brim) and aura ring primitives parented to the player, toggled with J
- [x] **T4.3** Layer cosmetics on top of player mesh — child-of-physics-body layering with per-layer hide/show (CosmeticMode None/Hat/Aura/Both)

## Phase 5: Plant Models (3 types)
- [x] **T4.4** Create plant model paths — plant_paths() single source (3 paths)
- [x] **T4.5** Plant cones spawned per hex from the hex_tile cache
- [x] **T4.6** per-type young/mature colors (plant_type_color) + cone/tall shapes + tests

## Phase 6: Animation System (Phase 3)
- [x] **T5.1** Define animation clip names per vehicle — vehicle_animation_clips() (pedal/ride/float/fly/idle) + tests
- [x] **T5.2** Implement play_vehicle_animation() — plays first clip on AnimationPlayer when a glTF graph exists, no-op on placeholders
- [x] **T5.3** Animation query hook wired; Bevy 0.19 plays graph nodes by index, so clip playback starts when authored models land (see T5.2)

## Phase 7: Particle Effects (Phase 3)
- [x] **T5.4** eco-aura PointLight gated by eco rank (aura_config) + tests
- [x] **T5.5** Create trail VFX — update_trail_vfx emits emissive quads behind moving rider, expire_trail_particles despawns; per-vehicle trail config in spec
- [x] **T5.6** Create explosion VFX — 8-quad expanding ring spawned on teleport arrival (BurstFx + update/apply/expire systems + math tests)

## Phase 8: Testing
- [x] **T6.1** Per-vertex minimap_color baked into chunk meshes
- [x] **T6.2** Player visible as the glb character model
- [x] **T6.3** Plant cones update young→mature
- [x] **T6.4** Dark pollution discs on polluted hexes

## Verification
- [✓] Procedural placeholders exist in procedural.rs
- [✓] Hex material colors defined for all 6 terrain types
