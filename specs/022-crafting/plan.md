# Plan 022: Grass Regrowth, Craft Bench & Discovery Crafting

> **Implementation Plan**

## Architecture

Server-authoritative, slot-anchored. All new mechanics reuse the existing
`world_object` row (kind + mature_at + slot offsets) and the `player_item`
inventory. The client routes E-presses by held item + selected-slot contents;
the craft menu is a pure UI layer over one new reducer.

### Server (idlecore-server)
- `objects.rs`:
  - `plant_grass(ctx, address, hex_id, slot_x, slot_y)` — consumes 1 Grass,
    inserts `Grass` object with `mature_at = now + GRASS_GROWTH_SECS`.
  - `place_craft_bench(ctx, address, hex_id, slot_x, slot_y)` — consumes 1
    CraftBench item, inserts `CraftBench` object.
  - `craft(ctx, address, hex_id, slot_x, slot_y, ingredients)` — requires a
    CraftBench object at that exact slot; multiset-matches 4 ingredients;
    consumes + grants.
  - `gather_object`: Grass branch rejects immature (`mature_at`), same as trees.
  - Shared helper `require_empty_cell(hex_id, slot_x, slot_y)` — cell free of
    existing objects (used by plant_grass / place_craft_bench).
- `types.rs`: `GRASS_GROWTH_SECS = 75`, `OBJ_CRAFT_BENCH`, item consts
  (`ITEM_PICKAXE`…), recipe table + matcher.
- `lib.rs`: reducers `plant_grass`, `place_craft_bench`, `craft`.

### Bindings
- Regenerate `crates/idlecore-client/src/net/gen` via spacetime CLI.

### Client (idlecore-client)
- `net/plugin.rs` `interact_key_press` — routing on the selected slot:
  1. Tool held → tool action (Pickaxe→Rock, Axe→Tree, Shovel→Grass, Hoe→plant
     Wheat via the crop reducer), nearest matching node to the slot.
  2. CraftBench on the slot → open the craft menu (no reducer).
  3. Existing: gather node → harvest crop → clean.
  4. Held Grass → `plant_grass`; held CraftBench → `place_craft_bench`; held
     Seed → `plant_tree`.
- `net/craft.rs` (new) — craft menu UI: 2×2 grid, cell cycling through
  available ingredients (inventory-count-limited), Craft button, result line,
  ESC/E closes. No recipe hints.

## Deliveries
- **D1** — Server: grass regrowth (`plant_grass`, immature-gather rejection).
- **D2** — Server: bench + crafting (`place_craft_bench`, `craft`, recipes).
- **D3** — Bindings regeneration.
- **D4** — Client: selected-slot E routing (tools, grass, bench).
- **D5** — Client: craft menu UI + wiring; tests; build green.

## Testing Strategy
- Server unit tests: recipe matcher (known/unknown/order-insensitive),
  immature-grass rejection, cell-occupied rejection.
- Client tests: E-routing priority helpers (pure functions), craft-grid
  cycling logic.
- Manual: plant → wait → harvest loop; bench place → craft pickaxe.
