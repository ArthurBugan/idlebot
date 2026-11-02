# Spec 030 tasks — click-to-strike polish

- [x] T1 minimap.rs: delete the world-floor LMB select / double-click
  teleport; minimap-only select stays; `cursor_on_minimap` helper exported
- [x] T2 td.rs `td_combat_input`: world click = strike (mob hit or ground
  facing), structure clicks skip to the 026 pick; minimap rect + panel +
  build-mode guards; facing update + attack clip on strike; KeyF triggers
  the clip too
- [x] T3 skins.rs: slice `Sword.png` (10f) + `Bow and Arrow.png` (7f)
  per character; one-shot `attack` clip overriding idle/walk/run;
  `start_attack`
- [x] T4 weapon-in-hand overlay 14 → 10 wu (offsets ~0.7×)
- [x] T5 world labels: level pip 14 → 4 ru, hover nameplate 12 → 4.5 ru
- [x] T6 structure pick: remove the stale `rows.is_changed()` gate
- [x] T7 verify: build clean, 84 tests green, live run no panics / sheets
  sliced
- [ ] T8 (open) remote players play the attack clip via `combat_event` fx
- [ ] T9 (open) fist-tier punch-only clip (no sword art)
- [x] T10 playtest round 1: remove the weapon-in-hand overlay sprite;
  hotbar weapon select sends `equip_weapon(tier)`; restyle name+level
  labels (pip 2.2 ru, two-line chip nameplate 2.6 ru); 84 tests green
