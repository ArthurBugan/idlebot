# Spec 035 — Classic TD path: fixed dirt trace from the frontier to the capital

> **Objective:** the classic tower-defense shape. Enemies spawn at ONE fixed
> point, walk a FIXED dirt path from the frontier to the capital, and never
> leave it. Defenses pinch the path: walls placed ON the path block and get
> chewed (sapper dps applies), towers placed near the path hit everything in
> reach while they walk the line.
>
> **Status:** spec 2026-11-02 (028 stays reserved for farming/wildlife).

## 1. Model

- The capital stamps its raid spawn + FULL MARCH TRACE once at placement.
  **REV #3 (playtest): random CURVED path, straight-line dead.** A pure
  generator walks OUT from the capital hex to a random length
  (`PATH_TARGET_MIN..=PATH_TARGET_MAX` = 6–9 hexes), spending a random
  bend budget (2–5 same-distance "sidesteps") to leave the straight line;
  the stored trace is that walk REVERSED — spawn hex first, capital hex
  last. Randomness is `ctx.rng()` (transaction-seeded); steps are
  land-only, unmaterialized hexes count as land, owned hexes skipped so
  raiders never spawn inside a claim.
- The trace is stamped ONCE and both sides consume it as-is:
  server `raid_march_target` steps along the stored nodes; client paints
  the ribbon through the stored hex centers. No re-derivation, and client
  and server agree by construction.
- Stored additively on `Capital`: `spawn_q: i32`, `spawn_r: i32`,
  `path_hexes: Vec<u64>` (empty trace = legacy capital → raiders keep the
  old free march).

## 2. Server

- **S1 — `place_capital`:** stamp spawn + trace on the capital row at
  placement and re-placement (`stamp_raid_path`). Every placement rolls a
  NEW length + bend count + direction.
- **S2 — raid march:** the marcher's target becomes the NEXT hex along the
  stored TRACE, not the capital itself (`raid_march_target` takes the node
  slice; empty = legacy). Re-entry helper mirrors the greedy: the step
  toward an adjacent fixed hex IS the path step, so `march()` and its
  WalledIn/trap machinery stay untouched. Walls on the path get chewed
  exactly as walls anywhere (sapper bonus intact).
- **S2b — REV (playtest):** the capital is a CONTACT target — every kind
  (ranged included) must walk the path until adjacent to the capital
  before damaging it (`engagement_target` gates the capital at dist ≤ 1;
  tower stand-offs keep their range). No more sniping from 3 hexes out.
- **S3 — `spawn_wave`:** one spawn point (the stored one) instead of two
  picked border points. Hunt-chase (aggro) stays free — but every raid
  mob re-enters the path via the nearest line node, so raids never stay off
  the trace.
- **S4 — rooted blooms and roamers unchanged** (bloom spawns inside; roamers
  wander, they are wildlife not raiders).

## 3. Client

- **C1 — dirt path render:** every line hex draws a ground overlay sprite
  (dirt crop from `models/Tileset/Path tiles.png`) just above the floor,
  below props. Static per capital: spawned with the capital row, dropped
  when the capital moves/destroys (tracked set, no per-frame work).
- **C2 — spawn pad:** the first path hex gets a slightly darker patch so
  the enemy origin reads at a glance.
- **C3 — REV (playtest):** the lane is a smooth floor-ribbon — slots whose
  centers sit within ~1.2 slots of the chain through the hex centers, not
  full 4×4 tile blocks (those read as a fat staircase). Spawn pad is a
  round blob on the first hex center (the stamped spawn). Painted by the
  floor builder from `RaidPathSlots`, same cells/scale as the ground (no
  stretched overlays). **REV #3:** the polyline is the SERVER-stamped
  curved trace, not the client-recomputed line; length is random 6–9.
  A mid-session placement despawns the lanes' live floor tiles and they
  respawn with dirt (live tiles never retexture on their own).

## 4. Non-goals

- No path costing beyond the land/owned walk filter — v2 went CURVED and
  random; water on a trace hex (vanishing-rare, splice fallback) just
  means the moisture step check fails and the mob chews/holds.
- Roamers/pursuit keep free movement.

## 5. Risks

- Additive schema: two new `Capital` columns — bindings regen once after
  publish. Legacy capitals keep playing with (0,0) sentinel.
