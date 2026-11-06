# Spec 036 — Tasks

> Status: **P1–P5 implemented + committed; P6 automated validation done,
> live playtest pending.** (An earlier draft of this file marked phases
> DONE that were never written; this file is the truthful ledger. The
> spec's Stage 5 plan held: server phases publish per-retune, client
> phases ride the same repo.)
>
> Server sim = 1 Hz; every beat lands on `now` seconds.
> Publish = `cargo build -p idlecore-server --release --target
> wasm32-unknown-unknown` + `spacetime publish` (catch: schema field
> added ⇒ regen bindings BEFORE client build; new columns APPEND at the
> struct end — an insert mid-struct is a "reorder" and forces
> --delete-data).

## T1 (P1) — The beat, server core — DONE (commit 7068ebb0)

- [x] `MobKind.strike_period_secs` added (core/defense_config.rs): Biter 4,
  Sapper 4, Skirmisher 5, Bomber 7, Bloom 6, Boss 3. Bounds test 3..=10
  (`every_kind_has_a_beat_period_and_archetype`).
- [x] `mob.next_strike_at: u64 #[default(0)]` + `mob.beat_count: u32`
  (types.rs, appended at struct end per migration rule).
- [x] `CE_MOB_WIND` ("mob_wind"), `CE_PLAYER_DODGE` ("player_dodge").
- [x] `combat.rs`: pure `beat_phase` / `strike_burst` (`dps×2, min 4`) +
  tests; `roamer_strike` = arm → Wind → Strike/whiff/dodge state machine;
  whiffs reschedule and re-open the chase-step. Contact aggro opens the
  schedule at half period. Stale (restart) arms re-stagger — no snap.
- [x] `apply_player_damage`: `dash_until > now` ⇒ `CE_PLAYER_DODGE` row
  (was a silent false).
- [ ] Publish to the live module + bindings regen (needs the user's
  `spacetime publish`; bindings ARE compatible either way — new columns
  are additive and the client ignores unknown fields until regen).

## T2 (P2) — Telegraph readability, client — DONE (commit 56ed2c6f)

- [x] fx arms `mob_wind` (0.9 s pose window + white pulse; Big Slime 1.8 s
  + 1.35× body swell — `WIND_POSE_SECS` / `WIND_POSE_BOSS_SECS` /
  `WIND_BOSS_SWELL`, pinned by test) and `player_dodge` (gray "DODGED!"
  float, no shake). fx coverage test extended to both kinds.
- [x] `animate_mobs` gained the wind-white pulse pass + boss swell ease.
- [ ] Optional `mob_whiff` fx arm: deferred until playtest says the miss
  reads badly.
- [ ] Mobile: on-screen dodge button (touch.rs button surface exists;
  layout pending).

## T3 (P3) — Dodge-step, client — DONE desktop (commit 56ed2c6f)

- [x] Space → `player_dash` (2 s server cd, 1 s i-frame window), local
  0.6 s ×3 burst (`DASH_DURATION_SECS = 0.6` restored), heading =
  movement direction (facing if idle).
- [x] Dash ghosts system re-registered; HUD chip "Dodge [Space]" drains
  the 2 s cooldown.
- [ ] Mobile: mirror the Space button (see T2).

## T4 (P4) — Strike honesty — DONE (commit 382429cf)

- [x] `pick_arc_victims` (≤ 3 nearest-first; bow passes `ranged=true` → 1);
  primary full / side targets half, min 1; per-victim event rows so each
  hit pops its own number.
- [x] `weapon_shoves` family map (Axe/Pickaxe/Shovel) gates the mob shove;
  core test pins all 71 ids via `WeaponAnim` identity.

## T5 (P5) — Archetypes + boss — DONE (commit 9cbea776)

- [x] Skirmisher kite-step: after a landed strike, range > 1 kinds step one
  hex away from the player (land + wall honest, only if it opens distance).
- [x] Bomber splash read: on a landed strike, every non-trap structure
  within 1 hex of the PLAYER's hex takes half burst (chest refund + rubble
  inline).
- [x] Boss (Big Slime): slam = `strike_burst ×3` (enraged ×4 at hp ≤ 30 %,
  pure `boss_enraged` / `boss_slam_mult` + tests), period 3 → 2 enrage,
  summon beat every other slam → 1 wave-owned "Slime", ≤ 2 live adds,
  spawned adjacent with caught-up `path_idx`, aggro inherited.

## T6 (P6) — Balance + validation — PARTIALLY DONE

- [x] 25-trace march+beat regression sim (beat_sim_25_traces over the
  pure state machine: wind precedes every strike, strangeness none,
  restart-mid-windup never snaps) — in `combat.rs` tests.
- [x] Suites green: core 368, server 107, client 89; wasm server release
  build passes.
- [ ] Playtest checklist (spec §6) executed WITH THE USER:
  can-I-react / do-I-know-why-I-got-hit / ≥ 3 forced dodges per wave /
  standing worse than reading / boss teaches by pattern / feel intact.
- [ ] Publish to nestfeed + `spacetime publish` (needs SpacetimeDB access
  + binding regen in this order: wasm build → publish → regen → client).
- [ ] Freeze period/burst numbers after playtest feedback (server-only
  retune).
