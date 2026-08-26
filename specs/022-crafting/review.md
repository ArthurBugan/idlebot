# Review 022: Grass Regrowth, Logs, Craft Bench & Discovery Crafting

> Implementation report for Spec 022 (`specs/022-crafting/spec.md`).

## Status: COMPLETE

All five design areas are implemented and covered by tests. `cargo test --workspace`
passes (358 core + 56 server + 47 client tests, 0 failures).

## What shipped

### 1. Grass regrowth (§1)
- `plant_grass` reducer (server `objects.rs`) consumes 1 Grass and plants a
  growing `Grass` node with `mature_at = now + GRASS_GROWTH_SECS` (75 s).
- `gather_object` now rejects immature grass via the shared `growth_block`
  helper (same "still growing" message as trees). Mature grass still drops
  2 Grass + the 55% seed chance.
- Client renders immature grass as a small green-tinted sprout (sapling path).

### 2. Logs (§2)
- `natural_spawn` rolls fallen `Log` nodes on forest-floor terrains (Forest /
  Taiga / TropicalRainforest) only, in the same grass slot band with a
  deterministic ~8% rate. Existing `logs_spawn_only_in_forested_terrains` test
  pins this down.
- `gather_object` `OBJ_LOG` branch drops 1–2 logs (deterministic by object id)
  and frees the slot via the standard tombstone/respawn flow.
- Logs are the bootstrap resource: 4 build the first bench.

### 3. Craft bench (§3)
- `place_craft_bench` consumes exactly 4 logs and inserts a `CraftBench` world
  object on an empty plot of an adjacent hex — no bench item exists, placing
  IS building. Renders as the Tiny Dungeon crate tile, taller than plants.
- Pressing E on the bench's slot opens the craft menu instead of gathering.

### 4. Discovery crafting (§4)
- `craft` reducer matches the order-insensitive multiset of exactly four
  ingredients against the fixed recipe table, with `Log` normalized to `Wood`
  before comparison. Unknown combinations return "Nothing happened." and
  consume nothing; the bench stays.
- Recipe table + `match_recipe` + `normalize_ingredient` live in `types.rs`
  (`RECIPES`, `CRAFT_INGREDIENTS`); covered by order-insensitive, log-sub, and
  negative tests.
- Client `net/craft.rs`: 2×2 grid, click a cell to cycle through carried
  ingredients, Craft button. No recipe hints anywhere in the UI.

### 5. Selected-plot invariant (§5 / tools)
- `interact_key_press` (client `net/plugin.rs`) routes by held item first:
  Pickaxe→Rock, Axe→mature Tree, Shovel→Grass tuft, Hoe→till Wheat (consuming
  a Seed via the new `till` reducer, reusing the crop system). Tool actions
  target the nearest matching node in the selected hex; the bench opens the
  menu; then the existing gather → harvest → clean → plant-by-held-item chain
  runs. Maturity checks use the time comparison (not just `mature_at == 0`).

## Acceptance criteria

1. Planting grass on an empty plot creates a growing node at the slot center.
   ✅ `plant_grass` + `require_empty_cell`.
2. Gather before `mature_at` fails; after succeeds and frees the slot.
   ✅ `growth_block` gate; tested in `growth_gate_blocks_only_before_maturity`.
3. Log entities spawn in forested terrains and drop Log items when gathered.
   ✅ `natural_spawn` + `OBJ_LOG` gather branch; tested.
4. Placing a bench on an empty plot with ≥4 Logs consumes exactly 4 and
   renders. ✅ `place_craft_bench` (4-log cost, no bench item).
5. Crafting a known recipe consumes ingredients (Log ≡ Wood) and grants the
   tool; unknown combos consume nothing and reveal nothing.
   ✅ `craft` + `match_recipe`; server + client tests.
6. Tool + E always applies to the selected slot's contents, never another hex.
   ✅ tool routing targets the selected hex; range check enforces 1-hex.

## Notes / non-goals honored
- No recipe persistence across sessions, no tool durability/tiers, no new
  sprites beyond reusing existing Tiny* tiles (Farm dead-wood = Log, Dungeon
  crate = bench, Town/Town+Farm tools).
- `move_player`/movement.rs warnings and other clippy lints are pre-existing
  and out of scope for this spec.
