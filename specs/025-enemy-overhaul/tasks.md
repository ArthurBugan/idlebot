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
