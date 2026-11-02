# Spec 032 — Retire art-less weapon tiers

> **Objective:** Remove from crafting/inventory every weapon tier without
> dedicated sprite + animation art in the asset pack.
>
> **Status:** built + published to `nestfeed` 2026-11-02.

## 1. Art audit (Pre-made + Icons)

| tier | attack sheet? | icon? | verdict |
|---|---|---|---|
| Fists | sword sheet shared | (hidden) | keep |
| Wooden Club (1) | none — reused Sword | re-crop of Wood Sword | **RETIRE** |
| Bone Spear (2) | none — reused Sword | Bones crop, no spear anim | **RETIRE** |
| Stone Sword (3) | Sword.png ✓ | 3. Iron Sword ✓ | keep |
| Iron Blade (4) | Sword.png ✓ | 5. Platinum Sword ✓ | keep |
| Bow (5) | Bow and Arrow.png ✓ | 1. Wood Bow ✓ | keep |
| Iron Axe (6) | Axe.png ✓ | 3. Iron Axe ✓ | keep |
| Frost Sickle (7) | Sickle.png ✓ | 7. Frost Sickle ✓ | keep |

## 2. Implementation
- Core: tiers 1/2 removed; `WEAPONS` = `[Weapon; 6]` with **sparse ids**
  (0, 3, 4, 5, 6, 7). Retired ids stay unallocated so any legacy
  `player.weapon` integer resolves to `weapon(id) == None` → `player_attack`
  refuses, a stale equip can never pick a wrong tier.
- Client: `ICON_WOOD_CLUB` / `ICON_BONE_SPEAR` atlas keys, crops,
  `PropTextures` fields and hotbar `by_item` mappings deleted; the forge
  loop auto-drops both rows. Forge tests re-keyed by `id` (array order ≠ id
  now): by_id(4) blade, by_id(5) bow.
- Server: no code change; republished so `equip_weapon(1|2)` now errors
  "Unknown weapon tier".

## 3. Verification
- Core 363 / server 87 / client 84 tests green.
- Live client run: no panics; forge lists Fists + 5 weapons.
- Debug note: the client test harness first reported
  `failed to initiate panic, error 5` — that was an ASSERT failure whose
  message was mangled by stale incremental artifacts. A full
  `cargo clean -p idlecore-client` + rebuild restored normal panic output;
  only then re-key the tests. If a panic message ever prints garbled after
  core changes, clean the client package first.
