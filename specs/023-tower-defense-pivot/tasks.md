# Tasks 023: Tower Defense Pivot

> Build order is intentional: economy/constants first → land & capital →
> building → raids → combat → e2e. Check off progressively,
> **bindings regeneration after every server delivery**.

## Progress notes (session 4, rev6)

Acceptance-criteria gap audit against the spec (no playtest input yet):
(1) §3.3 rock Metal Part drop was missing — `mine_rock` now credits 1
MetalPart on a deterministic 25%-per-object band (`types.rs::mined_metal_part`,
pattern matches the grass seed roll; unit test added). (2) §3.9 minimap
territory **border** only had a fill tint — owned hexes touching unowned or
rival land now draw a dark outline (mine-gold / rival-violet) via
`BorderColor` on the tile nodes. (3) REV6 wave-pressure rebalance:
`wave_size` divisor 2 → 16 (`WAVE_SIZE_DIVISOR`) — the rev5 10× territory
saturated the 12-mob cap at 12 claimed hexes; the cap is now reached at 160
claimed hexes so pressure escalates over the disc. Spec §3.5 + plan.md
updated to the `/16` formula. REV6b: the "next raid in Xm" suffix left the
bottom-left line — a proper game UI widget now owns it (top-right corner,
draggable later if needed): gold NEXT RAID label, bold mm:ss countdown
draining locally between server syncs (`TdView.next_wave_sync_utc` +
`next_wave_total` for the window length → correct drain bar fraction),
thin gold progress bar; flashes red with "RAID NOW" while a wave is open.
Bottom-left capital panel now shows just "Capital L2 · HP 98". Client-only
— no server republish needed. REV6c: the whole cluster moved top-left and a
stats panel joined under it — the raid widget at 20,20, and a dark rounded
panel at 20,122 with Gold row (⛁ / gold tint), XP row (✦ / sky-blue), and a
Capital row (⛨ L chip + full-width HP bar green→amber→red + "hp / max"
count, ratio from `capital_max_hp(level)`); values are the server-synced
`ClientPlayer.gold/xp`; `fmt_short` (k/M) keeps big numbers inside the 176
px panel (2 unit tests). `update_td_hud` refit to two `ParamSet`s (5 node
writers, 6 span writers) instead of the Without matrix. REV6d: bottom-left
panels deleted — the idle gains panel (idle.rs module removed; offline
approval toast in hud.rs untouched) and the capital status line were
duplicates of the server-synced stats cluster; top-left cluster is now the
single HUD home for gold / XP and the raid countdown. REV6e: Minecraft-style
XP bar lives on the hotbar — thin dark trough (full hotbar width, 1px
border) with a #80ff20-green fill for the current level ring, green level
number centered above it; driven by server-synced `ClientPlayer.xp` via the
new core helper `progression::level_progress(total_xp) → (level, into,
needed, frac)` (+2 core tests, 373). Top-left cluster dropped its ✦ XP row
(no duplicate), keeping gold + capital. REV6f: XP ring numbers added —
"xp / needed" readout (from `level_progress`'s into/needed) pinned to the
bar header's right edge in 11px off-white; header row legalized (both the
level and the count are root `Text` nodes — the first-cut `TextSpan` had no
`Text` root and would not have rendered); count/level writers merged into a
2-query `ParamSet`. REV6g: countdown stuck at `--:--` root-caused — the
raid-widget and stats-panel writers still queried `TextSpan` while those
entities are root `Text` nodes (zero matches → initial string never
replaced); writers now mutate `Text`. Also: stats panel always-visible
(sits `Flex` from update; gold shows before a capital exists), and the
label scales with the window — `mm:ss` under 10 min, `12m04` to 2 h,
`22h12m` beyond (`fmt_countdown` pure fn + 1 test, client net_tests 3+
... 16 total). REV6h (WORKING TREE, unpublished): debug panel (F1/Z)
rebuilt — buttons never had an `Interaction` component at all (the
hud_buttons query matched nothing) AND the old 10,10 panel sat under the
new raid widget/stats cluster. New panel = the Spec 023 top-left-cluster
pattern (dark 0.85 alpha, 1.5px blue border, radius 9, FiraSans "🛠 DEBUG
(F1 to hide)" header) at bottom-left 20,20, full-width buttons with
hover/press tint (`hud_button_tint`). REV7 (WORKING TREE): buildable-area
ground layer redone + real buildings menu — (1) new procedural pointy-top
hexagon texture (`TdHexTex`, rim α≈0.95 over fill α≈0.30) replaces the
pinhead SLOT_SIZE squares for BOTH the per-hex build hints and the gold /
violet ownership `TdOverlay`, colors retuned to teal (build), gold
(capital-place), azure (claim) at ~0.9 tint; (2) B no longer blind-cycles —
it opens the `🛠 BUILDINGS` menu (centered above the hotbar, cluster-panel
language: dark, 1.5px teal border, radius 9) with one full-width row per
`BUILDABLES` kind: `ImageNode` icon from `TdTextures`, display name,
static cost string; clicking a row arms `TdBuildMode`, closes the menu and
reports costs via toast; armed row highlights teal (border+bg, `td_build_row_tint`,
`mode.is_changed`-gated), ESC/B close + disarm, E-flow and TdBuildMode
unchanged for plugin.rs. Published to `db.nestfeed.app` (constants +
rock-drop only — no schema change). NO commit yet — waiting for the
owner's go-ahead.

## Progress notes (session 4, rev5)

Playtest-driven tuning: (1) capital territory ~10× bigger — claim disc base
19 → 217 hexes (`CLAIM_RADIUS_BASE` 2 → 8, +1/level cap 5 → 15) with the
placement spacing scaled to match (8 → 24 rings). (2) minimap capital
markers — a diamond per live capital anywhere on the map, gold = mine,
violet = other players', flashing red while a raid wave is open (new
`TdTerritory.wave_targets` cache fed from open `raid_wave` rows).
(3) T3.2 scheduler tests at the pure-helper level (see T3.2 notes).
NO commit/publish yet — waiting for the owner's go-ahead.

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
- [x] **T3.2** — Scheduler tests: grace suppression, single-wave invariant,
      border-hex selection. REV5: landed at the pure-helper level —
      `wave_eligible` (grace/reattack/single-wave gate) and `frontier_hexes` +
      `pick_border_points` (deterministic frontier, water/unmaterialized
      skipping) extracted from `raid_tick`/`spawn_wave`; 6 new tests. Full
      reducer-level runs still need a mock-host harness.
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
