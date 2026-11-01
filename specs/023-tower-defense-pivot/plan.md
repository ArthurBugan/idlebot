# Plan 023: Tower Defense Pivot — Land Claim, Capital City, Raids

> **Implementation Plan** — how Spec 023 lands in the codebase, mapped onto
> existing modules.

## Architecture

**Server-authoritative simulation, SpacetimeDB scheduled ticks, client renders.**
All combat/claim decisions happen in the server module; the client is a pure
viewer + input layer (same trust model as `movement.rs`). New tables replicate
via the existing `subscribe_to_all_tables()` — no protocol work beyond reducer
bindings.

### Data flow

```
claim_land / build_defense        (player reducers, item + territory checks)
raid_tick (30s) ─┬→ spawns raid_wave + mob rows at border hexes
                 └→ destruction cascade when capital hp ≤ 0
mob_tick (1s)   ─┬→ mob step/engagement (melee structures → capital)
                 └→ tower fire → mob damage → death → drops → capital xp
```

### Server (idlecore-server) — file map

- **`types.rs`**
  - New tables: `capital`, `territory`, `defense_structure`, `mob`, `raid_wave`,
    scheduled `scheduled_mob_tick` / `scheduled_raid_tick` (+ their `#[reducer]`
    wrappers, per the `scheduled_plant_growth` pattern).
  - New item constants: `ITEM_CAPITAL_CORE`, `ITEM_METAL_PART`, `ITEM_BONE`,
    `ITEM_ARROW_TOWER`, `ITEM_BALLISTA`, `ITEM_BONE_BARRICADE` (hotbar-held placables).
  - Mechanics constants: `CAPITAL_HP_BASE=100`, `MIN_CAPITAL_SPACING=24`,
    `CLAIM_RADIUS_BASE=8` (rev5: ~10× territory disc, 217 hexes; cap 15),
    `CLAIM_COST` (5 Wood + 3 Stone),
    `GRACE_FIRST_RAID_SECS=1800`, `GRACE_REBUILD_SECS=86400`,
    `WAVE_MAX_MOBS=12`, `WAVE_SIZE_DIVISOR=16` (REV6: old /2 saturated the
    cap at 12 claims on the rev5-sized territory), `WAVE_REATTACK_SECS=600`,
    `MOB_TICK=1s`, `RAID_TICK=30s`.
  - Mob & structure stat tables (kinds → hp/dps/drop) as pure structs/constants
    — unit-testable without SpacetimeDB.
- **`capital.rs`** (new)
  - `place_capital`, destruction cascade `destroy_capital`, `capital_xp/level`
    helpers reusing `progression::calculate_level`-style curve.
  - `claimable(owner, hex_id, capital) -> Result<ClaimCheck>` (pure): adjacency,
    radius, ownership, terrain walkability via `TerrainType::is_walkable`.
  - `claim_land` reducer (thin wrapper in `lib.rs`).
- **`defense.rs`** (new)
  - `build_defense`, `remove_defense`; reuses `require_empty_cell` from
    `objects.rs` and the `player_item` ledger helpers (`add_item/remove_item`).
  - `structure_stats(kind)` + `tower_fire(holder, mob) -> damage` pure helpers.
- **`mobs.rs`** (new)
  - `spawn_wave(capital)` — picks 1–2 border hexes (`territory` scan), inserts
    `raid_wave` + up to `WAVE_MAX_MOBS` mob rows.
  - `mob_step(mob, structures, capital)` — engagement priority logic (pure, tested).
  - Tower targeting per `mob_tick` body; drop crediting via `add_item`.
- **`scheduler.rs`** — `mob_tick_body` (1s), `raid_tick_body` (30s) bodies +
  registration alongside the 5 existing schedulers in `init` (lib.rs). Raid
  eligibility: grace window passed, no active wave, defender not protected
  (offline > 24h rule from Spec §5).
- **`world.rs`** — `ensure_hex` gains a 25% Metal Part roll for Rock gathers via
  `objects.rs::gather_object` drop table; nothing else changes.

### Cross-module invariants (must be enforced server-side, all race-checked)
- A hex can be claimed iff `territory` has no row for `hex_id` AND it's
  adjacent to an owned hex within the claim radius.
- Building iff owner of `hex_id == caller` AND free slot AND full inventory cost.
- One wave per capital (`raid_wave.ended_at IS NULL` check inside raid tick).
- Max 12 live `mob` rows per `capital_id` at any time.

### Bindings
- Regenerate `crates/idlecore-client/src/net/gen` after each server delivery:
  `spacetime generate --lang rust --module-path crates/idlecore-server
  --out-dir crates/idlecore-client/src/net/gen`.

### Client (idlecore-client) — file map

- **`net/territory.rs`** (new plugin) — reads `capital`, `territory`,
  `defense_structure`, `mob`, `raid_wave` caches each frame (dirty-flag pattern
  like `players_dirty` in `net/plugin.rs`):
  - claimed-hex tint collection consumed by `world_floor.rs`;
  - mob marker interpolation (glide between 1s ticks — reuse remote-player
    `MarkerMove` approach);
  - wave-arrival events for HUD/minimap.
- **`world_floor.rs`** — owner tint overlay for `territory` rows (visible hexes).
- **`plugins/player.rs`** — extend `interact_key_press`: held `Capital Core` →
  `place_capital`; held structure item → `build_defense` on selected slot.
- **`net/hud.rs`** — capital HP/level widget, raid countdown banner, drop toasts.
- **`minimap.rs`** — territory border strokes, capital marker, wave pulse.
- **Sprites** — Tiny Dungeon: monster tiles for the 3 mob kinds, crate→
  barricade, existing tiles for towers (placeholder rule per repository style:
  no new art assets in MVP).

## Deliveries (each = server tables+logic, then bindings, then client rendering)

- **D0** — shared constants + stat tables in core (`idlecore-core/src/defense_config.rs`),
  server consumes them; unit tests for balance math (wave size, dps/range math).
- **D1** — server: schema (all 5 tables + 2 scheduled), `place_capital`,
  `claim_land`, destruction cascade. Bindings + client place/claim UI.
- **D2** — server: `build_defense`/`remove_defense`; client build mode rendering.
- **D3** — server: raid scheduler `raid_tick_body` + spawn logic; client wave banner.
- **D4** — server: `mob_tick_body` + engagement/tower fire/drops/xp; client mob
  sprites + HP bars; minimap territory/borders.
- **D5** — e2e headless scenario (login → place → claim → build → forced wave),
  `review.md`, README roadmap toggle for tower-defense layer.

## Testing Strategy

- **Pure unit tests (no DB):** claimability matrix, wave-size scaling,
  mob engagement priority, tower fire range/dps, destruction cascade ordering.
- **SpacetimeDB reducer tests:** double-claim race → one winner; claim without
  adjacent territory → reject; building on unowned hex → reject; wave spawn
  eligibility (grace/protection/one-wave rules).
- **Client:** `interact_key_press` routing for Capital Core + structure items;
  mob marker interpolation math; territory tint over visible hexes.
- **e2e:** extend `crates/idlecore-client/src/bin/e2e.rs` with the Spec 023 flow.
- **Regression:** full `cargo test --workspace` — farm/idle/market suites must
  stay green (Constitution 6/7).
