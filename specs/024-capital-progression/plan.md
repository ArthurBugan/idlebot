# Plan 024: Capital Progression

Mirrors the 023 plan format. Keep every phase D-numbers stable; tests first
inside each phase.

## Phase D0 — constants & schema columns (core + server types)

- `defense_config.rs`: `STRUCTURE_LEVEL_MAX = 5`, `structure_max_hp`,
  `structure_dps` scaling helpers (+tests). Claim radius set live: base 26,
  +2/level, cap 34; spacing 60.
- `types.rs` (+ core mirror if any): `DefenseStructure.level: u8`,
  `DefenseStructure.total_xp: u64`; new `chest_item` table
  `(chest_id, slot_index, item, count)`.

## Phase D1 — server reducers + tick integration

- `transfer_building_xp(structure_id)`: owner check, cap check, XP surplus
  spend (only points above the current `level_progress` boundary), bump
  `level`, recompute `hp = min(hp, max_hp)`.
- `build_defense` unchanged placement gate (owner territory) + add
  radius-close check test.
- Chests: `place_chest`, `chest_deposit`, `chest_withdraw`,
  `destroyed-chest refund` inside the raid destroy path.
- Raids: structure level feeds `mob.rs` damage-towards logic (destruction of
  leveled buildings grants their Keeper nothing — kill only).
- Scheduler: no change to wave cadence (023 owns it).

## Phase D2 — offline & persistence

- XP transfer works offline? No — the reducer runs when called; the player
  can send it from the client anytime client is connected. Chests persist
  (rows). Raid destruction refunds chest content server-side (offline safe —
  handled by the sim tick, not the player session).

## Phase D3 — client UI (net.rs plugin side + inventory.rs)

- Structure markers get a small `L{n}` pip above the HP bar from L2.
- Building menu: rows show "L2 → L3 [Transfer 400 XP]" affordance when
  standing on/near an owned structure hex of that kind.
- Toast on level-up (capital + structure).
- Chest tab in the inventory panel (visible while inside own territory):
  lists `chest_item` rows of the nearest chest; deposit = drag like the
  existing slot interactions; withdraw = click chest item.

## Phase D4 — tests

- Core: level curves (max_hp, dps, caps) — 4 tests.
- Server: `transfer_building_xp` (owner mismatch, insufficient surplus, cap),
  chest deposit/withdraw/refund, placement-inside-territory radius test —
  6 tests.
- Client: HUD `format level pip`, menu afford afford calculation — 2 stat
  tests.

## Phase D5 — playtest & review

- T5.x: user playtests the radius bump + XP transfer loop.
- T5.y: `review.md` after a shared pass (same gate as 023 T5.4).
