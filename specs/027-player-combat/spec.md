# Spec 027 — Player combat: weapons, attacks, dashes

> **Objective:** The player becomes a fighter. Early game you kill roaming
> hostiles **while you gather** — construction materials for towers must be
> collected with a weapon in hand, not idle-watched. Ships: an equipped
> **weapon** (campaign of craftable tiers), a server-authoritative
> **attack**, and a **dash** with a short invulnerability window. Resolves
> Spec 025 §15 (D7 "player threat") into design — the schema it needed
> (`player_hp`, respawn rules) lands here, not in 025.
>
> **Status:** built 2026-11-02 (see §13 deviations). Numbering: the
> pending farming/wildlife spec takes **028**.

## 1. Problem

- The game has no player-vs-enemy mechanic at all (Spec 023 §Non-goals
  wrote it off; REV4 changed the balance). Until the capital can fund
  towers, the *only* defense is walls, and the only enemy source is a
  full raid wave. Walking to a Wood deposit on a fresh account is riskless
  and dull; the tower escalation starts too late to feel earned.
- Kills during raids credit the capital (`CAPITAL_XP_PER_MOB = 10`), so
  the player is a bystander in every fight: mobs ignore the character,
  the character cannot shoot, and the best "active" play is pressing U
  next to a tower.
- Spec 025 §15 deferred the player-threat decision with the outline
  already drafted (HP meter, adjacency dps, capital respawn). This spec
  is that decision, expanded with offense and mobility rather than
  threat-only.

## 2. Design principles (constitution-bound)

- **Server-authoritative on the 1s tick + integer math.** Player combat
  runs through the same `tick` family as raids: `player_attack` is a
  reducer, the damage lives in the tick pipeline, `combat_event` rows
  carry fx. No per-frame combat simulation.
- **Client predicts, server validates.** Movement already works this way
  (`move_player` + `displacement_cap`); attack/dash follow the same shape.
- **Idle base-defense stays the answer to sieges.** Player combat power
  is capped: it fights *roaming* hostiles reliably and raiders at a
  penalty — the player is a skirmisher, not a wall replacement.

## 3. Player combat state (schema)

`Player` gains columns (all `#[default]`-seeded, bindings regen + publish):

| Column | Type | Meaning |
|---|---|---|
| `player_hp` | `u32` | current HP; max from `player_max_hp(level)` (§6) |
| `weapon` | `u8` | `0 = unarmed`, else craft tier id (§4) |
| `attack_cd_until` | `u64` | server-secs gates the `player_attack` reducer |
| `dash_cd_until` | `u64` | gates `player_dash` (per §5) |
| `dash_until` | `u64` | 0 = grounded; while set, dash speed + i-frames |
| `hostile_until` | `u64` | out-of-combat HP regen pauses until this tick |
| `death_until` | `u64` | > now ⇒ dead: client shows corpse timer, reducers reject |

`ClientPlayer` mirrors `hp`, cooldowns (same pattern the 5 s action
cooldown timer already established: `last_seen` + `until` timestamps,
HUD drains locally).

## 4. Weapons

Equipped from inventory; stance is an inventory-slot item that grants
offense. All tiers cost only existing items (SlimeGel falls out of
roaming-slime kills — the loop closes).

| Tier id | Weapon | cost | dmg/hit | cd (s) | range (hex) | notes |
|---|---|---|---|---|---|---|
| 0 | Fists | — | 4 | 1 | 1 | never unarmed-capable of dueling Bear packs alone |
| 1 | Wooden Club | 6 Wood | 7 | 1 | 1 | first craft after the starter depot |
| 2 | Bone Spear | 4 Bone + 2 Wood | 9 | 1 | **2** | reach — kite tool, less damage |
| 3 | Stone Sword | 5 Stone + 3 Bone | 14 | 1 | 1 | strong, close |
| 4 | Iron Blade | 3 MetalPart + 4 Bone | 20 | 1 | 1 | endgame skirmisher |

- dmg/hit is the per-attack burst (reducer-level), *not* a dps constant:
  server applies `dmg` to the chosen target, per 025-style damage model
  (`kill_or_wound`-equivalent for players), integer only.
- One weapon equipped at a time (`player.weapon`); swapping is free
  (reducer `equip_weapon(u8)`), switching consumes nothing. Weapon items
  are **not consumed** on craft — one purchase, permanent (matches the
  buildable model, not the trap model).
- Ranged weapons (slingshot/bow with Arrow item ammo) are **non-goal §8**.

## 5. Attacks and dash

### 5.1 `player_attack(direction: i32)`

- Reduced-cost reducer (no cooldown-tick reliance for input latency):
  rejects while `attack_cd_until > now`, `death_until > now`, or while the
  5 s action cooldown runs (same gauntlet build uses).
- Face the 8-way `direction` (mirrors movement headings), pick the
  nearest *hostile* mob within `range` hexes of the player in the
  120° arc facing that heading (integer heading test on axial coords).
- Apply `dmg`; write `combat_event { kind: "player_hit", attacker_mob: 0,
  target: mob }` → client renders a weapon-swing arc + impact bloom.
- **Aggro transition:** any mob the player damages becomes `aggro_toward
  player` for 20 s (roaming hostiles always aggroe on taking damage; see
  §7). Aggroed mobs march at the player instead of the capital, and their
  engagement resolves against the player's HP, not a structure's.
- Server publishes the keepalive side of cooldowns so the HUD drains
  locally (§3 pattern).

### 5.2 `player_dash(direction: i32)`

- Cooldown **6 s** (`dash_cd_until`); duration **0.4 s**; while active:
  - Movement speed ×3 through `displacement_cap` (server validates the
    burst exactly as it validates normal travel).
  - **i-frames vs. attacks only:** aggroed mob dps targeting the player
    is skipped while `dash_until > now` (terrain/tower fire unaffected).
- Dash is the answer to "surrounded while gathering": lunge *through*
  the pack, eat 0 burst damage, keep the loot.
- Client vfx: short after-image trail, dust puff on the hex, HUD dash
  icon drains (`Spec 025` cooldown-timer `CooldownFill/Text` components
  reused with a dash tag).

## 6. Player HP and death

- `player_max_hp(level) = 80 + 10·level` (idle level, reused — combat
  scales with the same progression as everything else).
- Out of combat (`hostile_until < now`): +2 HP per tick (full from 0 in
  ~2 min at L1), capped at max. In combat: no regen.
- **Grace (fresh-spawn rule):** 60 s protection on capital respawn — mobs
  cannot target the player; protection drops the moment the player
  attacks first.
- **Death:** `player_hp → 0` sets `death_until = now + 10`; client shows
  a death veil + countdown; respawn places the player at the **capital
  hex** with full HP. No inventory drop, no XP loss (early-accidental-
  death floor); this may be revisited when PvP exists.

## 7. Roaming hostiles (the early-game enemy)

Raids alone can't put an enemy on a Wood deposit. The world spawns
lightweight roaming packs on the existing 1s scheduler:

- **Slime packs:** 2–4 Slimes (the 22 HP fodder from 025 §3 — no new mob
  kind) spawn wander points sampled in grass biomes away from capitals
  (`tile_distance ≥ 3` from every capital, ≤ 20 hexes from the spawning
  node cluster so gathering stays local). Cap: max 6 live roaming packs
  globally per spawn tick; despawn after 300 s unpicked.
- Roaming mobs use the existing `mob` table with `capital_id = 0`,
  `wave_id = 0` guarded by a `roaming` derived check (`capital_id == 0`);
  their idle AI is a slow random walk (one step per `step_interval ×4`).
- Drops on kill: the standard Slime drop (SlimeGel 35%) PLUS Bone 25% —
  give the starter player a path to Bone Spear before waves exist.
- Player kills roamers: **drops go directly to the killer's inventory**
  (not the capital), +50 player XP each (idle-level table, 017).
- Roamers never siege, never attack structures; they attack the player
  only once aggroed (§5.1) or if the player walks into their hex.

## 8. Raids vs. the player (D7 resolution, penalty model)

- Raider waves keep tower/structure priorities: **aggression toward the
  player is opt-in-by-aggro** — a raider the player damaged (arrow of the
  melee swing) turns on the player for 20 s exactly like roamers.
- An aggroed raider's dps applies to `player_hp` per tick (adjacency ≤ 1
  for melee; ranged kinds keep their `range_hexes`).
- Player melee dmg vs. raid-kind HP is deliberately bad economy: Slime 22
  HP dies in 2 club hits, but BoneWalker-class HP takes ~4 Iron Blade
  hits — viable skirmishing, hopeless tanking. Towers still carry raids.
- `capital_hit` event queue, encircle rings, splash etc. are untouched —
  the player threat rides the same event stream with new kinds
  `"player_hit"` (player deals damage) and `"player_hurt"` (player takes
  it) so the client fx plan gets two exhaustive-test arms like 025 §16.

## 9. Client feel

- Attack: 8-way facing cursor already exists (025 §13 attack pose);
  attacks reuse it + weapon swing arc sprite frames (32×32 sheets, crop
  verified before coding — the REV4 verified-crop rule applies here).
- Dash: after-image trail + HUD (§5.2); dash key `Shift` (held direction
  = last heading) — spacebar reserved for jump cosmetics later.
- HUD: **weapon slot** on the hotbar far-right; hp bar above it
  (player wellness chip style), regen shimmer when out of combat.
- Death veil: full-screen dim + `Respawning in 7…` countdown + camera
  eases back over the capital hex on respawn.

## 10. Non-goals

- PvP damage (auth identity + PVE cap rules first; revisit after market).
- Ranged player weapons / ammo item chain (022 crafting extension).
- Stamina economy for dash (cooldown-only, one knob).
- New hostile mob kinds (roamers reuse 025 Slime art/stats).
- Mounts, defensive turrets controlled by the player.
- Balance pass for boss-class (Big Slime) threats to the player.

## 11. Tests (integer-only)

- Core: `player_max_hp` curve; weapon table bounds (cd ≥ action-cooldown
  floor? n/a — independent), heading-arc math for 8 directions × edge
  cases; dash displacement formula ×3 within cap.
- Server: `player_attack` rejections (cooldown/dead/grace-protected
  target? mobs can't be protected — reject only on cd + death + no arc
  hit); aggro transition mutates target; roaming pack spawn respects cap
  + distance window; death → `death_until` respawn at capital; i-frame
  skip while `dash_until > now`.
- Client: fx plan exhaustive for the two new event kinds (025 §16
  pattern); HUD dash cooldown mirror B01-free (one `Without` matrix note).

## 12. Rollout order

1. Schema cols + `player_attack` vs. roamers + kill/drop pipeline.
2. Dash + i-frames + HUD timer.
3. Roaming spawner + wander AI.
4. Weapon crafting tiers + equip reducer + hotbar slot.
5. Death/respawn + raid-side player threat (§8) — last, so world-heavy
   combat ships before touches the raid pipeline again.
Server publish per phase (schema columns accrue by 025 pattern: additive
columns with `#[default]`).

## 13. Deviations (as built, 2026-11-02)

- **Extra Player columns:** `grace_until` (the 60 s respawn protection
  needed its own timestamp; spec folded it conceptually into spawn) and
  `weapons_owned: u16` bitmask (bit *i* = tier *i* permanently owned —
  weapon items are permanent, but the server needs a slot to remember
  the purchase; first equip consumes materials, switching free).
  Spec's 7 columns + these 2 = 9 total.
- **Mob table +4 columns:** `aggro_target: String` (default `""`),
  `aggro_until_secs`, `pack_id`, `born_at_secs` — roamers and the 20 s
  aggro window need durable state; raiders reuse `aggro_target`.
- **Dash key is Space, not Shift** (§9): the client's `player_movement`
  owns the speed burst and spacebar was free; jump cosmetics postponed.
- **Server dash = 1 tick:** server `DASH_TICKS = 1` (0.4 s rounds up at
  the 1 s tick resolution); the client runs the real 0.4 s local burst
  (`dash_burst_secs`) and the server validates ×3 travel in
  `move_player` while `dash_until > now`.
- **Contact aggro distance:** "walks into their hex" reads as
  `tile_distance ≤ 1` (adjacent hexes included), not the exact hex.
- **Roamer wander cadence:** one step per `step_interval × 4`; pack
  jitter 2–4 via `pack_tag(q, r)`; lifetime 300 s regardless of aggro;
  spawner attempts every 60 s, ≤ 6 live packs, needs an online living
  player, spawn hex = materialized Grass ≥ 3 from every capital and
  ≤ 20 from an online player, hash-picked by `now`.
- **Kill credit split (§7):** roamer kills pay drops (SlimeGel 35% +
  Bone 25%) + `ROAMER_XP = 50` directly to the credited player, no
  capital XP; raiders keep drop + `CAPITAL_XP_PER_MOB = 10`. Tower /
  burn-trap roamer kills credit the tower/trap owner via a
  `damage_credit` map. `kill_or_wound` moved to `combat.rs` with a
  `credit_fallback: Option<&str>` for the credited player.
- **`player_hp == 0` without a death stamp = unseeded:** reads FULL on
  both server (`hp_of`) and client (`update_combat_hud`), so legacy rows
  (pre-027 players) don't render as dead.
- **Aggroed raiders skip structure/capital engagement** and the ring
  pass, march + strike the player hex directly; frost aura still doubles
  their cadence; walled-in chase = no dps.

## 14. Addendum — Bow + weapon forge panel (2026-11-02, same day)

Lifted the §10 "ranged / ammo" non-goal by request (approved in-chat):
ammo item model, 3-hex reach medium bow, data-driven forge UI.

- **Tier 5 "Bow"** — cost 5 Wood + 3 Bone + 2 Grass (string), dmg 12,
  cd **2 s** (the only 2 s cadence; §11's "1s cadence" pin now asserts
  `≥ 1` + per-tier sheet pins), range **3** hexes, consumes **one Arrow
  per real strike** (empty arc costs cd only, never an arrow — you
  didn't release). `weapons_owned` bit 5; `u16` already holds it.
- **Arrow item** — new `player_item` "Arrow"; one costs 2 Wood +
  1 Stone. `craft_arrows(count)` reducer crafts 1..=32 at once from the
  stacks (all-or-nothing with exact refund on race-fail); bench-free,
  same purchase model as `equip_weapon`. Reentries via KeyV `weapon+1`
  ladder now caps at 5 and KeyC cycles include the bow.
- **Dry-stack shots reject:** `player_attack` errors
  `"No Arrow — craft more in the forge"` before the arc pick.
- **Weapon Forge panel (KeyG):** data-driven off the core `WEAPONS`
  const — per-tier stat line, one-time cost line, owned/equipped/
  affordable row tints, Buy/Equip buttons (server re-validates), and
  an Arrow batch row (×10 per press, capped by affordability display).
  Same open/close-audit trail as the craft-bench menu; E/B/F/C/V keys
  untouched. Status line shows the equipped tier's Arrow stack count.
- **HUD:** the weapon chip appends `· N Arrow` when a bow is equipped —
  firing dry becomes visible before you try.
- Client Arrow icon reuses the ArrowTower arrow art (no new sheet, so
  the REV4 crop rule isn't triggered; swing sprites stay deferred).
