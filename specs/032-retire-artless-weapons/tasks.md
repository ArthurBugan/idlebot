# Spec 032 tasks — retire art-less weapons

- [x] T1 art audit: only Wooden Club + Bone Spear lacked dedicated attack
  sheets; everything else has sheet + verified icon
- [x] T2 core: WEAPONS → 6 sparse-id tiers (0,3,4,5,6,7); tests assert
  weapon(1)/weapon(2) are gone and retired ids resolve to None
- [x] T3 client: club/spear atlas keys, crops, PropTextures fields, hotbar
  by_item mappings removed; forge rows auto-drop; forge tests re-keyed
  by_id (bow = array [3])
- [x] T4 server rebuild + publish (stale equip_weapon(1|2) now refuses)
- [x] T5 clean-rebuild client tests (stale-incremental panic artifact);
  84 green; live run no panics
