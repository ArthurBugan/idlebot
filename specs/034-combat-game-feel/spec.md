# Spec 034 — Combat game feel: HLD/Terraria cadence, juice and attack rhythm

> **Objective:** movement + combat that FEEL alive. Terraria gives hold-to-swing
> melee that keeps firing while you run; Hyper Light Drifter gives snappy dashing,
> lunge swings, hitstop and screenshake. The lattice/server-tick architecture
> stays; the dead part is cadence and lack of feedback, not movement freedom.
>
> **Status:** spec written 2026-11-02, implementation in tasks.md.

## 1. Diagnosis — why combat currently feels dead

| # | finding | consequence |
|---|---------|-------------|
| D1 | `player_attack` gates on the **global 5 s ACTION_COOLDOWN** (`last_action_at`) shared with build actions | a 1 s-cd weapon actually swings every 5 s — cadence killed at the source |
| D2 | dash cooldown 6 s | HLD-feel needs the dash as an *every-encounter* tool |
| D3 | `player_hit` fx = thin gold bolt + bloom, no reaction on the strike order itself | whiff and hit are near-identical; kills look exactly like grazes |
| D4 | attacks start only on press (LMB or F) | no auto-swing on hold — sustained melee needs frantic clicking |
| D5 | one 0.07 s/frame clip plays for every weapon | slow weapons (2 s cd) finish their 0.5 s clip, then freeze mid-pose; nothing sells weight |
| D6 | no hitstop / shake / numbers | damage lands silently |

Movement itself (continuous WASD + exp-eased velocity + slide) already matches
the target: **it stays**. This spec is cadence + juice.

## 2. Server changes (authoritative, small)

- **S1 — attack cadence (D1):** `player_attack` stops consulting
  `ACTION_COOLDOWN_SECS`. Attacks honor only `attack_cd_until` (their own
  weapon cd, ≥ 1 s server round). Build/repair actions keep the 5 s gate in
  `defense.rs` — no exploit opens up, melee and building stop fighting over
  one meter.
- **S2 — dash cadence (D2):** `DASH_CD_SECS` 6 → 3. Burst (×3 speed, 0.4 s
  i-frames, 1-tick rounding) unchanged.
- **S3 — strike payload (D3/D6):** the `player_hit` event's `detail` carries
  `"<weapon>|<dmg>"` so the client floats the number without a schema change.
- **S4 — kill pulse:** new event kind `mob_fell` emitted when `kill_or_wound`
  deletes the mob row (roamers + raiders). Client shows a bigger burst — kills
  finally read as kills.
- **S5 — keep fairness walls:** range hexes, 120° heading cone, arrow ammo, i-
  frames, occupancy: none touched. No reducer signature changes (no binding regen).

## 3. Client input changes

- **C1 — ~~hold-to-swing (D4, Terraria)~~ REVOKED (playtest):** holding LMB
  auto-struck toward the cursor while the local cd estimate ran down. First
  session read it as endless sliding + machine-gun swings — attack is
  CLICK-ONLY again; the Terraria-hold idea stays parked.
- **C2 — KeyF unchanged:** strikes the facing arc.

## 4. Client game feel

- **C3 — lunge (D5, HLD):** during every attack clip the sprite slides toward
  the strike heading: 0.22 × tile forward over the first 40 % of the clip,
  eased, then eases back. Pure offset, never moves the player position.
- **C4 — residue frames:** a slow weapon's clip freezes on the frame death
  (the rest of the cadence holds the recover frame), reads as a heavier weapon.
- **C5 — hitstop (D6, HLD):** on a confirmed 3-falling event
  (`player_hit`/`mob_fell`) the run-loop time scale dips to 0.3 for ~70 ms
  (`Time<Virtual>`), then snaps back — the whole world breathes for a beat.
- **C6 — screenshake (D6):** decaying jitter on the camera offset — small on
  own hits, bigger on own falls and when hurt.
- **C7 — dash ghosts (HLD):** while the dash burst lives, white translucent
  silhouettes spawn every 60 ms at the player position and fade over 0.25 s.
- **C8 — floating damage numbers (D6):** `player_hit`/`mob_fell` spawn a
  rising number at the target (0.7 s, fades) parsed from S3's detail.
- **C9 — knock nudge:** the `player_hit` impact bloom now spawns offset tward
  the target (a "smeared" push), instead of dead center. Visual only.

## 5. Explicit non-goals

- No continuous collision, no animation root-motion server-side, no per-frame
  netcode. The server still authoritatively clamps movement deltas.
- No new weapon kinds, no art changes.
- Client-side only the *cosmetic* cadence repeats; the server still rejects
  real spam (own cd), so held cheats don't bypass tick fairness.

## 6. Risks

- Hitstop slows ALL virtual-time systems — movement dt is clamped (0.05 s max
  frame) so a 70 ms dip only briefly slows walking; acceptable and reversible.
- Hold-to-swing can hammer the reducer across the network when cd is ready;
  client side-estimates the weapon cd and the server still rejects, so worst
  case is a few rejected calls on cd boundary — cheap no-ops on cb.
