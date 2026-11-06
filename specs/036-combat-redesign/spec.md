# Spec 036 — Complete combat redesign: THE BEAT

> **Objective:** rebuild combat around one idea. Every enemy strike is
> *announced one beat ahead*, and the player's skill is acting on the
> **off-beat**: strike into gaps in enemy rhythm, dodge fired strikes, and
> park where the next beat can't reach you. Not a balance pass — the enemy
> damage model, the defensive verb, and enemy behavior grow from that law.
>
> **Status:** spec written 2026-11-05, awaiting approval. Plan + tasks.md follow
> approval (constitution: no code before an approved spec). Numbering: next
> free spec number after 035 is **036**.

---

## Stage 1 — Understand the game (what stays, what dies)

Architecture facts that bound the design (verified in tree):

- **Server sim beats at 1 Hz** (`scheduled_mob_tick` → `mobs::tick`);
  timestamps are whole unix seconds. `player_attack` resolves **immediately
  in the reducer** (not tick-gated); mob→player damage is applied on tick.
- **Movement is client-predicted**, server clamps at `move_player` (0.15 s
  sync). The player experiences 60 fps; the world experiences 1 fps.
- **Presentation is interpolation** (mob glide, remote-player glide); fx ride
  `combat_event` rows (30 s TTL).
- Roster: **15 mob kinds, all with shipped animated sheets**; the economy
  (Bone/SlimeGel/MetalPart drops → 71 weapons, 40 armors) is drop-fueled.
- Playtest history that stays law: hold-to-swing REJECTED, dash REMOVED
  (Space inert), buffered single swing KEPT, cone/knockback fx KEPT.

### What works (keep)

| Keep | Why |
|---|---|
| Server-authoritative 1 Hz sim + instant-reducer player input | the frame the whole game is built on; sub-second server combat would be a rewrite, not a redesign |
| Client-predicted movement with server clamp | combat positioning already feels real-time on the player side |
| `combat_event` fx stream, Zelda numbers, hitstop/shake/hurt blink | feedback layer is now strong — the weakness is upstream (what happens to you is unreadable, not unrendered) |
| Weapon/armor drop economy, reach/cd/ammo weapon matrix | horizontal progression skeleton already exists; it just doesn't matter in fights |
| Lane-locked raid march, wave patterns, tower defense | untouched; player combat rides alongside, never replaces towers |

### What dies and why (the real reasons combat disappoints)

| # | Finding | Why it kills the experience |
|---|---------|------------------------------|
| D1 | **Enemy damage is a continuous dps aura.** `roamer_strike`/`raider_strike` apply `kind.dps` every tick a mob stands in reach. No windup, no rhythm, no recovery. | You cannot dodge an aura. With no *when*, there is no *should I move* — damage is ambient tax, combat has no decision to make. Clarity violation #1. |
| D2 | **No defensive verb.** Dash was removed; dodging = walking away, and the aura in D1 applies damage at 1 Hz regardless of run speed — outrunning it is impossible. | Encounters degenerate to "stand, take ambient damage, leave". With no consumable heal, retreat is the only defensive answer. |
| D3 | **Attacks are target selectors, not strikes.** One nearest mob in a 120° arc eats the full damage; the swing art hits nobody else. | Sword swings (area reads) and Bow shots (laser reads) resolve identically as single-target: positioning never pays. |
| D4 | **Enemy "identity" is stat-table identity.** 15 kinds differ by hp/dps/range/step + splash/sapper columns, but all walk-in-and-hit behaviorally. Readability fails: two enemies look different, play the same. | Learning an enemy counts for nothing; knowledge = damage math only. |
| D5 | **Boss = 260 hp stat stick.** Big Slime leads every 4th wave with melee-contact dps. | Zero encounter design; the "boss" is the same enemy with more of the same. |
| D6 | **Knockback is random cosmetic.** Player shoved one king-step away from the striker; unrelated to any choice. | Positioning punishes/rewards nothing; knockback can shove the player into worse trouble with no teaching moment. |

The live system accumulates band-aids over D1–D2 (buffered swings, hurt
blink, HP bars). Those stay — they are good presentation of a now-dying
core. The core is what this spec replaces.

## Stage 2 — Combat philosophy: THE BEAT

**The 1 Hz tick is not a limitation to hide — it is the design.**

> *Every enemy strike is announced exactly one beat before it lands. The
> player's verbs resolve instantly, between beats.*

One sentence of player experience: **"I read the windup, I chose — strike,
dodge, or step — and the world told me honestly whether I was right."**

Principles kept from classics, mapped onto this server:

| Classic principle (ALTTP / Minish Cap / SotN) | On a 1 Hz authoritative server |
|---|---|
| Enemies telegraph before committing (Moblin windup, Octorok pause) | enemy windup = the tick *before* the strike: a `mob_wind` event + pose |
| Dodge/i-frames are the defense (Link's roll, bat form) | a fast **dodge-step** verb with a 1 s server-validated i-frame window |
| Sweeping sword hits a fan of enemies | cone multi-hit (primary + side targets at 50 %) |
| Heavy weapons shove, light weapons don't | weapon-*family* knockback, not random |
| Reach is a real property (SotN whip, spear) | unchanged matrix, now honest: what the swing art covers is what it hits |
| Bosses are patterns, not hp pools | Big Slime: 3-beat slam rhythm, summons, dodgeable splash |

Deliberately rejected (directive + playtest-consistent):

- **Block/shield**: a hold-input state machine on a click-only, touch-friendly
  control scheme; dodge-step already answers "how do I not eat it". No.
- **Combos/cancel chains**: fighting-game degrees of freedom the directive
  bans and the click cadence can't carry. No.
- **Magic/spell economy**: nothing in the drop economy says "mana". The bow's
  Arrow stack is already the ammo-resource lesson. No second resource.
- **Parry/riposte timing at 60 fps**: enemy verbs live on 1 s beats; a frame-
  perfect parry window would contradict the source of truth. (Revisitable if
  a fast tick ever lands — noted, not specced.)

## Stage 3 — The system

### 3.1 Player verbs (complete list — there are no others)

**V1 — Strike** (kept, refined). Click / KeyF / arrow-ammo rules exactly as
today. Changes:

- **Cone multi-hit**: the swing hits the primary target (nearest, today's
  pick) for full `weapon.dmg`, plus every *other* active mob in the same
  120° arc within range for `max(1, dmg / 2)`, capped at 3 total victims.
  Empty arc: **no change** — still sets cd and costs nothing (whiff cost =
  the cd; that is the commitment).
  Edge: bow stays single-target (projectile identity; ammo is its cost).
- **Shove by weapon family** (replaces D6 randomness): survivors of the
  strike are shoved one king-step along the swing heading **only when the
  weapon family shoves** — Axe families (Axe, Pickaxe, Shovel) shove; Sword
  families (Sword, Sickle, Hoe, Bow, Fists) don't. Light = flow, heavy =
  displacement. Shove rules stay as today (refused for rooted, water, walls).
- Buffered attack (0.9 s expire) stays exactly as shipped.

**V2 — Dodge-step** (restored, reworked — the game needed a *reason* first;
D1's fix is that reason). Input: **Space** (desktop) / on-screen button
(mobile), reads the movement heading or facing if idle.

- Effect: movement burst ×3 speed for 0.6 s *local* (client-instance, as the
  old dash), **i-frames for 1 s server-side**: strikes targeting the player
  while `dash_until > now` resolve as DODGED.
- Cooldown **2 s** (old dash was 3–6 s). Cheap on purpose: dodges are now
  telegraphed by enemy windup, so the cd doesn't ration them — the enemy
  rhythm does. The decision is per-beat.
- Feedback: dust ghost streak (the old dash ghosts, once, forward), gray
  floating **"DODGED!"** number at the missed strike, tiny hop of the sprite.
- Edge: i-frames checked at the damage instant; starting a dodge 1 ms before
  the strike tick still dodges (server clock decides, honest either way).
- Space inert since removal — this restores input ownership of Space.

**V3 — Walk / reposition** unchanged. Sprint stays local-only. Positioning
is the third verb implicitly: standing where windups can't reach you.

No V4. Block, traps buffing, charge attacks are out (Stage 2).

### 3.2 The strike contract (replaces D1's dps aura) — **core server change**

Enemy → *player* damage becomes **discrete, telegraphed strikes**:

1. Every mob gets `next_strike_at: u64` (unix secs). Scheduling: on spawn and
   after every strike, `next_strike_at = now + kind.strike_period_secs`
   (fresh per-mob stagger `+ mob_id % period`).
2. At `next_strike_at - 1 >= now` (the windup beat) the server emits
   **`combat_event kind "mob_wind"`** (attacker_mob, target hex = player's).
   Client poses the mob (present pose/white pulse rises over the beat).
3. At `next_strike_at` the server resolves the strike **only if the player is
   still in reach** (`tile_distance ≤ kind.range_hexes`): if the player walked
   out — nothing lands, mob repositions or waits. If the player dodges —
   `player_dodge` event, no damage. Otherwise damage =
   `strike_dmg = (kind.dps × 2).max(4)` **minus armor, floored at 1**,
   one king-step knockback (today's rule), event `player_hurt`.
4. **Between strikes a mob in reach deals zero.** The aura is gone.

Numbers note: `dps × 2` per `strike_period` keeps kinds' effective dps at
~half the old standing-tax (measure in P1; exact curve in tasks.md with the
per-kind table in §3.4). That's intent: standing in a windup should *hurt*
(one strike stings more than one tick of aura did), but leaving after the
windup is now a *real* answer.

**Anti-exploit walls kept:** dead/offline players aren't wind targets
(scheduler skips, reschedules on next tick), mobs under grace/protected
players don't schedule, `spawn_at`-staggered mobs schedule on activation,
walled-in chasers never schedule (pinned without reach), server restart
resets `next_strike_at = 0` → one beat of grace then re-staggered (no
instant snap-strike).

**Strikes vs structures/capital stay the existing per-tick dps** — towers
can't dodge, siege is the towers' boss fight, not a dodge dance. The
contract is player-facing only. Bomb splash/Venom zone dps, web pins, frost
aura, burn traps: untouched.

### 3.3 Clarity rules (directive §3, feedback requirements)

| Feedback | Trigger | Form |
|---|---|---|
| Windup pose | `mob_wind` | mob freeze-frame + white pulse swell over the beat; bomber raises arm (existing idle frame, tint shift) |
| Strike lands | `player_hurt` | red number, shake 0.3, hitstop 0.05, red blink, knockback (all already shipped) |
| Dodge succeeds | `player_dodge` (new event) | gray "DODGED!" float at the missed strike hex, ghost streak, no shake |
| Player walked out | nothing | absence is the reward; the whiff itself is visible in the strike animation's absence. Optional `mob_whiff` fx arm decided in tasks |
| Pop numbers | (kept) | boosted letters already live |

Sound: **deferred** — the project has no audio pipeline yet; this spec hook
is text-only. Visuals carry clarity alone until an audio spec lands.

### 3.4 Enemy archetypes (from the shipped 15 kinds — zero new art)

15 stat-kinds regroup into **5 behavioral archetypes** + a boss. `period` =
`strike_period_secs`; all wind 1 beat before landing.

| Archetype | Kinds (art exists) | Rhythm | Behavior beyond strike |
|---|---|---|---|
| **Biter** — swarm rushers | Slime, Blue/Pink/Pupple/Black Slime | p=4 | on strike, lunge 1 hex *toward* the player along the swing heading |
| **Skirmisher** — standoff shooters | Archer Goblin, Myconid ×4 | p=5 | after each strike, takes 1 king-step *away* from the player (range-keeper) |
| **Sapper** — greedy melee | Spear Goblin | p=4 | targets walls as today (walls keep the aura rule — siege side); vs player follows the strike contract |
| **Bomber** — area denier | Bomb Goblin | p=7 | splash reads the *player's hex* at strike: hitting the player also damages adjacent structures (Bomb-Goblin splash reuse, as today) — leaving = denying it |
| **Bloom** — rooted zone threat | Venom Bloom ×1 (step 0) | p=6 | stationary; its wind paints a range-2 zone warning: stand in the zone = eat it |
| **BOSS** — Big Slime (3-beat slam + summon) | Big Slime | p=3 | §3.5 |

Every archetype needs only: parsed `kind` → archetype map in server code
and existing fx ops. No new sheets, no new events beyond `mob_wind` /
`player_dodge` (+optional whiff), no schema beyond `next_strike_at`.

### 3.5 Boss: Big Slime — pattern over pool (D5 fix)

Same stats, **new personal rules** (deterministic, 1 Hz honest):

1. **Slam every 3rd beat**: wind beat big (client scales it 1.35× during
   wind), strike = `dps × 3` damage in a **range-1 splash**: hits the
   player's hex + every adjacent structure (Bomb-Goblin splash reuse,
   literal). Dodge it; stand behind terrain you can lose (a wall eats the
   structure damage, not you).
2. **Summon every 6th beat**: spawns 1–2 Slimes adjacent (existing spawn
   code, capped at 2 live adds, wave-owned). Kill them or the pack
   out-gnaws you.
3. **Enrage at hp ≤ 30 %**: period 3 → 2, slam dmg ×3 → ×4. Ends when it
   dies; boss hp stays reachable (260 → verify tasks math vs. weapons: ~10
   Iron Blade hits = challenging but not a sponge).
4. Reads still fail safe: if the player is offline/dead, the boss reverts to
   plain siege behavior (structures keep the continuous rule).

Kill rewards unchanged (drops + capital xp; roamer-transport only for
roamers — boss is wave-side, so capital credit; tasks pin it).

### 3.6 Encounter design (how archetypes compound)

Raid composition already cycles named patterns; the archetype layer makes
those patterns play differently without touching composition code:

- **Scout** waves (biters) teach rhythm-counting: cheap, forgiving window.
- **Sap Team** (sappers) teaches wall-priority + a bodyguard decision.
- **Volley** (skirmishers) teach must-advance: closing 1 hex under 5-beat
  rhythm while taking positions behind lane props.
- **Demolition** (bombers) teach zone denial: step, bait, step.
- **Swarm** mixes biters + ranged: 3-beat congestion — the dodge-step's
  crowd proof, and where cone multi-hit earns its keep.
- Boss waves teach the whole kit: the enrage lands only after the first
  summon beat, so the player meets each rule one at a time.

No new wave tables. Same code path; the *behavior schema* is what rewrites
the room.

### 3.7 Progression (horizontal only; no new items — directive §6)

The matrix stays; combat makes its existing columns *matter*:

| Column already exists | Becomes meaningful because |
|---|---|
| `range` 1 vs 2 vs 3 | Biters (range 1) get outranged by Hoe/Sickle/Bow; Bloom (range 3) answers ranged with range — pick your engagement |
| `cd` 1 s vs 2 s | Beat window count: a 1 s weapon swings inside a 4-beat window 4×; a 2 s weapon needs reach or shove to survive the comparison |
| `shove` (new derived, family map, no schema) | Axe/Pickaxe/Shovel move biters out of the lane edge — crowd tool as identity |
| `ammo` (Arrows) | Skirmisher duel: the bow's range 3 matches the Archer Goblin's range 3 — trading beats at standoff is a fair duel, ammo is the pressure to end it |
| Armor `def` | per-strike subtraction with a floor of 1: armor now front-loads a whole strike. Full 4-piece plate vs one Black Slime strike reads as 2–3 damage |

Zero new weapons/armor/items. Difficulty comes from reading, not stat
checks; the Stage 6 checklist pins that fight math at each capital L1–L5.

## Stage 4 — Technical architecture

### 4.1 Server state (schema delta — additive only, `#[default]`)

| Table | Column | Type | Meaning |
|---|---|---|---|
| `mob` | `next_strike_at` | `u64` | 0 = unset (schedule on next qualifying tick) |
| `Player` | (none new) | — | dodge reuses the parked `dash_until` / `dash_cd_until` columns + `player_dash` reducer; only constants and client input change |

No reducer signature changes ⇒ **bindings regen: schema field added ⇒ one
regen** after wasm build (types-only; all client code already handles
unknown fields).

### 4.2 Reducer / tick changes

| Unit | Change |
|---|---|
| `roamer_strike` / `raider_strike` | replaced by a shared `strike_beat(cx, mob, kind)`: windup emit → strike-or-whiff-or-dodge → reschedule. Contact-aggro still opens a schedule at `now + period/2` |
| `apply_player_damage` | keeps the armor flat-reduce + knockback + death; i-frame check stays at top. New: on **dodge** (dash_until active) emit `player_dodge` instead of silent return |
| windup emit | at `next_strike_at - 1`, only the *wind* event (stateless, idempotent with `combat_event` TTL) |
| `MOB_KINDS` | gains `strike_period_secs: u32` + boss flags per §3.4/§3.5 (constants, not table data) |
| `player_dash` | retunes into the dodge: `DASH_TICKS 1` stays (= the 1 s server i-frame window), `DASH_CD_SECS` stays 2, stale comment fixed (combat.rs ~270 says "6 s"; constant is 2) |
| cone multi-hit | extends `pick_arc_target` → `pick_arc_victims` (≤ 3 targets, primary full / others half). Edge: same-hex synthetic heading reuses today's code |
| shove | weapon family map in `player_combat.rs` (`WeaponKind::shoves()`), consumed by `player_attack`'s post-hit push |

### 4.3 New event kinds + client arms

- `combat_event kind "mob_wind"` (attacker_mob, target = player's hex):
  fx plan arm → present pose swell 0.9 s.
- `combat_event kind "player_dodge"`: fx arm → ghost streak + gray float.
- Everything else = existing fx arms re-weighted (numbers, hitstop, shake).

### 4.4 Client systems

| System | Change |
|---|---|
| `net/td.rs::td_combat_input` | Space binds to `player_dash`-via-dodge: 0.6 s local burst (the removed `dash_burst_secs` restored with its ghosts); mobile: dodge button |
| `plugins/player.rs::player_movement` | dash burst mechanics re-hosted as they existed before removal (recover from `c221ed39` history) |
| `TdFx` | `mob_wind` map keyed on mob_id (like `posing`): 0.9 s swell pose; `player_dodge` gray float |
| mob marker | wind state = the existing white-pulse op, window 0.55 → 0.9 s + 1.35× swell while winding |
| `spawn_zelda_number` | gains a gray "DODGED!" variant path |

### 4.5 Edge-case ledger (each lands in tasks + tests)

| Case | Answer |
|---|---|
| Server restart mid-wind | `next_strike_at - 1 >= now` misses (stale), re-stagger on next tick; no snap damage |
| Mob dies in windup | no wind event if the spawn_at/stagger guard ate it; strike guard re-checks the player hex anyway |
| Player leaves reach during wind | strike whiffs; mob repositions or re-arms. Optional `mob_whiff` event for client fx — decide in tasks |
| Player dies in windup | strike drop-checks `death_until`; wind aborted by scheduler |
| Dodge at strike instant | `dash_until > now` at damage-giving = dodge (server clock is the judge) |
| Two mobs wind on the same beat | fine — reads as pressure, is pressure |
| Latency > 1 s (wind unseen) | strike still lands / dodges server-side; you lose the read, not the dodge — cap the loss |
| Grace/respawn window | scheduler skips, reschedules with `+ period` |
| Rooted/walled mob | never schedules a strike (skip wind entirely); walls re-verified at strike instant if anyway |
| Wave lifecycle | schedule resets on every mob insert/update (0 = re-arm) |

## Stage 5 — Rollout (dependencies, priorities)

| Phase | Ships | Depends on | Server publish? |
|---|---|---|---|
| **P1 — The Beat (server core)** | `next_strike_at`, wind → strike/whiff/dodge contract, per-kind `strike_period_secs` table, remove the dps aura | — | yes + **bindings regen** (new schema field) |
| **P2 — Telegraph readability (client)** | windup pose/swell, `mob_wind` + `player_dodge` fx arms | P1 | no |
| **P3 — Dodge-step** | Space + mobile dodge button, burst restores, ghosts, gray float, HUD "Dodge [Space]" chip | P1 | no |
| **P4 — Strike honesty** | cone multi-hit (cap 3), family shove, tests | P1 | yes (logic only, publish for parity) |
| **P5 — Archetypes + boss** | skirmisher kite-step, bomber splash read, Big Slime 3-beat slam / summon / enrage, tests | P1–P4 | yes |
| **P6 — Balance + validation** | per-kind period/dmg curve pass over capital L1–L5, regression march sims final | all | no |

Every phase ends green (client 88+, core 367+, server 100 suites) with a
desktop panic-free run before any publish. Each publish = user playtests +
feedback funnel before the next phase builds on it.

## Stage 6 — Iteration and validation

**Automated (integer tests, per constitution ≥ 1 assert per requirement):**

- core: archetype table integrity (every kind mapped, periods ≥ 3 s), weapon
  `shoves()` family map, boss flag cadence.
- server: state-machine tests over a **25-trace sim** (existing march/knock
  sim style): wind → walk-out = whiff; wind → stay = damage + knock; wind →
  dodge = dodge event + zero hp delta; restart mid-windup = no snap strike;
  boss every-3rd-beat slam + summon cap + enrage threshold; cone cap 3 with
  primary full / secondary half; shove only on shove-weapons, refused on
  rooted/water/wall; armor floors at 1.
- client: fx-plan exhaustive arm tests for `mob_wind` / `player_dodge`
  (Spec 025 §16 pattern); B01-free HUD matrix note.

**Playtest checklist (each phase):**

1. *Can I react?* Every wind readable within the 1 s window — at base zoom
   and mid-swarm (15 kinds).
2. *Do I know why I took a hit?* Every damage lands one beat after a visible
   wind. A hit the player didn't see telegraphed = bug or P2 fail.
3. *Does dodge matter?* ≥ 3 forced dodge moments per wave.
4. *Is standing viable?* "Stand still and soak" must be strictly worse than
   "read and move" — otherwise rebalance.
5. *Does the boss teach?* Big Slime survivable by pattern reading, not by
   armor stats.
6. *Did the feel survive?* Hitstop, Zelda numbers, shake intact — compare
   clips to c221ed39 when in doubt.

**Balance bounds (pin):** standing still through an L2 wave with the starter
kit should kill the player by wave end; actively dodging + strike-weaving
should clear it with ~40 % HP left at L2, ~15 % vs the boss wave. TTK per
kind pinned in tasks.

## Explicit non-goals

- Sub-second server tick / per-frame combat netcode (architecture stays).
- Block/parry, combos, magic, second resources, consumable heal item.
- Player-vs-player anything (constitution: PvP out until auth/market land).
- New mob art / new item art (15-kind roster + existing sheets only —
  nothing we don't have sprites for).
- Tower/structure siege model changes; raid lanes, compositions, drops,
  xp flows all untouched.
- Fourth knockback redesign beyond the two already pinned rules (mobs shoved
  per swing, players shoved straight off) — arrival polish only if a bug.

## Risks

- **R1 — period values fossilize**: the windup/strike contract pins the
  shape, but per-kind periods and strike damage are tunable numbers.
  Mitigation: spec pins bounds, tasks table carries values, P6 owns the
  final curve; all server-side, one publish per retune.
- **R2 — wind unreadable on small sprites**: 15 kinds render 32×32. Mitigation:
  1.35× swell + pose freeze + the gray float is the spec floor; P2 is
  client-tunable with no publish dependency.
- **R3 — cone multi-hit meets 71 weapons**: full/half math must not trip the
  bow (single-target identity). Mitigation: family map as single source;
  boundary tests pin `shoves()` and hit-count per weapon family.
- **R4 — mobile touch budget**: dodge must not crowd the joystick. Mitigation:
  touch.rs already ships a button surface; tasks pick placement from
  screenshots before code.
