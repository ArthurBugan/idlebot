# Tasks 023: Tower Defense Pivot

> Build order is intentional: economy/constants first → land & capital →
> building → raids → combat → e2e. Check off progressively,
> **bindings regeneration after every server delivery**.

## Progress notes (session 3, D5 complete)
Client layer + e2e are green: core 372 / server 67 / client 13; the real
client boots (B0001 fixed — `Without`-disjoint UI queries in
`update_td_hud`); `bin/e2e.rs` now runs 14 phases against the self-hosted
server (prod budget 250 ms teleport RTT) and passes end-to-end including
the Spec 023 kit. Note for playtests: e2e throwaway capitals near the spawn
will start attracting (and eventually absorb) raid waves — they are
self-cleaning through the destruction cascade.

## D0 — Foundations
- [x] **T0.1** — `idlecore-core/src/defense_config.rs`: mob kinds, structure
      stats, claim/wave/grace constants (single source shared by server+client).
- [x] **T0.2** — Server `types.rs`: 5 new tables + 2 scheduled tables + item
      constants, mirroring Spec 019 style (`hex_id_of`, slot-friendly fields).
- [x] **T0.3** — Unit tests for wave-size formula, claim radius by level,
      tower dps/range math.

## D1 — Capital + Land Claim
- [x] **T1.1** — `capital.rs`: `place_capital` (spacing, water, single-use item),
      `claim_land`, `claimable()` pure validator.
- [x] **T1.2** — Destruction cascade `destroy_capital`: territory freed,
      structures removed, Capital Core refunded, `grace_until = +24h`.
- [x] **T1.3** — `new_player` grants `ITEM_CAPITAL_CORE` ×1.
- [x] **T1.4** — Reducer tests: double-claim race, adjacency, radius, terrain,
      spacing (two capitals < 8 hexes), claim cost consumption (pure
      validators; DB-level races need a mock-host harness).
- [x] **T1.5** — Bindings regen.
- [x] **T1.6** — Client: `place_capital`/`claim_land` reducers wired into
      `interact_key_press` (CapitalCore in hand + E founds the capital);
      claimed-hex ownership overlays (gold = mine → violet = others, via
      `net/td.rs` sync) + minimap tint (`td_tile_tint`).

## D2 — Defense Structures
- [x] **T2.1** — `defense.rs`: `build_defense` (owned hex, free slot, exact cost,
      `MAX_OBJECTS_PER_HEX` budget), `remove_defense` (50% refund).
- [x] **T2.2** — Reducer tests: unowned rejection, slot collision, refund math
      (pure refund/cost tests; slot/terrain checks live in the reducer).
- [x] **T2.3** — Bindings regen.
- [x] **T2.4** — Client: build mode (held ArrowTower/Ballista/BoneBarricade →
      `build_defense` on the selected slot), placeholder sprites with kind
      tints + HP bar children (Tiny Dungeon/art pack crops, no new assets).
      REV3: placement-hint pass `td_place_hints` — floor glows per armed
      action (capital yellow on legal hexes, structures green on owned hexes,
      claim blue on the territory ring), plus the missing `claim_land` E
      binding (empty hand + 5 Wood + 3 Stone, adjacent-owned).

## D3 — Raid Scheduling
- [x] **T3.1** — `scheduler.rs` + `mobs.rs::spawn_wave`: eligibility (grace
      windows, one active wave, offline-protection per user rev2 — removed),
      border spawn hexes, pressure-based `wave_size` (cap 12).
- [ ] **T3.2** — Scheduler tests: grace suppression, single-wave invariant,
      border-hex selection (reducer-level; needs mock-host harness).
- [x] **T3.3** — Client: raid banner (top-center, display toggles from the
      open-wave cache, raider count), minimap capital hex flashes red while
      a wave is open; ownership overlays breathe during raids.

## D4 — Mob Simulation + Tower Combat
- [x] **T4.1** — `mob_step`: 1s march toward capital, engagement priority
      (adjacent structure → capital), structure/capital damage application.
- [x] **T4.2** — Tower fire: nearest-mob targeting per stats, mob death →
      drops (`add_item`) + capital XP + level-up curve link.
- [x] **T4.3** — `mob_tick_body`/`raid_tick_body` wiring + raid end conditions
      (all dead / 15 min timeout / capital destroyed).
- [x] **T4.4** — Sim tests: engagement priority matrix, wave composition,
      dps-to-tick downs math; destruction cascade wired (full end-to-end
      sim test needs the DB harness).
- [x] **T4.5** — Bindings regen (`spacetime generate --bin-path` on the
      built wasm module; added place_capital/claim_land/build_defense/
      remove_defense + capital/territory/defense_structure/mob/raid_wave).
- [x] **T4.6** — Client: mob/structure/capital sprites through `net/td.rs`
      (dirty-flag sync like remote players), server-pinned HP bars
      (green→amber→red), "next raid in Xm" capital HUD panel, drop/kill fax
      surfaces through the existing toast line. REV2: bottom-anchored
      placement, full-figure art crops (house capital, complete scarecrow
      tower, stone chunk barricade), correct aspects; mob glide via the
      TdTracker observed-velocity pass.

## D5 — Polish & Ship
- [x] **T5.1** — e2e headless scenario (`bin/e2e.rs`, Spec 023 phases [10–14]):
      CapitalCore spawn kit, Earth-raster capital placement, territory
      auto-claim, claim/build resource-gate probes; tag-unique teleport
      targets so repeat runs never collide on.hex-occupancy arbitration.
- [x] **T5.2** — Regression: `cargo test` (core 372 / server 67→68 / client
      13)
      + `cargo clippy` clean on new code; full B0001 client-check resolved
      via `Without` disjoint queries in `update_td_hud`.
- [x] **T5.3** — README roadmap + feature list updated ("tower-defense layer:
      playable"). REV2: `debug_force_wave` reducer (owner-only, bypasses
      grace/reattack — balance-tuning button on the debug HUD panel);
      broken capital/tower/barricade crops replaced (full house / full
      scarecrow / stone chunk); MetalPart ingot icon; e2e plant phase made
      ring-aware + crowd-tolerant.
      REV4: **B-key build mode** (Barricade → ArrowTower → Ballista → off,
      ESC cancels; E places on owned hex; raw materials consumed directly —
      no tower item needed) + `TdBuildMode` resource, `td_build_keys`, and
      build-arm priority in `interact_key_press`; `td_place_hints` keyed on
      build mode > held item > claim; server marches now spawn ≥
      `MIN_MARCH_HEXES` (3) out via `walk_out` + pure `step_toward_hex` so
      raiders visibly trek to the capital (unit test: distance strictly
      decreases, march reaches capital); server tests 67→68; published to
      `db.nestfeed.app`.
- [ ] **T5.4** — `review.md` after a shared playtest pass.
