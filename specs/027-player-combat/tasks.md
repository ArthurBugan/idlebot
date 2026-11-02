# Spec 027 — Tasks

## Phase G — §14 addendum (2026-11-02): Bow + ammo + forge UI
- [x] G1 core: `WEAPONS` + tier 5 Bow (dmg 12, 2 s cd, 3 hex, Arrow ammo),
      `ARROW_COST` (2 Wood + 1 Stone) + `ARROW_BATCH_CAP` = 32; sheet pins
      updated (cd ≥ 1, melee ammo-free).
- [x] G2 server: `player_attack` ammo gate + per-strike consumption (empty
      arcs cost cd only); `craft_arrows` reducer (1..=32, all-or-nothing
      exact refund); `arrow_materials` unit test; published + bindings regen.
- [x] G3 client: `net/forge.rs` panel (KeyG) — data-driven weapon rows,
      owned/equipped/affordable tints, Arrow ×10 batch button; HUD chip
      shows `· N Arrow` for bows; KeyC cycle + KeyV ladder extend to tier 5;
      Arrow icon reuses ArrowTower art. Forge pure-fn tests +
      `ClientPlayer` arrow display; 76 client tests green.
- [ ] G4 (follow-up): bow draw/loose pose art + `player_hit` bolt visual
      tuned to the 3-hex arc (REV4 crop rule applies when art is drawn).

All phases done 2026-11-02; deviations recorded in spec.md §13.

## Phase A — core combat state
- [x] A1 schema: `player_hp / weapon / attack_cd_until / dash_cd_until /
      dash_until / hostile_until / death_until` (all `#[default]`-seeded) +
      bindings regen + publish.
      *(as-built adds `grace_until` + `weapons_owned: u16` → 9 cols; mob
      table +4: `aggro_target / aggro_until_secs / pack_id / born_at_secs`.)*
- [x] A2 core: `player_max_hp(level) = 80 + 10·level`, 8-way heading arc
      helper, weapon table (`WEAPONS` const, ids 0–4), unit tests.
      *(cores/idlecore-core/src/player_combat.rs — 5 tests; negative-dot
      heading fix pinned.)*

## Phase B — attack
- [x] B1 `player_attack(direction)` reducer: cd gate + arc hit pick integer
      test; `combat_event { "player_hit" }`.
      *(crates/idlecore-server/src/combat.rs — reduced-cost, gates but does
      not extend the 5 s `last_action_at` cooldown; empty arc still spends
      the attack cd; first attack/dash sheds grace.)*
- [x] B2 aggro transition (mob targets player 20 s after being hit) +
      roamer/raider shared behavior flags.
      *(mobs box `aggro_target` + `MOB_AGGRO_SECS = 20`; aggroed raiders
      march + strike the player hex, skip structure/capital engagement.)*
- [x] B3 player-take-damage pipeline on the 1s tick (adjacency for melee
      kinds, ranged reach) → `player_hurt` events, `hostile_until` pause
      of regen.
      *(i-frames `dash_until > now` + grace block strikes; tower/trap
      credit map feeds `combat::kill_or_wound`.)*

## Phase C — dash
- [x] C1 `player_dash(direction)`: 6 s cd, 0.4 s `dash_until`, speed ×3 in
      `displacement_cap` validation path; server tests fit movement suite.
      *(server DASH_TICKS = 1 (0.4 s rounds up at 1 s resolution); client
      runs the real 0.4 s ×3 local burst.)*
- [x] C2 i-frames: mob tick skips player-targeting while `dash_until > now`.
- [x] C3 client: after-trail fx + dash HUD cooldown (reuse cooldown slot
      pattern; watch B0001 disjointness).
      *(Space key, not Shift; local burst in `player_movement`; dash chip
      text reuses `fmt_countdown`.)*

## Phase D — roaming hostiles
- [x] D1 scheduled spawner: Slime packs (2–4), `capital_id = 0 / wave_id
      = 0`, distance window (≥3 from capitals), global cap 6 packs,
      300 s despawn; server tests.
      *(60 s spawn attempts, needs an online living player, materialized
      Grass ≥3 from every capital and ≤20 from an online player.)*
- [x] D2 wander AI (slow idle random walk) + straight-march-only trap
      exclusion still holds (roamers are mobs; traps apply? decide: yes —
      traps hurt roamers, cheap early-game defense synergy).
      *(wander = 1 step per `step_interval × 4`; contact aggro at
      `tile_distance ≤ 1`; packs track `pack_id`.)*
- [x] D3 kill credit: drops → killer inventory (SlimeGel 35% + Bone 25%) +
      50 player XP; capital XP *not* awarded for roamers.
      *(kill_or_wound's `credit_fallback: Option<&str>`; raiders keep drop
      + CAPITAL_XP_PER_MOB.)*

## Phase E — weapons
- [x] E1 weapon items + craft recipes (022 plumbing) + `equip_weapon(u8)`
      reducer; hotbar weapon slot UI.
      *(weapons_owned bitmask; first equip consumes materials, switching
      free; KeyC cycles owned tiers, KeyV buys the next tier.)*
- [ ] E2 per-tier swing visuals (32×32 sheets), crops verified before
      coding (REV4 crop-verification rule).
      *(deferred — weapon swing sprite frames not drawn; combat fx use
      the existing arc/impact pipeline.)*

## Phase F — death / respawn / raid threat
- [x] F1 death flow: `death_until` veil + countdown + capital respawn +
      60 s grace (fresh-spawn rule; drops on first attack).
- [x] F2 out-of-combat regen (+2/tick past `hostile_until`).
- [x] F3 raid-side threat (§8): aggroed raiders dps the player; kill/drop
      split (roamer vs. raider credit); fx plan exhaustive for
      `player_hit / player_hurt` (025 §16 pattern).
      *(client pin test `fx_plan_covers_every_server_event_kind` extended
      with both new kinds.)*
