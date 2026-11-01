# Tasks 024: Capital Progression

Working checklist. Status mirrors the working tree — nothing committed until
the owner says so.

## D0 — constants & schema (DONE, live)

- [x] claim_radius base 26 (+2/level, cap 34) — area math tests updated
- [x] MIN_CAPITAL_SPACING 24 → 60 + spacing test rewritten
- [x] STRUCTURE_LEVEL_MAX (5) + `structure_max_hp` (+30%/L) + `structure_dps`
      (+25%/L) + `structure_transfer_xp` helpers + tests
- [x] `defense_structure.level` / `total_xp` columns (`#[default(1)]`/`#[default(0)]`)
- [x] `chest_item` table (key = "{structure_id}|{item}") + generated client
      bindings (chest_deposit/chest_withdraw/transfer_building_xp reducers)
- [x] DB wiped (owner-approved) and re-published — clean schema live

## D1 — server (DONE)

- [x] `transfer_building_xp` reducer: owner-only, spends the player's
      current-ring surplus, caps at L5, hp clamps to the new max
- [x] chest `deposit`/`withdraw` reducers + 64-stack capacity guard
- [x] raid hooks: leveled tower fire (+25%dps/L), cleared-wave repair to the
      leveled max, structure death respawns at half level (chest contents
      refunded to the owner first), capital destruction refunds chests
- [x] build placement inside-territory test (existing gates assert radius 26)

## D2 — client (DONE except titles)

- [x] inventory hover tooltip (`SlotTooltip`, cursor-following)
- [x] building menu affordability (red cost, dim rows) via `TdBuildCost`
- [x] structure level pip (`L2` 2d text above the HP bar, from L2)
- [x] **U** invest-XP flow: nearest own structure within 1 hex →
      `transfer_building_xp` (+ `choose_upgrade_target` unit test: nearest
      own neighbor, tie → lowest id, out-of-range/enemy/empty → None)
- [x] chest tab in the inventory panel: "Deposit all (active item)" button +
      one `TAKE →` row per stored stack, rebuilt every 0.25 s while standing
      on an own StorageChest (`update_chest_near` picks the hex)

## D3 — rebuild + publish (radius live)

- [x] `spacetime build` + publish new wasm (radius/spacing constants live on
      `db.nestfeed.app` — existing hex-by-hex claim flow unchanged)
- [x] full publish with schema (DB wiped by go-ahead)

## Progress notes

- D0/D1 session (committed): core leveling helpers tested (4 new core tests:
  hp/dps scaling, 100·L² curve, transfer math incl. multi-ring + cap; server
  pure tests for half_level + deposit guard). Two real test bugs found &
  fixed while building: cap disc area 3487→3571 (bad arithmetic), and
  `half_level` ignored STRUCTURE_LEVEL_MAX (255 → 127). Server review also
  brought the claim-radius/spacing tests to the 26/60 world. Schema:
  `defense_structure.level` (u8, default 1) + `total_xp` (u64, default 0) +
  `chest_item` table; the DB was WIPED by owner go-ahead to apply the new
  schema (fresh world). Raid integration: leveled dps, leveled repair,
  half-level rubble respawn, chest refunds on tower/capital death.
- PLAYTEST tomorrow: place a capital (fresh Core comes from `new_player`),
  claim ring, build, force-wave, press **U** next to a tower to transfer XP,
  open Tab next to a chest to deposit/withdraw stacks.

## D4/D5 — tests & review

- [x] core: 376 total (4 new scaling/curve/transfer, 4 radius/spacing)
- [x] server: 76 total (+2 buildings pure tests)
- [x] client: 18 net tests (+2: level pip, upgrade-target pick)
- [ ] T5.x shared playtest → `review.md` (D5, gated)
- rev7 (this session): claim radius L1 8→26 (~10×, 2107 hexes vs 217),
  growth +2/level, cap 34 (3487), spacing 60; tests updated in
  `defense_config.rs` + `capital.rs`. Client: `TdHexTex` procedural hexagon
  overlays replaced the pinhead squares (build teal / capital gold / claim
  azure / ownership gold-violet rims), B-key blind cycle replaced by the
  🛠 BUILDINGS menu (click-to-arm, affordable costs red-dim, armed teal,
  icons from the atlas), inventory hover tooltips. Debug panel (F1/Z) fully
  rebuilt earlier as the top-left cluster pattern — no B0001, no commit
  until owner go-ahead.
