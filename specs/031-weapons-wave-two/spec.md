# Spec 031 — Second weapon wave: Iron Axe + Frost Sickle

> **Objective:** Extend the §4 weapon ladder from 6 to 8 tiers with two new
> craftable weapons, each carrying its own pre-made attack animation sheet,
> inventory icon, forge row, hotbar equip flow (already generic from 030),
> and per-tier damage/reach/cadence vs mobs.
>
> **Status:** built + published to `nestfeed` 2026-11-02.

## 1. Sprite survey (assets/models)

The pack's `Pre-made/<name>/` folders ship **four attack sheets per
character**, all on the 32×32 3-row grid we already slice:

| Sheet | Size | Frames/view | Used by |
|---|---|---|---|
| `Sword.png` | 320×96 | 10 | melee tiers (default strike) |
| `Bow and Arrow.png` | 224×96 | 7 | Bow |
| `Axe.png` | 192×96 | 6 | **Iron Axe (new)** |
| `Sickle.png` | 192×96 | 6 | **Frost Sickle (new)** |

Icons (`Icons/RPG icons/Weapons and Armor/<material>/`, 32×16 two-cell
sheets — left cell is the plain variant, right the outlined one; same
convention as the existing weapon icons, left 16×16 cells):

- `3. Iron/Axe.png` → **Iron Axe** icon
- `7. Frost/Sickle.png` → **Frost Sickle** icon
- Both crops verified via PIL render on the panel row bg before wiring
  (REV4 rule).

## 2. Design (§4 ladder extension)

| tier | name | dmg | cd | reach | anim | cost |
|---|---|---|---|---|---|---|
| 6 | Iron Axe | 26 | 2 s | 1 hex | Axe | 5 Metal Part + 8 Wood |
| 7 | Frost Sickle | 20 | 1 s | 2 hexes | Sickle | 6 Metal Part + 6 Bone + 10 Grass |

- **Iron Axe** = heavy lumber chop: hardest single hit in the set, paid
  for with a slow cadence.
- **Frost Sickle** = fiber-wrapped hook: kite-range (2 hex) sweeps on the
  fast 1 s cadence, dmg between Bone Spear and Stone Sword.
- No new ammo tier: the Bow stays the only ammo weapon (saturating check
  now reads `if w.ammo.is_some() { assert id == 5 }`).

## 3. Implementation

### 3.1 Core (`idlecore-core/src/player_combat.rs`)
- `Weapon` gains `anim: WeaponAnim` (new enum: `Sword/Bow/Axe/Sickle`).
- `WEAPONS` → `[Weapon; 8]`; new tiers 6/7 as tabled above; every existing
  tier carries an explicit `anim`.
- Tests updated: caps now at 7; axe/sickle stat assertions.

### 3.2 Server (published)
No schema/reducer changes — `player_attack`, `equip_weapon` and the
purchase grant flow are fully tier-generic; rebuilding + republishing
carries the new table. Item rows named `Iron Axe` / `Frost Sickle` are
granted on first purchase automatically.

### 3.3 Client
- `skins.rs`: loads + slices `Axe.png` / `Sickle.png` per character (7
  sheets/char now); `AttackAnim` enum mirrors `WeaponAnim`
  (`AttackAnim::of_weapon`), `PlayerSkins.attack: Option<AttackAnim>`;
  the one-shot clip plays the matched sheet at the same 0.07 s cadence.
- `net/td.rs`: both strike sites (click + KeyF) call
  `AttackAnim::of_weapon(player.weapon_def().anim)`.
- `world_floor.rs`: `ICON_IRON_AXE` / `ICON_FROST_SICKLE` crops +
  `PropTextures` fields.
- `inventory.rs`: hotbar `by_item` mappings; quick-slot equip flow from 030
  auto-matches the new names (no code change needed there).
- `net/forge.rs`: rows are data-driven off `WEAPONS` — new tiers appear
  automatically; icon-name test extended.

## 4. Verification
- Core 363 tests, server 87 tests, client 84 tests — all green.
- Server published to `nestfeed` (idle loop unchanged).
- Live client run: skins sliced (incl. Axe + Sickle sheets), no panics.

## 5. Open
- REMEMBER: uncommitted; `docker/precompress.ts` untouched.
- Sickle icon is intentionally pale (frost palette); if it reads poorly
  in-game, swap the crop to `6. Crimson/Sickle.png`.
