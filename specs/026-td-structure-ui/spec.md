# Spec 026 — Structure interaction: click-select, attack area, upgrade panel

> **Objective:** Clicking a placed buildable (tower / wall / trap / chest)
> selects it: the world shows its **attack area**, an inline panel shows its
> stats, and buttons drive **upgrade (XP transfer)** and **remove (refund)**.
> Extends Spec 024's proximity-press-U upgrade into a real selection UI.
> Also adds placement-side parity: build mode shows a rotatable ghost
> preview on the world before the building is committed.
>
> **Status:** authored 2026-11-02, not started.
> Numbering note: 026 was tentatively reserved for the farming/wildlife
> plan; that spec is still pending and will take **028** (027 is now
> player combat).

## 1. Problem

Everything a raid defense needs is already server-side, but invisible:

- Towers have `range_hexes` (2 / 3 / 2), Frost tower a `slow_radius` (2) —
  the world never draws them. Positioning towers is blind guesswork; you
  learn a tower's reach only by watching where it fires.
- Upgrading exists (`transfer_building_xp`, +30% HP / +25% dps per level,
  cap L5) but the only UI is a **proximity keypress**: stand next to *a*
  structure and press **U** — you cannot pick *which* one, you cannot see
  the current level, XP bucket, or what the transfer buys.
- Traps and the chest have no inspection at all: their HP (fire-trap fuel)
  and slot placement are only visible as the sprite.
- Costs are paid on placement; `remove_defense` refunds half of every
  ingredient, but nothing in the UI offers removal — you have to remember
  the reducer exists.

## 2. Existing building blocks (do not rebuild)

| Piece | Where |
|---|---|
| `DefenseKind` config: cost / hp / dps / range / trap fields | `defense_config.rs` (shared core) |
| `structure_max_hp(kind, level)` (+30%/L), `structure_dps` (+25%/L) | `defense_config.rs` |
| `STRUCTURE_LEVEL_MAX = 5`, `structure_xp_for_next_level(level)` = `100·L²` | `defense_config.rs` |
| `defense_structure` rows: `owner`, `level`, `total_xp`, `hp`, `x/y` world pos | cached per-row on the client (`TdView`) |
| `transfer_building_xp(structure_id)` reducer + ownership/XP server checks | `buildings.rs` |
| `remove_defense(structure_id)` reducer + 50% refund | `defense.rs` |
| Client structure sprites at real world coords, per-kind heights | `td.rs` `structure_sprite` |
| Client already depends on `idlecore-core` → deltas computable locally | — |

The spec is therefore **client-heavy**: the selection/panel/overlay work
needs no server change, and the only schema change is the placement
`rot` column (§3.5).

## 3. Design

### 3.1 Click-to-select

- LMB (not a UI click, not in build mode) picks the structure whose sprite
  rect contains the click point: screen → world `camera.viewport_to_world`,
  then nearest `defense_structure` row within its render extents
  (sprite width from crop aspect × height, height from `structure_sprite`;
  traps count their ground hex — click anywhere on the trap's hex).
- Terrain clicks deselect; ESC deselects; opening the build menu (B)
  deselects. Selecting never blocks capital-claim or build hint flows.
- **Ownership matters for buttons, not for the ring:** you may click any
  structure to *see* it, but the panel shows Upgrade/Remove only for your
  own rows (server would reject either way — the client mirrors that).

### 3.2 Attack area overlay (selected structure only)

> **Implementation deviation (recorded):** the movement lattice is SQUARE
> and the server's `tile_distance` is Chebyshev (`max(dq, dr)`) — so the
> true in-range shape is a `(2·range+1)·TILE` **square**, not a disc. The
> overlays draw that exact square (same language as the territory fills),
> matching precisely what the server counts; the frost slow radius draws as
> a second outline square.

Drawn while selected, removed on deselect:

- **Tower range disc:** filled translucent disc at the tower's world
  position, radius `range_hexes · HEX_SIZE·√3` wu (the server's
  `tile_distance` metric), alpha ≈ 0.18 tint per kind color:
  Arrow azure, Ballista bone-white, Lantern warm amber, Coconut palm-green,
  Frost ice-blue.
- **Frost slow ring** (radius 2, no attack-range confusion): a dashed or
  lighter second disc so "slowed inside, shot within 2" reads in one look.
  If a structure has both ranges they draw as concentric rings with
  distinct alphas (Frost: range 2 + slow 2 → one ring + label).
- **Trap hex mark:** spike/web/fire traps get a pulsing 1-hex ground
  quad (the consumable's footprint), amber for fire, dim for web —
  the footprint *is* the area; no disc.
- **Walls / chest:** no range anything — selection ring only.
- The same disc preview appears in **build-mode placement hints** (phase 2
  task): while a tower/trap kind is armed, the floor preview shows the
  potential attack area at the hovered hex. This is the single biggest
  gameplay-quality win of the spec; do not ship §3.2 without it.

### 3.3 Inline selection panel

Small anchored panel (same style as the build menu, ~240px):

```
Arrow Tower  L2        ● HP 104/104
dps 12 · range 2       XP 0/400
[ Upgrade → L3  (dps 15) ]        ← primary
[ Remove  (refund: 4W, 2S, ½MP) ] ← destructive, twice-click confirm
```

- Header: `kind_display` + level pip (hidden at L1), HP bar with the
  existing green→amber→red tint.
- Stat line: `dps · range` from `structure_dps/structure_max_hp` at the
  current level; traps show `(trap: 40 dmg | pin 5s | burn 6dps×10)`
  instead of dps/range; walls show hp only.
- Next-level row: `XP 0/400` — spendable surplus math must mirror the
  server's `transfer_into` bucket (`structure_xp_for_next_level` and the
  player's mid-level XP surplus — reuse whatever the HUD's capital-level
  widget already reads; if that mirror does not exist yet, the panel does
  bucket math locally from `player.xp` and flags the gap).
- **Upgrade button** → `transfer_building_xp(structure_id)`. Disabled with
  a short reason when: not owner · L5 ("MAX") · insufficient surplus XP ·
  on action cooldown (5 s). On success the panel refreshes from the next
  `TdView` sync and floats a `+L2→L3` pip on the structure.
- **Remove button** → `remove_defense(structure_id)`. Requires a second
  click ("sure?") like the market confirm; disabled for non-owners. Shows
  the refund preview (half of every cost ingredient, from `DefenseKind.cost`).
- Chest-selected panel gets the same iterate as existing inventory: a
  "Contents" line (count of stored items) if the client row cache already
  carries `chest_item` — a deposit/withdraw UI is \*out of scope here\*
  (Spec 024 §2.3 owns it).
- Panel layout must survive the B0001 SendSyncVariadics rule: two sibling
  `Node` writers → use a `ParamSet` like `update_td_hud` does.

### 3.4 Hover glance (cheap tier)

Before the full panel, hovering (cursor inside the sprite rect, no click)
shows a one-line nameplate above the structure sprite: `Arrow Tower L2 ·
HP 104/104`. Same picking code, `Changed`-driven, low churn throttle (0.25 s).

### 3.5 Placement rotation + world preview (armed kind)

While a kind is armed in build mode, before the placing click:

- **Ghost preview:** the armed kind's sprite renders at the hovered floor
  hex as a translucent (alpha ≈ 0.6) ghost, tinted green when the hex is
  buildable and red when invalid (reuse the existing build-mode hint's
  valid/invalid signal — same source of truth, do not add a second one).
  The §3.2 attack-area disc follows the ghost for tower/trap kinds. The
  ghost is the *actual* placed sprite (same crop/height as
  `structure_sprite`), rotated per current orientation — not a new
  placeholder art.
- **Rotation:** **R** rotates the armed kind 90° clockwise (Q for
  counter-clockwise, if cheap — R alone satisfies the requirement).
  Four orientations (0/90/180/270), stored as `rot: 0..3` in the armed
  state; re-arming a kind resets to 0. Rotation is per-arm, never per-
  ghost-per-hex.
- **Placement carries the orientation:** the placing call sends `rot` with
  the position; the placed structure's client sprite renders at that
  rotation, and the selection/hover views of §3.1/§3.4 render the same
  rotation.
- **Which kinds rotate:** kinds whose sprite is direction-dependent —
  walls/fences (a 32×18 fence facing east vs. north is the whole point)
  and towers. Single-hex ground traps (spike/web/fire) and the chest have
  no visible orientation: rotation is accepted but ignored for them
  (keeps the input model uniform; `rot` is stored as-is server-side).
- **Server implication (this spec is no longer client-only):**
  `defense_structure` gains a `rot u8` column (bit-packed or new column —
  implementer's call, keep it 2 bits of information) and the placement
  reducer takes `rot` with a range check (reject >3). This is the spec's
  **only** schema change → bindings regen + a single publish; placement
  rows written by the old client default `rot = 0`.

### 3.6 Out of scope for placement

- No free-angle or 8-way rotation (90° steps cover the pixel-art crops;
  arbitrary angles are not representable in the existing sprite system).
- No two-stage (arm → click-confirm) placement: hover-preview + click
  places immediately, same as today — the preview is informational, the
  click is the commit.

## 4. Non-goals

- No repair button: post-wave repair is automatic (Spec 024); damage state
  heals between raids by design.
- No chest deposit/withdraw UI from the panel (Spec 024 §2.3 flow).
- No multi-select, no structure move/reposition (teardown-rebuild covers it).
- No per-structure in-game re-rotation after placement (teardown-rebuild
  with R covers orientation changes).
- No new server reducer unless the client cannot compute the XP bucket
  locally — then the wrapper returns `{bucket, level}` with ownership
  checks, nothing more.
- No mobile/touch interaction model.

## 5. Acceptance criteria

- [ ] Click a tower → its attack disc draws; deselect (ESC / terrain click)
      removes it. No overlay outside build mode.
- [ ] Frost tower shows slow ring distinct from range; traps show footprint
      quad; walls/chest show selection ring only.
- [ ] Selection panel shows level, hp, dps/range (or trap stats), XP bucket;
      numbers match `structure_max_hp/structure_dps/…` math for all 12 kinds.
- [ ] Upgrade button on own tower at L1→L2 with 100 surplus XP succeeds;
      server log shows `TRANSFER-BUILDING-XP`; panel + world level pip
      refresh inside one `TdView` sync. Disabled states: non-owner, L5,
      broke (red reason line), cooldown.
- [ ] Remove flow: refund preview line matches 50% of `DefenseKind.cost`
      per ingredient; double-click confirm required; row disappears after
      sync; rubble rule (no rubble for traps) still holds.
- [ ] Build-mode placement preview shows the armed kind's attack area on
      hovered hexes before placement.
- [ ] Ghost preview follows the hovered hex (green/red buildability), shows
      the actual rotated sprite + area disc; R cycles 90° steps and the
      placed structure keeps that orientation (all views: world, hover
      nameplate, selection). Re-arming resets to rot 0.
- [ ] `defense_structure.rot` round-trips: place at rot 2 → row carries 2 →
      sprite renders at 2; invalid rot (≥4) rejected by the reducer.
- [ ] No B0001 panic (sibling Node writers vs. `Without` matrix), tests for
      picking rect math + bucket math + refund preview strings.

## 6. Rollout order

1. Picking + selection ring + hover nameplate (§3.1, §3.4).
2. Selection panel read-only (stats, hp bar, level) for all 12 kinds (§3.3).
3. Range/slow/footprint overlay + refresh pips (§3.2).
4. Upgrade + remove buttons wired to the existing reducers (§3.3).
5. Build-mode ghost preview + rotation (§3.5) and area preview (§3.2).
Publish is now required: the `rot` column is a schema change (bindings
regen); everything else is client-only.
