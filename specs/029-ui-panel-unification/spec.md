# Spec 029 — Unified UI: one tabbed "Command Panel" for Inventory · Forge · Build

> **Objective:** The three centred overlay panels — **Inventory**
> (Tab / I), **Weapon Forge** (G) and **Buildings** (B) — become three
> tabs of **one shared panel shell**. One frame, one visual language,
> exclusive open state; each tab keeps its content logic untouched.
> Motivation: the three overlapping/mismatched panels described in §1.
>
> **Status:** built 2026-11-02 (see §7 implementation notes). Numbering
> note (per 026/027): **028** stays reserved for the pending
> farming/wildlife spec; this takes **029**.

## 1. Problem

Three independently-spawned overlay panels exist today, each with its own
root entity, its own toggle key and its own styling constants:

| Panel | Key(s) | Root | Anchor | Aesthetic today |
|---|---|---|---|---|
| Inventory (`inventory.rs`) | Tab / I | `inventory-panel` | top-center, `margin-top: 90` | `PANEL_BG` `(0.07,0.07,0.10,0.96)`, no border, plain text header |
| Weapon Forge (`net/forge.rs`) | G | `forge-wrapper` | top-center, `margin-top: 90` | same bg + gold `BUY_BORDER`, green/red tinted state rows |
| Buildings (`net/td.rs`) | B | `td_build_menu_root` | centered, `bottom: 126` (above hotbar) | teal border `(0.28,0.62,0.5)`, blue row bg `(0.1,0.18,0.3)`, emoji header |

Consequences:

1. **Overlap bug:** Inventory and Forge anchor the identical screen slot
   (`top 90`, centered). Open both and they stack — forge rows draw
   through the inventory grid.
2. **Three visual languages:** the ID panel is flat, the forge is the
   gold-bordered idler language, the build menu is the teal/top-left
   cluster language. None of them matches the touch/bottom-sheet chrome
   (`BackgroundColor(srgba(0.16,0.18,0.26,0.85))`, 1 px gray border,
   bright cyan text) or the 026 structure-select panel.
3. **Discoverability cost:** three keys to learn (plus Tab), three
   close paths (Tab, G, B, ESC semantics differ per panel), no
   indication that more tabs exist while one is open.
4. Text-only headers inventory/build vs emoji (`🛠 BUILDINGS`).

## 2. Existing building blocks (do not rebuild)

| Piece | Where |
|---|---|
| Slot grid + drag/brush/craft-grid + chest sub-tab | `inventory.rs` (phases `spawn_hotbar`, `spawn_inventory` …) |
| Forge panel + pure row-state helpers + 7 tests | `net/forge.rs` (`row_state`, `row_btn_text`, `arrow_affordable`) |
| Build rows + `TdBuildRow(kind)` + placement flow | `net/td.rs` `TdBuildMenuRoot` subtree |
| Card chrome used by touch bottom sheet | `touch.rs` lines ~171–183 |
| Fonts: `FontSource` handles passed into every subtree | current spawn fns |

**Strategy:** keep every content tree; replace only the shells. Each
panel's spawn function is re-anchored from "spawn my own root" to
"spawn my content under the shared frame's active-tab body". Toggle
state collapses to one resource.

## 3. Design

### 3.1 The shell (`ui/panel.rs`, new module)

One root, top-center, identical anchor for every tab:

```
┌──────────────────────────────────────────┐ ▲
│  [Bag]  [Forge]  [Build]        ✕        │ │ header rail
├──────────────────────────────────────────┤ ─
│                                          │ │ body
│   (active tab's existing subtree)        │ │ (flex column,
│                                          │ │  row_gap 4)
│                                          │
│  status/caption line (per tab)           │ ▼
└──────────────────────────────────────────┘
```

- Root node: `PositionType::Absolute`, `justify_content: Center`,
  `align_items: Start`, `margin-top: 90` (same slot inventory/forge used
  — moves the build menu up to join it; the hotbar keeps its own layer
  and stays visible beneath).
- Width: owned by the **body** per tab (inventory keeps
  `9·(44+4)+20`, forge `340`, build `330`) — the frame wraps content
  (`Self::auto`) instead of fixing one width, so each tab stays its
  current size.
- Max height `Val::Vh(70.0)` with `overflow: clip` on the body; if a
  tab grows taller (deep chest stacks) it clips — no scroll UI this
  spec.

### 3.2 Shared chrome tokens (`ui/panel.rs` constants)

One source of truth; every other module migrates to these:

```rust
pub const FRAME_BG:     Color = Color::srgba(0.07, 0.07, 0.10, 0.95);
pub const FRAME_BORDER: Color = Color::srgb(0.35, 0.35, 0.42);   // 1 px
pub const RAIL_BG:      Color = Color::srgba(0.10, 0.10, 0.15, 0.90);
pub const TAB_TEXT:     Color = Color::srgb(0.62, 0.70, 0.80);
pub const TAB_TEXT_HOT: Color = Color::srgb(1.0, 0.95, 0.6);    // active
pub const HEAD_TEXT:    Color = Color::srgb(0.85, 0.90, 1.0);
pub const BODY_TEXT:    Color = Color::srgb(0.75, 0.85, 0.80);
pub const ACCENT:       Color = Color::srgb(1.0, 0.80, 0.4);   // gold (buy/hover)
pub const GOOD:         Color = Color::srgb(0.4, 0.95, 0.6);
pub const BAD:          Color = Color::srgb(1.0, 0.4, 0.35);
pub const ROW_BG:       Color = Color::srgba(0.10, 0.10, 0.14, 0.88);
pub const ROW_RADIUS:   f32 = 6.0;
pub const FRAME_RADIUS: f32 = 9.0;
pub const TEXT_SHADOW:  TextShadow = /* BLACK, offset(1,1) — all text */;
```

`inventory.rs`/`net/forge.rs` re-point their consts to these (delete the
dupes); build rows re-skin from teal/blue → `ROW_BG` + `FRAME_BORDER`,
row-interact hover → `ACCENT` border. The gold border already built for
the forge buys stays (it *is* `ACCENT`).

### 3.3 Tabs + exclusive open state

- New resource `CommandPanel { open: bool, tab: Tab }`,
  `enum Tab { Inventory, Forge, Build }`.
- Key mapping (all three existing keys stay, mapped into the panel):
  - **Tab / I** → open (+`tab = Inventory`) or close if already
    there; if open on another tab, switch to Inventory.
  - **G** → Forge, same rules.
  - **B** → Build, same rules. **B keeps a second use:** while the panel
    is open on Build it re-presses nothing — placement still ends with
    E / right-click as today; ESC closes the panel and cancels the
    armed build mode (unchanged 023 rule).
  - **ESC** → closes the panel (whichever tab) — replaces per-panel
    ESC handling where safe; never collides with 026 selection
    deselect (panel close wins, deselect follows next frame's existing
    rules when the intent is a world click).
- Toggle helpers as **pure, tested fns**:
  `fn on_key(panel: &mut CommandPanel, tab_for_key: Tab) -> bool`
  (returns "consumed"), so the three input systems share one code path.

### 3.4 Tab rail content

- Three equal tab chips (Button + Interaction), 64×22 px each,
  `ROW_RADIUS` corners, label + small key superscript, e.g.
  **Bag ^Tab**, **Forge ^G**, **Build ^B** — icons come from the
  existing atlas (`ICON_PICKAXE`/`ICON_BOW`/build-row icons optional;
  text-only ships first, icons are Phase B polish).
- Active tab: bg `RailActive` tint `(0.14,0.14,0.20,0.95)`, text
  `TAB_TEXT_HOT`, 2 px bottom accent underline in `ACCENT`.
- `✕` close chip at rail right (20 px), same click handler as ESC.
- Below-rail caption line per tab (current hint strings move here):
  Inventory: `[Tab] close · click two cells to move a stack`
  Forge: (existing last equip/craft status line)
  Build: `pick one → press E on a lit hex; ESC cancels`

### 3.5 What stays (per-tab behaviours unchanged)

- Inventory: slot grid, drag/brush, craft grid, chest sub-panel (chest
  stays a section inside the Inventory tab, not a new tab).
- Forge: data-driven rows off `WEAPONS`, state tints (`ROW_OWNED`,
  `ROW_EQUIPPED`, `ROW_UNAFFORDABLE`), arrow batch caps, Arrow HUD chip.
- Build: grouped `BUILD_GROUPS` rows, cost labels, ghost preview and
  world-click placement. Only its frame restyles.
- While the panel is open the world input already ignores movement
  clicks for the inventory path; extend the same "UI open ⇒ block
  aimed interactions" check to all three tabs by testing
  `CommandPanel::open` instead of `Inventory::open` (entity-aim +
  attack + dash keep their own keys — they don't read this panel).

### 3.6 Exclusions

- The 026 structure-select panel stays world-anchored (it follows a
  *selected structure*, not a tab) — but it adopts the chrome tokens so
  nothing else drifts.
- Tesla `HUD` chip cluster (top-left), retail hotbar, touch
  bottom sheet: out of scope; they only inherit the tokens in a later
  polish pass if trivial.

## 4. Server impact

None. Pure client spec; no reducer, schema or bindings change.

## 5. Tasks

### Phase A — shell + tokens
1. `crates/idlecore-client/src/ui/panel.rs`: `CommandPanel`, `Tab`,
   chrome constants, frame spawn (Startup), `on_key` pure fn + tests,
   tab-rail click map (Interaction), registered in `main.rs`.
2. Re-point `inventory.rs` + `net/forge.rs` color consts at the shared
   tokens; delete duplicated constants; keep module-private behaviour
   tests green.

### Phase B — re-home the three tabs
3. `sys_toggle_inputs`: one system reading Tab/I/G/B → `CommandPanel`
   (and forwarding B's second use to `TdBuildMenu`). Delete
   `inv.open`/`TdBuildMenu.open` as independent flags — they become
   derived views of `CommandPanel` (field or accessor) so existing
   checks keep compiling.
4. Inventory subtree → frame body (tab Inventory); same anchor slot;
   its own caption line.
5. Forge subtree → frame body (tab Forge); `forge-wrapper` root
   dropped, panel node only.
6. Build subtree → frame body (tab Build); re-skin rows to tokens.

### Phase C — polish + verification
7. Tab icons from atlas (optional branch); hover accent borders; ESC in
   chest-drag state tests.
8. `cargo test -p idlecore-client -- --test-threads=1` (target ≥ 77
   green + new pure-fn tests); live `cargo run` playtest: all three
   keys from every open tab, ESC path, overlap regression (open
   Inventory then G — forge must switch tabs, never stack).

## 6. Acceptance

- No two overlay panels can overlap; every key opens its panel in ≤ 1
  frame-switch.
- One palette file; `grep` finds no dup color literals for
  `0.07,0.07,0.10` outside `ui/panel.rs`.
- All pre-existing tab behaviours demonstrably unchanged (drag stack,
  forge buy spam, build placement).
- Tests green (`--test-threads=1`).

## 7. Implementation notes (built 2026-11-02)

- New module `crates/idlecore-client/src/ui.rs` → `ui/panel.rs`: the
  `CommandPanel { open, tab }` resource, the pure `on_key` toggle (7 unit
  tests), the shell spawn (`command-panel-wrapper/frame/rail/body`), the
  content-adoption system, the display sync, chip chrome + clicks. All
  chained in one `PanelPlugin`, registered in `main.rs`.
- All three tabs re-homed: `InventoryPanel`/`ForgePanel` root + the td
  menu wrapper are gone; each spawns a `TabContent(Tab::…)` root adopted
  by the shell body. Visibility lives in `sync_panel_tabs` alone.
- Key wiring: Tab/I + the touch bag button → Bag, G → Forge, B → Build
  (`command_panel_keys` consumes `TouchControls::inventory`); ESC closes
  the shell. `Inventory.open` survives as a legacy view fed by
  `sync_inventory_from_panel` (touch visibility + world-click guards).
- Build flow: ESC stays the disarm key (023 rule); closing the Build tab
  via B does **not** disarm an armed kind (the row-click flow closes the
  shell right after arming, and B simply re-reveals the lit hexes) — a
  deliberate change from old B-close-disarm.
- Token adoption: `ui::panel` owns the palette; inventory, forge and the
  td build rows (plus `net/craft.rs`'s duplicate `PANEL_BG`) re-point at
  it. Forge row-state tints stay forge-specific. Rows use the green
  `GOOD` / gold `ACCENT` / grey `FRAME_BORDER` language; tab chips hover
  gold and underline the active tab.
- 84 client tests green (`--test-threads=1`; 77 before + 7 new shell
  tests); live run panics-free, ~967 entities.
- Deferred (was Phase C optional): tab icons from the atlas chips — text
  chips shipped.
