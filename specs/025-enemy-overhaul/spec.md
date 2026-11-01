# Spec 025 — Enemy overhaul

## 1. Problem

Raid mobs today (Spec 023) are three stat-sticks — Scavenger / BoneWalker /
RustHusk — marching one hex-step rule, melee-only, rendered with the wrong
art (a frog, a crow, an ostrich) tinted purple/white/orange. The art pack
(`assets/models/Enemy/`) ships real, animated 32×32-frame sheets for slimes
(7 colors × small/big/sprout), goblins (archer / spear / bomb / merchant),
Myconid (5 colors) and a rooted Venom Bloom — none of it used. Combat reads
as: mob walks until adjacent, hits, dies. No variety, no ranged pressure,
no fear of splash.

## 2. Goals

1. Field **real enemy art** with **animated walk** for every raider kind
   (idle/walk from the shipped frame sheets; damage/dead polish deferred).
2. Give enemies **distinct mechanics on the 1s server tick**: melee, ranged,
   splash bomb, wall-priority sapper, rooted turret.
3. Keep everything **server-authoritative on 1s ticks with integer math**
   (constitution rule) — no per-frame combat.
4. Replace the Scavenger/BoneWalker/RustHusk roster entirely; legacy mob
   rows despawn naturally via the existing "unknown kind → delete" guard.

## 3. Roster (server `MOB_KINDS`)

All kinds attack once per tick when in their `range` of a structure or the
capital; melee kinds require adjacency (distance ≤ 1), ranged kinds reach up
to their column. March = the existing greedy one-hex step.

| Kind | Lvl | hp | dps | range | step | specials | art |
|---|---|---|---|---|---|---|---|
| Slime | 1 | 22 | 3 | 1 | 2 | swarm fodder | `Slimes/Green/Slime` |
| Blue Slime | 1 | 30 | 4 | 1 | 2 | swarm fodder | `Slimes/Blue/Slime` |
| Spear Goblin | 2 | 55 | 7 | 1 | 3 | +5 dmg vs walls (sapper) | `Goblins/Spear Goblin` |
| Archer Goblin | 2 | 40 | 6 | 3 | 2 | ranged volley | `Goblins/Archer Goblin` |
| Myconid | 3 | 70 | 5 | 2 | 4 | ranged spores, slow tank | `Myconid/Green` |
| Bomb Goblin | 3 | 45 | 10 | 2 | 2 | splash: half dps to every *other* structure adjacent to the mob | `Goblins/Bomb Goblin` |
| Venom Bloom | 4 | 90 | 8 | 3 | 0 | rooted — never marches; spawns inside the territory | `Venom Bloom/Purple` |
| Big Slime (boss) | 4 | 260 | 14 | 1 | 5 | siege breaker, rare | `Slimes/Golden/Big Slime` |

Drops (chance %, qty → defender):

- Slime ×2: `Slime Gel` 35%, 1 · Spear: `Bone` 60%, 1 · Archer: `Arrow x3`? —
  keep inventory items that exist: `Bone` / `Metal Part` / `Slime Gel` (new
  item, craft-hooked later) · Myconid: `Mushroom`?? — no: `Slime Gel` stays
  the only new item; Myconid drops `Bone`? No — Myconid drops nothing new:
  kept drops: Bone, Metal Part, Slime Gel only.

Final drop map (items that already exist in the inventory economy):

| Kind | drop | chance% | qty |
|---|---|---|---|
| Slime | Slime Gel (NEW) | 35 | 1 |
| Blue Slime | Slime Gel (NEW) | 55 | 1 |
| Spear Goblin | Bone | 60 | 1 |
| Archer Goblin | Bone | 40 | 2 |
| Myconid | Slime Gel | 45 | 2 |
| Bomb Goblin | Metal Part | 50 | 1 |
| Venom Bloom | Slime Gel | 80 | 3 |
| Big Slime | Slime Gel | 100 | 5 |

`Slime Gel` enters `add_item` like Bone/Metal Part (no recipe yet — a
farming/alchemy spec consumes it later).

## 4. Mechanics (server `mobs.rs`)

- `MobKind` gains `range_hexes: u32` and `splash: bool`.
- **Engagement**: nearest target within mob `range_hexes` wins (distance
  ranking, ties → lowest id — same rule as towers today). Structures beat
  the capital at equal reach... unchanged from Spec 023: any structure in
  reach is preferred over the capital.
- **Ranged**: archer/myconid/venom bloom deal their dps without closing;
  the client keeps the glide so they visibly stand off and shoot.
- **Splash (Bomb Goblin)**: on its attack tick, every *other* structure
  within 1 hex of the bomb goblin takes `dps / 2` (min 1) in addition to
  the main target.
- **Sapper (Spear Goblin)**: +5 dps when the target structure kind is a
  wall (Bone Barricade); plain dps vs towers/capital.
- **Rooted (Venom Bloom)**: `step 0` — spawns at distance ≤ 2 inside the
  territory (frontier conversion: ring outside-in from the capital), never
  moves.
- **Boss spawn**: Big Slime enters every 4th wave (deterministic counter
  per wave_id parity on spawn order) beyond capital L4.
- Wave composition: deterministic shuffle over the level-eligible kinds;
  composition weights stay simple (cycle + inject the boss rule).

## 5. Client (net/td.rs)

- One idle + one walk crop per kind from the sheets (32×32 frames:
  idle col 0 row 0; walk alternates col 0/col 1 of the walk sheet every
  0.25 s while the marker glides).
- Kind → sprite map mimics `mob_texture` but with real art and neutral
  tints (no more frog/crow/ostrich placeholders).
- Damage flash: when a mob's HP row drops, tint red for ~0.15 s.
- HP bars unchanged.

## 6. Tests

- Core: roster integrity (unique names, drops exist, ranges ≥1 except
  rooted bloom), composition eligibility per capital level, boss cadence.
- Server: ranged engagement beats out-of-range targets; splash math;
  sapper wall bonus; rooted mob never steps; legacy kinds self-delete.
- Client: kind→crop map covers every roster name; walk-flip cadence.

## 7. Non-goals / deferred

- Damage/dead animation frames + on-death poof (after first playtest).
- Goblin Merchant as an NPC trader (needs a trade UI — later spec).
- New tower categories to counter ranged mobs (Spec 026 candidate).
- Wildlife/farm life — next spec (026), as decided.
