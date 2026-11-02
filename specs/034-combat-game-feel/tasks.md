# Spec 034 tasks — combat game feel

Build order: server cadence first (S1 unlocks everything), then juice layers
purely additive client-side.

- [x] T1 (S1) `player_attack` drops the ACTION_COOLDOWN gate; own `attack_cd_until`
  only; test: strike twice inside 5 s accepted, build actions still gated
- [x] T2 (S2) DASH_CD_SECS 6→3 in player_combat.rs + tests
- [x] T3 (S3) player_attack emits `detail = "<weapon>|<dmg>"`; server test
- [x] T4 (S4) CE_MOB_FELL "mob_fell" const, emitted by kill_or_wound delete path
  (roamer + raider); server test
- [x] T5 (C1) hold-to-swing: LMB held re-strikes toward cursor on local cd expiry,
  same guards as the first click; pure cadence helper unit-tested
  — **REV after playtest: REVERTED.** A held button read as endless sliding +
  machine swings; only a real LMB click strikes now (KeyF unchanged). The
  local-cd plumbing (`TdFx::last_attack_sent`) stays for future use.
- [x] T6 (C2) KeyF path confirmed unchanged
- [x] T7 (C3+C4) skin anim: lunge offset curve + last-frame hold for slow weapons;
  pure offset/curve functions unit-tested — NOTE: implemented as a one-shot
  velocity impulse consumed by `player_movement` (`LUNGE_IMPULSE`, the exp-ease
  recovers it), and the last-frame recover-hold was dropped (the idle fallback
  reads fine at 1 s cadences)
- [x] T8 (C5) hitstop resource + Time<Virtual> dip driver (0.3 for 70 ms on
  player_hit/mob_fell)
- [x] T9 (C6) camera shake: CamerasShake resource, additive jitter in follow_camera
- [x] T10 (C7) dash ghost sprites during the local burst window
- [x] T11 (C8+C9) damage floats parsed from detail; impact bloom offset toward
  strike; fx_plan covers "mob_fell"
- [x] T12 tests: server cadence/kill events; client fx coverage incl new kind;
  publish server + regen bindings unchanged signatures (types.rs only)

## Build-out notes (2026-11-02)

- Server published to `nestfeed`; no reducer signatures changed → no binding
  regen.
- B0001 trap hit twice while adding the damage-number ticker: Bevy treats two
  `&mut X` query params as conflicting UNLESS a `Without` exists between them
  (a `With` difference is NOT enough). toasts/bolts/floats all got mutual
  `Without` filters.
- `td_combat_input` and `sync_td_entities` grow params against the 16-SystemParam
  cap: mode+panel merged via `ParamSet`, and the structures/capitals/overlays
  trio merged into one `Or` respawn query.
- `Time<Virtual>::set_relative_speed(0.3)` is the whole hitstop (70–90 ms);
  movement dt clamp keeps the dip safe.
- Client tests need `Time<Virtual>` present in the tick system — combat_fx_tick
  registered as before; no test-harness changes.
