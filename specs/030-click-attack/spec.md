# Spec 030 — Click-to-strike + polish pass (weapon scale, labels, select fix)

> **Objective:** Left-click stops being a teleport and becomes the
> player's primary attack — striking **whatever is under the cursor** with
> the equipped weapon, played through the character sheets' own attack
> animations. Also: weapon-in-hand + world-building labels shrink, and the
> Spec 026 structure-selection click works again.
>
> **Status:** built 2026-11-02.

## 1. Problem

- Left-click (single = select, double = fire `teleport_player`) on the
  world floor collided with combat: you cannot click to hit what you want
  to hit, and harmless-looking clicks teleport you across the map.
- The armed weapon never shows motion: attacks play no animation, so the
  strike reads as nothing happening.
- The weapon-in-hand overlay is oversized relative to the 32×32 figure,
  and the world-space labels on buildings (level pip, hover nameplate)
  render at 12–14 ru against a ~17 wu tile — several times the character.
- Clicking a building no longer opens its upgrade/stats panel: the
  selection pick required `TdStructRows.is_changed()`, and that resource
  only flips `changed` when the replicated rows actually differ — once
  replication settled, every click fell on a stale `false`.

## 2. Changes (client-only)

### 2.1 Click = strike (net/td.rs `td_combat_input`)
- LMB on the world (not a widget, not the minimap rect, no command panel
  open, build mode unarmed) now attacks:
  - a mob under the cursor (unchanged 027 footprints) → strike toward it;
  - empty ground → strike the facing that points at the click;
  - a structure under the cursor → **no attack**; the 026
    selection pick owns the click.
- The strike orients the player (`PlayerOrientation.facing_angle`) so the
  attack clip and the weapon-in-hand overlay face the blow.
- KeyF keeps its facing-arc strike; both paths fire the attack clip.

### 2.2 Attack animation (skins.rs)
- The pre-made folders ship per-character attack sheets: `Sword.png`
  (10 columns × 3 views) and `Bow and Arrow.png` (7 × 3) — sliced with
  the same pipeline as Idle/Walk/Run (down/up/side rows mirrored
  westward). Cropped verification was done against the rendered sheets.
- `IsoCharacter` gains `sword` + `bow` frame sets; `PlayerSkins` gains a
  one-shot `attack: Option<bool>` clip (`Some(bow)`), advanced at
  `ATTACK_FRAME_TIME = 0.07 s` (≈ 0.70 s sword, 0.49 s bow), not looping:
  the last column clears back to idle/walk/run next frame.
- `skins::start_attack(skins, bow)` is called on every sent
  `player_attack` (click or KeyF). The bow clip plays exactly when the
  equipped weapon has ammo (`WEAPONS[tier].ammo` is `Some`).

### 2.3 Teleport removal (minimap.rs)
- The world-floor left-click select + double-click teleport branch is
  deleted. The minimap keeps its own select/teleport flow (select by
  minimap click, double-click on the minimap fires, the HUD Teleport
  button still consumes `selected_hex`); the world click simply leaves
  the minimap alone.

### 2.4 Scale fixes
- Weapon-in-hand overlay: 14 → **10** wu; side offsets 7/5/7 → 5/3.5/5.
- Level pip above structures: 14 → **4.0** ru. Hover nameplate: 12 →
  **4.5** ru (shadow offsets scaled to match).

### 2.5 Selection fix
- `td_structure_pick` drops the `rows.is_changed()` gate; the widget /
  build-mode guards already keep spurious clicks out.

## 3. Out of scope
- Remote players playing the attack clip (only the local skin animates).
- Punch-only (fist-tier) clip — the sword sheet plays for all melee tiers;
  the weapon-in-hand overlay differentiates the tool.

## 4. Verification
- Client build clean; 84 tests green (`--test-threads=1`).
- Live run: skins sliced (incl. attack sheets) with no asset panics; no
  runtime panics in a 12 s session.

## 5. Playtest round 1 (2026-11-02)
Player feedback: weapon swap did nothing, an oversized floating sprite sat
near the player, and building labels were still huge.

- **Weapon-in-hand overlay removed** (`plugins/player.rs`): the 10 wu
  icon sprite read as a giant floating sprite beside the character.
  `WeaponInHand` / `WeaponInHandRes` / `update_weapon_in_hand` deleted;
  the inventory icons remain for the panel/hotbar.
- **Hotbar weapon selection now equips** (`inventory.rs
  `auto_equip_weapon_on_select`): selecting a WEAPONS item in the hotbar
  fires `equip_weapon(tier)` once per selection change (skipped when the
  tier is already equipped). The server tier drives `weapon_def()` →
  damage/reach/cooldown, the bow-vs-sword attack clip, and arrow ammo.
  Non-weapon items leave the current weapon in hand.
- **Building labels restyled** (`net/td.rs`): level pip 4.0 → 2.2 ru with
  a soft alpha; hover nameplate rebuilt as a two-line block (name · level,
  then HP) at 2.6 ru on a dark chip (`TextBackgroundColor`) in warm
  parchment ink, lifted to clear the roof.

