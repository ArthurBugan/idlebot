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

- [ ] **server `combat_event` table** (§10): kinds, writer inside `tick`,
      scheduled scrub > 30 s; audit entry keeps a fired-count.
- [ ] **walls block the march** (§11): passability filter in `march`,
      nearest-wall engagement fallback, breach resumes; tests.
- [ ] **melee encircle ring** (§12): deterministic ring spot per mob id
      when ≥ 3 melee jostle one target; test.
- [ ] **client fx per event kind** (§13): tracer/puff/bomb/splash/fell/
      capital-flash/shake/toast + capital HP bar gold-pulse on hits.
- [ ] **`attacking` animation state** on `TdMarker`: pose sheet cols per
      kind (white-blink fallback rule); blocked when no event this tick.
- [ ] **wave opener banner** with pattern name (§14) on wave open.
- [ ] suites: server tests for event rows + block/funnel + scrub; client
      exhaustive `kind→fx` match test; build + publish + boots.

## D6 — humored wave patterns (this revision, §14)

- [ ] pattern table in core (`wave_pattern(wave_index, level)`) replacing
      the plain cycle where gated; boss cadence stays.
- [ ] core tests: monotone level gating, coverage roster names, cadence.
- [ ] pattern name surface in raid banner.

## D7 — player threat (§15 stretch; needs user decision)

- [ ] decision needed: `player_hp` on `Player` + adjacency dps from mobs,
      capital respawn + gold loss — **schema accrual; ask the user**.

## D8 — REV3 playtest + review

- [ ] playtest: every capital HP drop has a visible cause in ≤ 1 s.
- [ ] wall line demo: a barricade line funnels a solo test wave.
- [ ] `review.md` append (REV3 findings).
