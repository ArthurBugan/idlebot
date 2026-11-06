# Spec 036 — Tasks

> Status: **writing — code unchanged yet.** (An earlier draft of this file
> marked phases DONE that were never written; it is rewritten below. The
> spec leaves Stage 5's plan untouched: every phase ends green, publish
> after each server-phase with a real playtest before the next.)
>
> Server sim = 1 Hz; every beat lands on `now` seconds.
> Publish = `cargo build -p idlecore-server --target wasm32-unknown-unknown
> --release` + `spacetime publish` (catch: schema field added ⇒ regen
> bindings BEFORE client build; new columns APPEND at the struct end — an
> insert mid-struct is a "reorder" and forces --delete-data).

## T1 (P1) — The beat, server core — TODO

- [ ] `MobKind.strike_period_secs` added (core/defense_config.rs): Biter 4,
  Sapper 4, Skirmisher 5, Bomber 7, Bloom 6, Boss 3. Bounds test 3..=10.
- [ ] `mob.next_strike_at: u64 #[default(0)]` + `mob.beat_count: u32`
  (types.rs, appended at struct end per migration rule).
- [ ] `CE_MOB_WIND` ("mob_wind"), `CE_PLAYER_DODGE` ("player_dodge").
- [ ] `combat.rs`: pure `beat_phase` / `strike_burst` (`dps×2, min 4`) +
  tests; `roamer_strike` = arm → Wind → Strike/whiff state machine; whiffs
  reschedule and re-open the chase-step.
- [ ] `apply_player_damage`: `dash_until > now` ⇒ `CE_PLAYER_DODGE` row
  (was a silent false).
- [ ] Publish to nestfeed + bindings regen.

## T2 (P2) — Telegraph readability, client — TODO

- [ ] fx arms `mob_wind` (0.9 s pose window + white pulse; Big Slime ring
  1.8 + 1.35× body swell) and `player_dodge` (gray "DODGED!" float).
- [ ] `animate_mobs` gains the swell pass (spawn-pop keeps its scale).
- [ ] Optional `mob_whiff` fx arm: decide during playtest whether the miss
  reads badly; deferred otherwise.
- [ ] Mobile: on-screen dodge button (touch.rs flags usable; layout pending).

## T3 (P3) — Dodge-step, client — TODO

- [ ] Space → `player_dash` (2 s server cd, 1 s i-frame window), local
  0.6 s ×3 burst (`DODGE_BURST_SECS = 0.6`).
- [ ] Dash ghosts system re-registered; HUD chip "Dodge [Space]".
- [ ] Mobile: mirror the Space button (see T2).

## T4 (P4) — Strike honesty — TODO

- [ ] `pick_arc_victims` (≤ 3 nearest-first; bow passes `ranged=true` → 1);
  primary full / side targets half, min 1; per-victim event rows so each
  hit pops its own number.
- [ ] `weapon_shoves` family map (Axe/Pickaxe/Shovel) gates the mob shove;
  core test pins all 71 ids.

## T5 (P5) — Archetypes + boss — TODO

- [ ] Skirmisher kite-step: after a landed strike, range > 1 kinds step one
  hex away from the player (land + wall honest, only if it opens distance).
- [ ] Boss (Big Slime): slam = `strike_burst ×3` (enraged ×4 at hp ≤ 30 %),
  period 3 → 2 enrage, adjacent-structure splash at half slam (direct
  destroy + rubble, chest refund), summon beat every other slam → 1
  wave-owned "Slime", ≤ 2 live adds, spawned adjacent with caught-up
  `path_idx`, aggro inherited.

## T6 (P6) — Balance + validation — TODO

- [ ] 25-trace march+beat regression sim (march sims exist; extend for
  strike beats) + playtest checklist (spec §6) executed with the user.
- [ ] Freeze period/burst numbers after playtest feedback (server-only
  retune).
