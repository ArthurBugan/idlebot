# 025 tasks

## D0 — core roster + mechanics fields

- [x] `MobKind` += `range_hexes`, `splash`, `wall_dps` (+`ITEM_SLIME_GEL`)
- [x] 8-kind roster replaces Scavenger/BoneWalker/RustHusk (§3 tables)
- [x] composition: cycle + boss cadence helper (`inject_boss`, every 4th wave ≥ L4)
- [x] core tests: roster integrity / eligibility / cadence

## D1 — server tick

- [x] engagement by `range_hexes` (melee ≤1 unchanged); ranged mobs hold position
- [x] splash (Bomb Goblin): half dps to every other structure ≤1 hex
- [x] sapper (Spear Goblin): +`wall_dps` vs Bone Barricade
- [x] rooted (Venom Bloom): spawns inside territory ring ≤2, never marches
- [x] boss spawn replaces the marcher slot per cadence
- [x] server tests: ranged pick / splash / sapper / rooted no-step / unknown kinds self-delete

## D2 — client art + animation

- [x] atlas crops: idle + walk w0/w1 per kind (32×32 frames; venom 48×48 rooted)
- [x] `TdTextures` kind→sprite map, neutral tints (frog/crow/ostrich placeholders die)
- [x] walk/idle frame flip 0.25 s while gliding; happy state = idle frame
- [x] damage flash: red 0.15s on HP decrease; HP bars unchanged

## D3 — build/publish + suite

- [x] `spacetime build` + publish with schema (mob rows with legacy kinds self-delete on first tick)
- [x] full verification: core / server / client net tests + wasm32 + clippy + boot

## D4 — playtest + review

- [x] shared playtest (`review.md`) — gated, as 023/024

## D5 — REV3 siege legibility (this revision, §8–13)

- [x] **server `combat_event` table** (§10): kinds, writer inside `tick`,
      scheduled scrub > 30 s; audit entry keeps a fired-count.
- [x] **walls block the march** (§11): passability filter in `march`,
      nearest-wall engagement fallback (chew range 1), breach resumes; tests.
- [x] **melee encircle ring** (§12): deterministic ring spot per mob id
      when ≥ 3 melee jostle one target; test.
- [x] **client fx per event kind** (§13): tracer/puff/bomb/splash/fell/
      capital-flash/toast + capital HP bar gold-pulse on hits.
      (Deviation: the capital reads via a strong red flicker, no transform
      shake — shake would drift the marker; revisit if wanted.)
- [x] **`attacking` animation state** on `TdMarker`: white-pulse fallback
      (§13 fallback clause), armed from `combat_event` on every mob-act.
- [x] **wave opener banner** with pattern name (§14) on wave open:
      "Wave N — Pattern: X raiders march".
- [x] suites: server tests for event wiring (pure pickers pinned: step
      avoid/wall pick/encircle; client exhaustive `kind→fx` + banner cadence);
      build + publish (non-breaking, tables added) + boots.

## D6 — humored wave patterns (this revision, §14)

- [x] pattern table in core (`wave_pattern(wave_index, level)`) replacing
      the plain cycle where gated; boss cadence stays.
- [x] core tests: monotone level gating, coverage roster names, cadence.
- [x] pattern name surface in raid banner.

## D7 — player threat (§15 stretch; needs user decision)

- [x] decided: resolved by **Spec 027 player combat** — `player_hp` +
      respawn/schema land there (raiders engage the player only after the
      player aggros them; roamers are 027's domain).
- [ ] server-side raid §8 hookup follows 027 Phase F3.

## D9 — REV5 natural enemy movement (§17–18)

Root cause (playtest "mobs kind of teleport"): `sync_td_entities`
despawns + respawns every marker per rebuild → each step re-spawns the
mob at its new tile center; `TdMove.speed` computes but glide never runs
(dist ≈ 0 at spawn).

- [ ] persistent mob entities keyed `mob_id` (update not respawn;
      despawn only when row leaves the snapshot).
- [ ] glide speed from the kind's `step_interval_secs` (2–5 s per hex);
      walk frames gated on actual distance; heading `flip_x`.
- [ ] death at last client position (red flash + 0.25 s fall/shrink).
- [ ] desync guard: jump > 2 hexes snaps, never glides a long haul.
- [ ] client tests: persistence across rebuild, glide speed per kind,
      walk-frame idle gate (extends 025 §6 client section).

## D8 — REV3 playtest + review

- [ ] playtest: every capital HP drop has a visible cause in ≤ 1 s.
- [ ] wall line demo: a barricade line funnels a solo test wave.
- [ ] `review.md` append (REV3 findings).
