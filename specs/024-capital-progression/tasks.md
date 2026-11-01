# Tasks 024: Capital Progression

Working checklist. Status mirrors the working tree — nothing committed until
the owner says so.

## D0 — constants & schema (partial: shipped rev7)

- [x] claim_radius base 26 (+2/level, cap 34) — area math tests updated
- [x] MIN_CAPITAL_SPACING 24 → 60 + spacing test rewritten
- [x] STRUCTURE_LEVEL_MAX + scaling helpers (to do — Phase D)
- [ ] `defense_structure.level` / `total_xp` columns
- [ ] `chest_item` table (chest_id, slot_index, item, count)

## D1 — server

- [ ] transfer_building_xp reducer + tests
- [ ] chest deposit/withdraw/destroy-refund reducers + tests
- [ ] build placement inside-territory radius test

## D2 — client

- [x] inventory hover tooltip (`SlotTooltip`, cursor-following)
- [x] building menu affordability (red cost, dim rows) via `TdBuildCost`
- [ ] structure level pip over HP bar
- [ ] transfer panel (owner building click → level-up send)
- [ ] chest inventory tab

## D3 — rebuild + publish (radius live)

- [x] `spacetime build` + publish new wasm (radius/spacing constants live on
      `db.nestfeed.app` — existing hex-by-hex claim flow unchanged)
- [ ] full-re publish when D1 columns land (schema add-safe)

## Progress notes

- rev7 (this session): claim radius L1 8→26 (~10×, 2107 hexes vs 217),
  growth +2/level, cap 34 (3487), spacing 60; tests updated in
  `defense_config.rs` + `capital.rs`. Client: `TdHexTex` procedural hexagon
  overlays replaced the pinhead squares (build teal / capital gold / claim
  azure / ownership gold-violet rims), B-key blind cycle replaced by the
  🛠 BUILDINGS menu (click-to-arm, affordable costs red-dim, armed teal,
  icons from the atlas), inventory hover tooltips. Debug panel (F1/Z) fully
  rebuilt earlier as the top-left cluster pattern — no B0001, no commit
  until owner go-ahead.
