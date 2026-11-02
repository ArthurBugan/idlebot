# Spec 033 tasks — weapons & armor matrix

- [x] T1 art audit: Sword/Bow/Axe/Sickle/Pickaxe/Hoe/Shovel have real
  attack strips; Staff/Fishing/Watering excluded (no per-premade anim);
  armor icons exist in all 10 material folders
- [x] T2 core: WEA_MATERIALS × WEA_KINDS matrix (70 weapons + Fists),
  armors() 40 pieces with def ladder; tests
- [x] T3 server: equip_weapon by item-row ownership + legacy bitmask
  repair; craft_armor reducer; player_defense + apply_playerDamage floor-1
  reduction; publish + binding regen
- [x] T4 skins: 10 sheets/char (incl. Pickaxe/Hoe/Shovel), AttackAnim
  variants, of_weapon mapping
- [x] T5 icons: request_dyn string paths, 110 crops registered + PIL
  verified, PropTextures weapon_icons/armor_icons, inventory by_item loop
- [x] T6 forge: scroll container (OverflowAxis::Scroll + wheel reader),
  71 weapon rows, Armor section + craft_armor clicks, stack-count row
  states, ⛨ status line
- [x] T7 verify: core 364 / server 87 / client 84 green; live run clean,
  all icons built
