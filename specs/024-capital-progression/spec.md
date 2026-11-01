# Spec 024: Capital Progression — Levels, Buildable XP, Grown Territory

> **Objective:** Make the Capital a living, leveling base: capitals grow in
> level (and buildable soil), and **defense structures level up too** by the
> owner transferring their own XP into them. Buildables are ONLY placeable
> inside the owner's capital area.
>
> **Status:** authored in-session rev7 — the L1 radius bump (10×) and the
> client HUD/menu polish land immediately; Levels/XP-transfer is the phased
> build ahead of us (see plan.md).

## 1. Problem Statement

Spec 023 shipped a static capital: level 1 forever, no XP sink, territory
locked at radius 8 (217 hexes — playtesting found it cramped), and towers
never improve after placement. Trading exists (011), idle XP exists (001),
but progression is flat: gathering → build → one wave per hour with nothing
to spend XP on. This spec makes XP a *currency* that can be invested into
your realm, both at the capital and per structure.

## 2. Design

### 2.1 Capital levels → grown soil

- **Radius formula (live, rev9):** `claim_radius(level) = min(83 + 2·(level-1), 91)` —
  rev8 raised the initial claimable AREA ×10 (radius 26/2107 hexes became
  83/20 917 hexes); rev9 reshaped the claimable region into a **world-axis
  SQUARE** with half-side = radius · HEX_SIZE·√3 wu (L1: 1437.6 wu) instead
  of a hex disc, restoring the disc's lost corners (area ×4/π ≈ 1.27).
  L1 disc = **20 917 hexes** (rev8: 9.92× the rev7 2107; the original
  radius-8 disc was 217), L5+ cap = 25 117.
- **Radius growth grants reach, not land:** territory still spreads
  hex-by-hex (adjacent walk + resource cost per hex). Raising the radius
  raises the *boundary* of what you may claim around the capital.
- **Placement spacing:** `MIN_CAPITAL_SPACING = 200` (rev8: two full capped
  discs + walking gap; was 60 at the radius-26 era)
- **Capital levels up on XP** (`100·level²` curve; already consumed from
  raid kills via `CAPITAL_XP_PER_MOB` = 10):
  - L2+ waves add +15% wave size per level (already server-side).
  - +25 max HP per level (already server-side).
  - New: on level-up, player gets a cosmetic toast + the minimap capital
    diamond gets a level pip.
- **Structure placement is restricted to the owner's territory** — server
  already gates this (`Build on your territory — claim the land first`); this
  spec additionally tests hex-inside-radius so an allied owner can never
  build in someone else's claim.

### 2.2 Structure levels via XP transfer (new mechanics) — **implemented D1**

- `defense_structure` gains `level: u8` (default 1, `#[default(1)]`) +
  `total_xp: u64` (the current-level ring bucket, `#[default(0)]`).
- **Costs to level a structure:** consumes the *player's own* XP
  (`player.total_xp`, Spec 017). Reducer
  `transfer_building_xp(structure_id)` (live, `buildings.rs`):
  owner-only, capped at `STRUCTURE_LEVEL_MAX = 5`, transfer thresholds are
  `100 · L²` of the structure's own level (L1→L2 100, L2→L3 400, L3→L4 900,
  L4→L5 1600). One call consumes the whole offered surplus → the bucket
  fills and each crossing level bumps `level` + clamps `hp` to the new max.
- **Level perks** (in `defense_config` scaling helpers, used server-side on
  the 1s tick + client HUD):
  - `structure_max_hp(kind, level)` — HP grows +30% per level over base
    (barricade L1 60 → L2 78 → L3 101, …).
  - `structure_dps(kind, level)` — towers gain +25% DPS per level
    (arrow tower L1 10 → L2 12.5 → round down).
  - Range stays fixed (positioning skill — level buys toughness, not range).
  - Non-wall structures can be damaged → need repair → their level descends
    on destruction (a fully destroyed tower respawns at half its level,
    rounded down; level never below 1).
- **Transfer UX:** clicking one of your structures on the world (or the
  building menu's "Level up" row while hovering/standing on its hex) opens a
  small inline panel: `Arrow Tower L2 → `**`[Transfer 400 XP]`**` → L3` with
  hp/dps deltas shown. Confirming sends the reducer; server re-checks
  ownership + XP + caps (L5 cap on structure levels, structure XP curve
  `100·L²`, i.e. L1→L2 100, L2→L3 400, …).
- Anti-frustration: transferring XP never drains the player below level
  chunk boundaries (levels stay intact — XP is only spendable from the
  current mid-level surplus, mirrored from `level_progress`).

### 2.3 Storage chests (capital vaults)

Chest-like structure kind (`ITEM_STORAGE_CHEST`, cost 8 Wood + 2 Metal Part):
- Placeable only on owned soil, takes a plot slot.
- **Server-side death/repair resilience:** a destroyed chest returns its
  contents to the capital (or drops on the ground at the hex — TBD task
  review) so raid losses don't vaporize your run inventory.
- Reducers: `chest_deposit(item, qty, chest)` / `chest_withdraw(item, qty,
  chest)` — 64-slot capacity per chest, visible in the inventory UI as an
  extra tab while standing on your territory (client-only panel, data is
  server rows `chest_item`).

### 2.4 Inventory & building menu improvements (HUD polish, live rev7)

- Inventory: hover tooltip (`name ×count`) following the cursor on every
  panel slot and hotbar slot; no more click-to-identify.
- Building menu (B): cost strings turn **red** when the inventory can't pay;
  unaffordable rows dim; armed row keeps the teal highlight; icons come from
  the atlas. Structure-level flow plugs into this same menu (§2.2).

## 3. Non-goals

- No direct player combat (towers only — carried from 023).
- No instanced raid dungeons; raids stay in the shared world.
- No account wipe; XP transfer reuses the same `player.xp` column.

## 4. Acceptance Criteria

- [ ] `defense_structure.level` exists; building XP never exceeds the
      player's spendable surplus.
- [ ] Structure HP/DPS scale per level (server tick + HUD show level from L2).
- [ ] Build placement is rejected for hexes outside the owner's territory,
      server-enforced with a unit test.
- [ ] L1 claim radius ≥ 2000 hexes; growth curve capped at 34.
- [ ] `chest_deposit/withdraw` round-trips; destroyed chest refunds its
      content per the task-reviewed flow.
- [ ] HUD: capital chip shows level; building rows show per-level cost and
      disabled state.

## 5. Rollout order

Radius/spacing/constants + build-menu polish → ship now; chest and XP
transfer need schema columns (`level`, `total_xp`, `chest_item` rows) →
publish together with Phase D1 in plan.md.
