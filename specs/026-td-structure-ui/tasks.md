# Spec 026 — Tasks

## Phase A — picking + selection
- [x] A1 picking: screen→world click → sprite-rect hit test over the
      `TdStructRows` mirror (dedicated 0.2 s watcher — `sync_td_entities`
      was already at Bevy's 16-SystemParam cap, so the mirror could not ride
      it); trap = whole-hex Chebyshev hit; unit tests for rect/footprint math.
- [x] A2 deselect rules: ESC, terrain click, armed build mode / B menu;
      UI guard = any widget `Hovered|Pressed` (same trick as the minimap
      teleport clicks).
- [x] A3 hover nameplate (§3.4) with 0.25 s throttle, `[L{n}]` pip included.

## Phase B — panel (read-only)
- [x] B1 panel root + markers (`TdSelect` + `TdSel*` components; ParamSet
      capped at 8 sibling writers — the refund line is its own system
      `update_select_refund`).
- [x] B2 stat lines for all 12 kinds (tower dps/range, trap burst/pin/burn
      + "fuel = HP" note on fire traps, wall/chest hp), hp bar fill mirror;
      `upgrade_delta_line` unit-tested.

## Phase C — overlays
- [x] C1 attack area per tower kind tint — **DEVIATION (documented in spec
      §3.2): the grid is a square lattice with Chebyshev `tile_distance`,
      so the true server-counted range shape IS a `(2r+1)·TILE` square, not
      a disc; same overlay language as the territory fills.**
- [x] C2 frost slow ring (outline sprite frame), trap footprint quad with
      armed pulse (consumables), walls/chest: selection ring only — teal for
      your rows, ice for others'.
- [x] C3 soak: builds + live-run clean, no B0001 (all panel `Text` writers
      under one ParamSet; fx system spawns sprites only).
- [x] E1 build-mode preview: armed kind's ring + attack area (or trap
      footprint) on the hovered hex before placement (cursor world→tile;
      player hex fallback for touch).

## Phase D — actions
- [x] D1 upgrade button → existing `transfer_building_xp(structure_id)`;
      gated with owner/MAX/cooldown — reason surfaces in the button label;
      XP-bucket math mirrored (`level_progress` surplus + `100·L²` need).
- [x] D2 remove button with second-click confirm ("Sure?") + refund
      preview = `qty/2` integer division exactly matching
      `remove_defense`; `remove_refund_preview` unit-tested against three
      cost tables (incl. the hides-zero-ingredient case).
- [x] D3 refresh: next TD sync rewrites hp/level; the confirm gate decays.

## Phase E — publish
- Client-only (no schema change, no bindings regen needed). Server side of
  the XP transfer pre-existed (Spec 024 `transfer_into`).
