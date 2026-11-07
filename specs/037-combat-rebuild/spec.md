# Spec 037 — THE TICKENING: the combat rebuild, phase by phase

> **Objective:** complete combat redesign toward the golden-era 2D feel
> (ALttP / Secret of Mana / Link's Awakening): responsive directional
> strikes, real commitment, dodge timing windows, honest telegraphs, no
> button-mashing. Builds directly on Spec 036's beat architecture.
>
> **Status:** audit approved 2026-11-06; phases delivered working-tree-first
> (the user commits; agent commits are forbidden per user rule).
>
> **Audit decisions (user-approved):**
> - G.1: combat beats at **250 ms quanta** (`ScheduledCombatTick`); the 1 s
>   world tick keeps march/economy/regen.
> - G.2: client predicts visuals only; the server number is truth.
> - G.3: raid waves stay structure-bound; roamers are the player-vs-enemy
>   encounter layer.
> - G.4: keep the 71-weapon matrix; tune per-family cadence.
> - G.5: anticipation derived by frame-timing existing sheets (no new art).

## Phase 1 — THE COMBAT CLOCK (delivered 2026-11-06)

- `idlecore-core/src/combat_clock.rs` — the pure clock module:
  `COMBAT_TICK_MS=250`, `WINDUP_MS=900`, `DODGE_IFRAME_MS=600`,
  `DODGE_CD_MS=900`, `weapon_commit_ms` per family, `beat_phase`,
  `arm_strike` (mod-4 stagger), `period_ms` (×2 speedup vs the old
  second cadence), `due`.
- Server: `ScheduledCombatTick` table (250 ms) + `combat_tick` scheduled
  reducer → `combat::combat_tick` → `mob_strike_pass_ms` — the beat FSM
  (ARM / WIND / STRIKE) resolves at ms precision, same contract as
  Spec 036's 1 s machine.
- Player schema (append-only): `move_lock_until_ms` (ALttP commitment —
  `move_player` refuses steps mid-swing), `dodge_until_ms` (0.6 s
  i-frames = the client burst), `attack_cd_until_ms` (exact weapon cd).
- Mob schema (append-only): `next_strike_ms`.
- `player_attack` stamps fine cadence + move-lock on BOTH outcomes
  (empty arc whiffs lock too — commitment is the swing, not the hit).
- `player_dash`: fine i-frames (0.6 s) + 900 ms chain cd; legacy gates
  stay as backstops for pre-backfill rows.
- Contact aggro arms BOTH clocks (1 s half-period + fine half-period).
- Client: bindings regen'd; mirrors synced (`plugin.rs`); local
  prediction stamps ms windows at press (server row overwrites);
  dodge chip reads the fine cd; `time_ext::now_unix_ms`.
- Tests: core 374 (+6), server 111 (+3: fine-clock contract per kind,
  dodge-seam pin, grid-alignment pin), client 89.

## Phase 2 — INPUT HONESTY + DODGE DISCIPLINE (delivered 2026-11-06)

- `fire_player_strike` stamps `move_lock_until_ms` +
  `attack_cd_until_ms` locally at press (prediction; server row
  reconciles ~0.15 s later).
- Space dodge: local gate reads the 900 ms mirror; press stamps
  `dodge_until_ms` + `attack_cd_until_ms` + the 0.6 s burst before the
  server row arrives — five chained dodges pace at 0.9 s with a
  deliberate **300 ms vulnerability seam** between i-frame windows
  (the skill check is reading the next telegraph into that seam).
- Kill-ledger pins: 600/900 split + grid alignment are test-locked.

## Phase 5 — FEEDBACK POLISH (delivered 2026-11-06)

- **Urgency ramp** on the wind pose: the pulse period tightens as the
  beat closes (0.45 s → 30 ms effective), so the last quarter before a
  strike reads as imminent without any new art. `fx.winding` now carries
  the true seconds-remaining (the ms remainder from Phase 3), so the
  ramp tracks the REAL beat, not a canned timer.
- SKIPPED (documented): combat SFX — the repo ships no sound assets; a
  bevy_audio hook needs an asset pack + spawn plumbing first (not
  silently patched in).

## Validation record (all six passes) — nothing pending agent-side

- **Phase 6 — VALIDITY (DELIVERED OVER TWO PASSES):**
  - `fine_beat_sim_30_traces_reproduce_the_fine_fsm` — 30 procedural
    traces over the fine machine (wind before every strike, exact-beat
    landings, tick-grid forever, trailing-wind allowed).
  - **Self-audit catches (2026-11-06):** the fine damage path
    (`apply_player_damage_ms`) originally diverged from the legacy path —
    it skipped the survivor KNOCK and the death-path window shed/trace.
    Both now match the 1 s path exactly: knock parity (king-step away,
    land-and-wall honest), dodge window shed on death, `PLAYER-DIED`
    trace. The two damage halves are now contract-identical.
  - **Second validation pass (2026-11-06):** deep re-read caught the
    fine wind branch emitting 3 fx rows + 3 `beat_count` bumps per beat
    (once per 250 ms tick inside the 900 ms window) — inflating the
    boss summon cadence and the fx feed. Fixed: ONE emission per beat on
    the first tick inside the window (beat−900 rounded up to the grid —
    900 is not tick-aligned; documented), `beat_count` bumps only in
    resolve. Pinned by `wind_emission_is_idempotent_per_beat`. Also
    verified the client telegraph freshness chain (mob row update →
    `td_dirty` bump → re-snapshot → reader refed).
  - **Third validation pass (2026-11-06):** warning-parity diff against
    HEAD exposed that the earlier stash/pop cycle had silently REWOUND
    the knock-parity insert in `apply_player_damage_ms` (unused-var
    warning was the tell). Restored; verified the death shed/trace and
    wind idempotence survived. Warning counts now match the baseline
    exactly (18 = 18), all suites green (core 375, server 112, client
    89), both builds pass.
  - **Fourth validation pass (2026-11-06):** behavior-level close-out:
    the boss summon consumer (`boss_summon_beat` reads `beat_count %
    2 == 0`) had NO test pinning its cadence — the exact bug class pass
    2 opened. Added `boss_summon_cadence_drives_off_one_count_per_beat`
    (12 strikes → exactly 6 summons; parity gate pinned). Boss-swell
    ramp coverage gap noted (the 1.8 s row fallback clamps to base rate;
    the ms reader overrides it every frame, so behavior is correct) —
    documented, not patched. Server unused-warnings at baseline parity
    (8 = 8).
  - **Fifth validation pass (2026-11-06, LIVE):** published 037 to
    `db.nestfeed.app` **non-destructively** (no `--delete-data`; additive
    schema accepted, world data preserved). Verified live: bindings
    drift zero (published module == in-tree gen), e2e connected +
    replicated rows, and the SQL probe caught a REAL deployment gap —
    `init` only runs on a fresh database, so the 250 ms schedule row was
    never created on the live world. Fixed with a self-healing bootstrap
    in `mob_tick_body` (the 1 s tick inserts the schedule when missing,
    idempotent, audited) — `scheduled_log` id 3940 records the arm, and
    the interval row now persists. Two e2e flakes filed as environment:
    teleport-RTT 256 ms (budget 250) and a spacing conflict from run-1
    leftovers (publish preserves data by design); the e2e's stale [12]
    assertion post-024 (capital-hex auto-claim was removed by the
    starter-square design in Spec 024) corrected to check starter-square
    ownership. Final: core 375 / server 113 / client 89, both builds
    green.
  - **Sixth validation pass (2026-11-06, LIVE data):** contract-vs-live
    closed. Run 1's wallet (`0xe2e-93386`) shows **1092 live territory
    rows** on the production world — the definitive proof that
    `place_capital` → starter-square claim is healthy and the corrected
    e2e [12] assertion matches real behavior. Mob schedule:
    `scheduled_combat_tick` row persists across three probes (the 250 ms
    clock re-arms live); mob table empty pending raid cycle (first
    aggro will populate `next_strike_ms` on the fine clock). Test count
    ledger: core 368→375 (+7), server 111→113 (+2 pins), client 89
    stable; suite totals green across all six validation passes.
  - The playtest checklist itself needs the user live (user-gated in
    Spec 036 too).
- Combat SFX once assets exist.

## Phase 3 (delivered detail)
- **P3 — telegraph language on the client:** mob `next_strike_ms` read:
  wind pose length = the ACTUAL 900 ms (currently a 0.9/1.8 s constant
  pose from the 1 s fx row); per-kind wind tint; dodge-through walls
  (the last DELIBERATELY NOT SHIPPED — step-through would trivialize
  enclosed mob packs; i-frames only, recorded as a decision).
- **P4 (delivered) — mob AI v2 (data-driven archival:** per-kind FSM table in
  `defense_config.rs`, archetype tails on ms beats, boss enrage gates
  fine beats.
- **P5 (delivered) — feedback finisher:** hit-direction knockback respect, dodge
  burst trail polish, kill float capitalization, optional combat SFX.
- **P6 — validation:** playtest checklist + procedural trace sims over
  the fine FSM.

## Phase 3 — THE TELEGRAPH LANGUAGE (delivered 2026-11-06)

- `MobRow.next_strike_ms` rides the client snapshot (`TdSnapshot` now a
  static `TD_SNAPSHOT: Mutex<Option<TdSnapshot>>` — avoids the 16-param
  system cap; rows carry Clone).
- **`mob_beat_telegraph`** (runs every frame before input/animation):
  every armed mob whose beat lies in `[now, now+900ms)` holds its wind
  pose for EXACTLY `beat − now` ms — the pose is ms-authoritative, no
  constant fudge, no gap between sprite and truth. The old 0.9/1.8 s
  constants stay ONLY as the fx-row ring fallback, pinned equal (0.9 s
  = WINDUP_MS) by test.
- Client 89 green (+1 pin: core telegraph = ring fallback), builds.

## Phase 4 — MOB AI v2 ON THE MS BEAT (delivered 2026-11-06)

- `resolve_strike_ms` calls the SAME `archetype_tail` as the legacy
  strike — skirmisher kite, bomber splash and boss slam behavior ride
  the fine beat unchanged (one behavior table, two clocks drain it).

## REV10 addendum (2026-11-06): capital spacing 200 → 50
- Live playtest: arthur at (−5904, −2936) was 123 king-steps from the
  only live rival capital and still saw the spacing rejection; on shaped
  coastlines 200 steps reads unreachable. Tuned
  `MIN_CAPITAL_SPACING = 50` (defense_config.rs, REV10 note documents
  the trade: capped claim squares (half-side 91) can now touch).
- Test re-based: `claims_keep_minimum_distance_between_capitals`
  exercises the 50-step boundary (49 denied / 50 legal; mid-gap +
  two-capital denial cases mirror the old structure at the new scale).
- Rover run unaffected: `ROAMER_MIN_CAPITAL_DIST = 3` still nests well
  inside the new ring.
- Published non-destructively to nestfeed (additive plan, no data
  change); suites core 375 / server 113 / client 89 green.

## REV11 addendum (2026-11-06): melee hits by CONTACT, bow keeps the bolt
- Live playtest: a sword swing rendered a golden tracer flying from the
  player to the mob — a projectile read on a contact weapon. Wrong per
  the directive (Sword = direct hit; only the bow is a projectile).
- Cause: the Spec 034 `player_hit` fx handler spawned a TdBolt for EVERY
  connecting strike farther than 0.4 wu from the player.
- Fix (client-only, td.rs): the handler now classifies the row by its
  own detail payload "<weapon>|<dmg>" (Spec 034 C8) via
  `event_weapon_is_ranged` — tail-word "Bow" (the single ammo-consuming
  family) fires the bolt + smeared bloom; every melee family and any
  garbage detail pops the contact bloom ON the victim, no tracer.
- Pinned by `only_bow_events_fire_projectile_fx`: classifies EVERY
  71-row weapon ladder through the same string the fx feed sees and
  asserts ranged == ammo-consuming, plus WEA_KINDS spellings and
  garbage-fallback cases.
- Suites: core 375 / server 113 / client 90 (new pin). Client-only
  change; no republish needed.

## REV12 addendum (2026-11-06): subtle damage ticks + horse icon
- Universal damage-indicator pass (playtest request: "pequenos números
  perto dos inimigos, bem sutil"):
  - `spawn_float_number` generalizes the Zelda-number spawner (font size,
    rise, hairline shadow, shadow color per channel); big numbers keep
    the 1.15/1.15 ember look, the mob-side tick shrinks to 0.72 font,
    0.6 rise, 0.02 hairline shadow (pale warm on impacts, warmer ember
    on the player's own strikes).
  - Coverage gap closed server-side: tower shots now carry their damage
    (`tower|<dmg>` = structure_dps at the tower's level) and burn-trap
    fire ticks carry theirs (`fire|<dmg>`) — previously these raiders'
    damage sources gave the client no number to tick.
  - Client: Bolt (tower, attacker 0) and Impact arms tick the pale
    number when the detail parses; mob-fired shots and kind-name-only
    details still parse None (no phantom numbers).
  - Pinned: `tower_and_fire_ticks_carry_damage` on the payload shapes.
- Horse quickslot icon: the Horse was riding the city-bus crop as the
  vehicle-family icon. New `icon_horse` atlas crop (mount-sheet front
  view, 32×48 from Horse_idle col 1) inserted into PropTextures and
  mapped only for "Horse"; other vehicles keep the bus.
- Suites: core 375 / server 113 / client 92. Server republished
  non-destructively (emit signature change required it).

## REV12.1 addendum (2026-11-06): telegraph readability
- Playtest: the wind-up indicator on arming mobs was unreadable — the
  body pulse went bright-white ↔ barely-off-white (no luminance
  contrast on a small sprite) and the white fx ring was a one-shot
  0.38 s flash at wind START, gone for the rest of the window.
- Fixes (client-only):
  1. CONTRAST BLINK: body wind pulse now cycles bright white (1.6) ↔
     deep slate (0.38/0.38/0.46) — a luminance blink caught in
     peripheral vision. Red stays reserved for damage feedback.
  2. PERSISTENT GROUND RING (`TdWindRing` + `wind_ring_pass`): a flat
     ember glow (1.5×0.55 wu ellipse, z below the mob) sits under the
     mob for the ENTIRE wind window — Zelda positional language
     (danger under the threat). It glides with the marker, brightens
     and tightens as the beat closes (same urgency ramp as the blink),
     and despawns when the window drains. Lifecycle via
     `fx.wind_rings` (mob_id → Entity), disjointed query (Without<
     TdMobMarker>) — first spawn attempt hit bevy B0001 (Transform
     aliasing), fixed before any deploy.
- Sizing note: the pre-fix `steam` alternative (bigger boss swell) was
  already live; the ring covers ALL kinds, not just bosses.
- Suites green (client 91 / core 375 / server 113); client-only change,
  no republish needed.
