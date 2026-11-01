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

**REV2 — color-variant elite tiers.** Every slime and myconid recolor in the
art pack joins the roster as a stat-scaled elite (same sprite family, same
mechanics table, animated idle/walk from the shipped sheets):

| Kind | Lvl | hp | dps | reach | step | drop (chance, qty) |
|---|---|---|---|---|---|---|
| Pink Slime | 2 | 40 | 5 | 1 | 1 (sprinter) | Gel 45%, 1 |
| Pupple Slime | 3 | 70 | 6 | 1 | 2 | Gel 55%, 1 |
| Black Slime | 4 | 130 | 10 | 1 | 4 | Gel 70%, 2 |
| Myconid Blue | 4 | 90 | 6 | 2 | 4 | Gel 50%, 2 |
| Myconid Pink | 5 | 85 | 8 | 2 | 5 | Gel 55%, 2 |
| Myconid Purple | 5 | 100 | 8 | 3 | 5 | Gel 60%, 3 |
| Myconid Red | 5 | 150 | 12 | 3 | 6 | Gel 90%, 4 |

Roster = 15 kinds; wave composition cycles the full level-eligible tier set,
so higher capitals field deeper recolor waves.

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

---

# REV3 — Sieges you can see (playtest feedback)

## 8. Problem (playtest)

Playtest 2026-11: *"I don't see it attacking the capital, the life just
starts decreasing."* The server sim is honest, but the client shows none of
its causes:

1. **Ranged kinds are invisible shooters.** Archer/Myconid/Venom stop 2–3
   tiles out (`range_hexes`) and deal dps on the tick — no projectile, no
   muzzle puff, nothing. Damage appears on the capital out of nowhere.
2. **Melee kinds have no attack pose.** A slime parked on the capital tile
   renders its idle frame forever; the kill is silent.
3. **Structures and capital give no hit feedback** beyond the HP number:
   no flash, no shake, no toast. A wave that walked past the camera reads
   as "solar decay".
4. **Walls don't do anything anymore.** Walls are just higher-HP targets —
   march pathing ignores them, so a wall line neither blocks nor funnels,
   and the sapper mechanic (its whole reason to exist) rarely fires.

## 9. Goal

Every capital HP point lost must have a visible cause in the same second:
a mob standing/posing where it hurt you, a projectile, or a boom. Second
goal: make walls *matter* (funnel + chew), which is what makes enemy
composition fun — swarm slimes get funneled, sappers are the key, the bomb
goblin is the threat you must catch.

Keep every constraint: **server-authoritative, 1s ticks, integer math** —
all new visuals are client-derivative of existing table rows plus the new
event table (no client-side combat decisions).

## 10. Server: combat events (new table)

`combat_event` — thin, fire-and-forget rows a single tick can hold:

| field | type | note |
|---|---|---|
| event_id | u64 identity | monotonic |
| tick_at | u64 | server now, seconds |
| wave_id | u64 | ties fx to the wave banner |
| kind | string | `"shot" \| "melee" \| "bomb" \| "spore" \| "splash" \| "struct_fell" \| "capital_hit"` |
| attacker_mob | u64 | mob row |
| target_hex / target_slot | u64 / (i32,i32) | where the fx lands (slots for structures, center for the capital) |

Rules:

- Written inside `tick` **only for visible acts** (an attack or a kill).
  Marching/tower-fire use the same event kinds (`"shot"` covers tower fire
  too — the client distinguishes by attacker's presence in the mob table).
- `struct_fell` replaces the bare `tracing::info!` for the client rubble.
- Scrub: scheduled every 30 s deletes rows with `tick_at < now - 30`.
- Size cap: ≥ one event per acting attacker per tick; a 60-mob wave writes
  ≤ 60 rows/s — fine for the subscription.

## 11. Server: walls block (the funnel)

- `march` gains a passability filter: a step onto a hex with a
  **standing wall** (`is_wall` structure) is rejected like water.
- If every greedy step is blocked, the mob attacks the wall: the
  engagement target becomes that wall (sapper bonus now has real utility).
- One breach solves the line: once any wall in the path is destroyed the
  queue moves again (walls rebuild as rubble at half level — unchanged).
- Determinism: among equal-distance passable steps keep the existing
  tie-break order; among blocked steps pick the nearest wall by
  `(distance, structure_id)` — integer math, no randomness.

## 12. Server: encircle (melee crowd legibility)

- A melee mob in range of its target still *melees*, but if it is **on the
  target's tile** (only the capital/structure center tile is at distance 0)
  it has one marquee job: stop, pose, hit. Unchanged rule, verified.
- Crowd ring: when ≥ 3 melee mobs jostle the same target tile, each keeps
  the deterministic ring spot (sorted by mob id around the tie-break) so a
  swarm reads as *surrounding* the capital, not stacking into one pixel.

## 13. Client: fx per event kind

All fx are **client-only reactions to `combat_event` rows** (plus existing
HP-row diffs). 1 fx budget rule: every kind owned by its source file
`net/td.rs`; new atlas crops are allowed (32×32), no new systems.

| event | fx |
|---|---|
| `"shot"` | arrow/spore tracer: 0.3 s glide shooter → target hex (color by kind: arrows white, spores violet) |
| `"melee"` | attacker's 2-frame strike pose (or 0.12 s white-blink fallback until real frames ship) + small impact star at the target |
| `"bomb"` | 0.5 s lob arc then an orange AoE ring + smoke poof on the target ring |
| `"spore"` | green/violet puff decal, 0.4 s |
| `"splash"` | half-alpha echo of the bomb ring on each splash target |
| `"struct_fell"` | 0.5 s rubble crumble: structure sprite → alpha 0 + dust puff |
| `"capital_hit"` | capital sprite red flash 0.2 s + 3 px shake; HUD toast **"Capital under attack!"** on the first capital_hit of a wave; minimap ping already exists |

- **Attack pose wiring** replaces "standing on the top":
  `TdMarker` gains a 3-state animation cursor — `walk` (gliding), `idle`
  (no target), `attacking` (mob has a `"melee"`/`"shot"` event *this* tick).
  Ranged mobs hold the skeleton of a draw-pose while their tracer flies.
- **HP bar pulse** on `"capital_hit"`: capital HP bar dims gold for 1 s.

## 14. Wave design — hand-tuned patterns, not pure noise

`wave_composition` keeps cycling the level-eligible roster, but gains
**named threat patterns** per wave index (deterministic, integer):

| pattern | cadence | content | design purpose |
|---|---|---|---|
| **Scout Line** | fresh capital's 1st wave of a level tier | 4–6 slimes, one column | teach the target/tower loop |
| **Swarm** | every 3rd wave | slimes ×8 (dense 2-column) | farm fodder, focus-fire practice |
| **Sap Team** | every 4th wave ≥ L2 | spear goblins ×3–4 + 2 slurp shields | makes your wall line matter |
| **Volley** | every 5th wave ≥ L2 | archers ×3 + 2 spore tanks | anti-stationary pressure, tower spread |
| **Demolition** | every 7th wave ≥ L3 | bomb goblins ×2 + spear shield ×2 | punish passive walls; big AoE risk |
| **Boss** | unchanged §4 cadence | Big Slime leading a slime squad | siege breaker, first-wave danger block |

- Patterns escalate *within* a siege: a wave chain rolls
  `Scout → Swarm → Sap → Volley → …` cycling, capital-level-gated, so a
  level-3 capital fights a varied ladder every raid.
- The pattern name is player-visible ("Wave 3: Sap Team") in the raid
  banner — a preview: what you build against between waves.

## 15. Player threat (stretch — needs its own decision)

Mobs adjacent to the **player character** (not structures) deal their dps
to the player, with an HP meter, grace rules and a capital respawn —
genuinely makes towers + walls *for* you. Defer to the user decision
during review; it touches `Player` schema (`player_hp`, respawn rules)
which is the only place 025 becomes a schema-accrual task.

## 16. Tests

- Core: pattern table integrity (monotone wave gating, every roster kind
  appears in ≥ 1 pattern, boss cadence unchanged).
- Server: wall block changes step selection (`march_blocked_by_wall`);
  breach resumes march; encircle ring picks; combat_event written per
  hit; event scrub age rule. Integer-only assertions.
- Client: event-kind fx mapping covers every `kind` string the server
  writes (exhaustive test on the match); attack-pose cursor transitions;
  cap toast gating (first capital_hit of wave only).

