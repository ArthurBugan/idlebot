# Spec 029 tasks — unified command panel

## Phase A — shell + tokens
- [x] A1 `ui/panel.rs`: `CommandPanel` + `Tab`, chrome constants, shell
  spawn (Startup), `on_key` pure fn + tests, tab-rail click map,
  registered in `main.rs`
- [x] A2 re-point `inventory.rs` + `net/forge.rs` colors at the shared
  tokens; delete duplicated constants

## Phase B — re-home the three tabs
- [x] B1 unified input: Tab/I/G/B (+touch bag) → `CommandPanel`; ESC
  closes; legacy `Inventory.open` becomes a derived view
- [x] B2 Inventory subtree → shell body (Bag tab)
- [x] B3 Forge subtree → shell body (Forge tab); forge `ForgeMenu` +
  own key handling removed
- [x] B4 Build subtree → shell body (Build tab); td rows re-skinned to
  the tokens; `TdBuildMenu` removed; dirty-flag overlay re-render kept

## Phase C — polish + verification
- [x] C1 hover accent borders, active underline, ✕ chip
- [x] C2 craft.rs duplicate `PANEL_BG` re-pointed (acceptance §6)
- [x] C3 tests: 84 green (`--test-threads=1`, incl. 7 new shell tests)
- [x] C4 live `cargo run` panics-free ~60 fps
- [ ] C5 (deferred, optional) tab icons from the atlas in the rail chips
