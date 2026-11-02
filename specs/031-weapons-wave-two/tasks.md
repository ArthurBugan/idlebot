# Spec 031 tasks — second weapon wave

- [x] T1 survey pack sprites: Axe.png + Sickle.png attack sheets (6f × 3
  views) confirmed per character; icon sheets verified with PIL crops
- [x] T2 core: `WeaponAnim` enum + `Weapon.anim`; WEAPONS[8] with Iron Axe
  (26 dmg / 2 s / 1 hex) and Frost Sickle (20 dmg / 1 s / 2 hex); tests
- [x] T3 server: tier-generic code paths unchanged; build + publish to
  nestfeed
- [x] T4 skins.rs: slice Axe/Sickle sheets; `AttackAnim` + `of_weapon`;
  one-shot clip plays per-tier sheet
- [x] T5 td.rs: click + KeyF strikes pick the equipped tier's clip
- [x] T6 icons: ICON_IRON_AXE + ICON_FROST_SICKLE crops, PropTextures
  fields, inventory `by_item` mappings
- [x] T7 forge rows auto-list new tiers; test vec updated
- [x] T8 verify: core 363 / server 87 / client 84 tests green; live run
  no panics, sheets sliced
