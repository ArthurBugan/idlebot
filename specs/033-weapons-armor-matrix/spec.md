# Spec 033 — Full weapons & armor ladder (RPG-icons matrix) + armor defense

> **Objective:** Adopt *all* material tiers from
> `Icons/RPG icons/Weapons and Armor/` — Wood, Copper, Iron, Gold,
> Platinum, Crimson, Frost, Shadow, Fairy, Obsidian — as craftable
> weapons, plus armor pieces that grant flat damage reduction.
>
> **Status:** built + published 2026-11-02.

## 1. Art audit (what has animations / sprites)

Character pack (`Pre-made/<name>/`) attack sheets per character:

| strip | frames | kind |
|---|---|---|
| `Sword.png` | 10 | Sword |
| `Bow and Arrow.png` | 7 | Bow |
| `Axe.png` | 6 | Axe |
| `Sickle.png` | 6 | Sickle |
| `Pickaxe.png` | 6 | Pickaxe |
| `Hoe.png` | 6 | Hoe |
| `Shovel.png` | 5 | Shovel |

Staff/Fishing Rod/Watering Can have **no per-pre-made-character attack
strip** (the 23. Mage staff strip belongs to a different body set), so
Staff/Fishing Rod/Watering Can are NOT combat rows — every weapon row
added carries its own animation. Armor icons (Helmet/Chestplate/Leggings/
Boots) exist in every material folder; armor pieces are passive.

## 2. Core ladder (`player_combat.rs`)

- `WEA_KINDS` (7) × `WEA_MATERIALS` (10) → 70 weapons + Fists (id 0) =
  `weapons()` (71 entries; matrix id = `kind_i·10 + mat_i + 1`).
  Names read exactly like icons: "Gold Sword", "Obsidian Sickle"…
- Dmg ladders per kind (base + 2·mat), e.g. Sword 8…26, Axe 12…30,
  Bow 6…24 (ammo = 1 Arrow/shot, range 3, cd 2).
- Cost: kind primary ×2(m+1) + MetalPart (m+1); bows add Grass (m+1).
- `armors()` — 40 pieces; def = weight (helm 1, chest 2, legs 1,
  boots 1) × (m+1) → a full set spans 5 (Wood) … 25 (Obsidian).

## 3. Server

- `equip_weapon` reworked: ownership = a player_item row with the exact
  weapon name (bitmask `weapons_owned` retired for gating — the old
  bits still grant a free legacy repair); purchase consumes all cost
  pairs once.
- New reducer **`craft_armor(id)`**: pays (2·(m+1) Bone + weight·(m+1)
  Stone), grants the permanent piece; re-craft refused.
- **Defense**: `player_defense` = Σ def over owned pieces;
  `apply_player_damage` does `dmg - def` (floor 1 — never full immunity).

## 4. Client

- `skins.rs`: Pickaxe/Hoe/Shovel strips sliced per character (10 sheets);
  `AttackAnim` + `of_weapon` extended; every attack plays its own clip at
  the 0.07 s cadence.
- `slice.rs`: `request_dyn`/String-path support; `world_floor.rs` enqueues
  all 110 icon crops (7 weapons + 4 armor × 10 materials; left 16×16
  plain cells crop-verified via PIL render), then fills
  `PropTextures.weapon_icons` / `armor_icons` keyed by item name.
- `inventory.rs`: hotbar icons resolve every matrix weapon + armor piece;
  quick-slot weapon select keeps working via exact names.
- `net/forge.rs`: the Forge tab now lists the 71 weapon rows + an Armor
  section (40 rows) inside a **scroll container** (`OverflowAxis::Scroll`
  + ScrollPosition; wheel reader scrolls while open), then the Arrow row.
  Row states read ownership from live stack counts; armor rows send
  `craft_armor`. Status line shows equipped weapon + total ⛨ defense.

## 5. Verification

- Core 364 / server 87 / client 84 tests green.
- Server published to `nestfeed`; bindings regenerated (+`craft_armor`).
- Live run: skins sliced (10 sheets/char), "Prop/icon textures built"
  (all 110 matrix icons landed), no panics.
