# Tasks 022: Grass Regrowth, Logs, Craft Bench & Discovery Crafting

- [x] **D1 — Server grass regrowth**: `plant_grass` reducer; `growth_block` gate
      on `gather_object` for Grass (mirrors trees); `GRASS_GROWTH_SECS = 75`.
- [x] **D2 — Server bench + crafting**: `place_craft_bench` (4-log cost, no
      bench item); `craft` reducer; `RECIPES` + `match_recipe` + `normalize_ingredient`
      (Log ≡ Wood) in `types.rs`; `till` reducer (Hoe → Wheat consuming a Seed).
- [x] **D3 — Bindings**: `spacetime generate --lang rust --module-path
      crates/idlecore-server --out-dir crates/idlecore-client/src/net/gen`
      (adds `plant_grass`/`place_craft_bench`/`craft`/`till` reducers only).
- [x] **D4 — Client selected-slot E routing**: rewrite `interact_key_press` —
      tool actions by held item, bench opens menu, gather/harvest/clean,
      Grass→plant_grass / Seed→plant_tree / 4+ logs→place_craft_bench.
- [x] **D5 — Client craft menu + wiring + tests**: new `net/craft.rs`
      (2×2 grid, cycling cells, Craft button, ESC/E close); sprites/icons for
      Log, bench and the four tools; `update_world_object_visuals` arms;
      `Inventory` icons/badges; registered `CraftPlugin`; `cargo test` green.

## Verification
- `cargo test --workspace`: all pass (358 core + 56 server + 47 client).
- `cargo clippy --workspace`: only pre-existing style lints remain.
