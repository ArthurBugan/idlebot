//! Spec 027 — player combat: weapon tiers, HP curve, 8-way heading arcs
//! and the roam-time knobs. Constitution-bound: server-authoritative on
//! the 1s tick, integer-only math, no per-frame combat simulation.

use crate::defense_config::{
    ITEM_ARROW, ITEM_BONE, ITEM_GRASS, ITEM_METAL_PART, ITEM_SLIME_GEL, ITEM_STONE, ITEM_WOOD,
};

/// Spec 033 — the RPG-icons `Weapons and Armor` ladder: every material
/// folder row (Wood … Obsidian) × every weapon type with a real attack
/// animation in the character pack (Sword/Axe/Pickaxe/Hoe/Shovel/Sickle 6–10
/// frame strips + Bow 7). Armor pieces from the same folders grant flat
/// reduction on every hit (no per-slot equipping — owning a piece wears it).
use std::sync::OnceLock;

/// Max HP = 80 + 10·level (idle level reused — combat scales with the same
/// progression as everything else, Spec 027 §6).
pub fn player_max_hp(level: u32) -> u32 {
    80u32.saturating_add(10u32.saturating_mul(level))
}

/// Out-of-combat regen: +2 HP per 1s tick (`hostile_until` must read past).
pub const PLAYER_REGEN_PER_TICK: u32 = 2;

/// An aggroed mob hunts the player for this many seconds after being hit.
pub const MOB_AGGRO_SECS: u64 = 20;

/// Roaming Slime specifics (§7): pack sizes 2–4, at most 6 live packs,
/// 300 s lifetime, a spawn attempt every 60 s, any capital ≥ 3 hexes away.
pub const ROAMER_XP: u64 = 50;
pub const ROAMER_PACK_MIN: u32 = 2;
pub const ROAMER_PACK_MAX: u32 = 4;
pub const ROAMER_PACK_CAP: u32 = 6;
pub const ROAMER_LIFETIME_SECS: u64 = 300;
pub const ROAMER_SPAWN_PERIOD_SECS: u64 = 60;
pub const ROAMER_MIN_CAPITAL_DIST: i32 = 3;
/// "≤ 20 hexes from the spawning node cluster": roamers live near players —
/// within this distance of some online player's hex (gathering stays local).
pub const ROAMER_MAX_PLAYER_DIST: i32 = 20;
/// Wander cadence: one step per `step_interval × 4` for roamers.
pub const ROAMER_WANDER_CADENCE: u32 = 4;
/// Contact aggro: walking INTO a roamer's hex (or its melee reach) turns it.
pub const ROAMER_CONTACT_DIST: i32 = 1;

/// Dash (§5.2): 6 s cooldown; the 0.4 s burst rounds up to ONE 1s tick on
/// the server (integer-second table) — the client trims the tail locally.
pub const DASH_CD_SECS: u64 = 2;
pub const DASH_TICKS: u64 = 1;
pub const DASH_SPEED_MULT: u32 = 3;
/// Client-side exact burst (real 0.4 s, mirrors the spec number).
pub const DASH_DURATION_SECS: f32 = 0.4;

/// Player XP per roamer kill lands directly on the killer (§7).
pub const PLAYER_KILL_XP_ROAMER: u64 = ROAMER_XP;
/// Roamers carry Bone at this chance (the starter's Bone Spear path).
pub const ROAMER_BONE_PCT: u64 = 25;
/// Death penalty: none besides the 10 s corpse timer (no drop, no XP loss).
pub const DEATH_RESPAWN_SECS: u64 = 10;
/// Fresh-spawn grace on capital respawn; drops the moment the player attacks.
pub const RESPAWN_GRACE_SECS: u64 = 60;

/// One craftable weapon tier (§4 + Spec 033 matrix): `cost` is paid ONCE
/// at first equip — weapon items are permanent purchases, one equipped at
/// a time.
#[derive(Debug, Clone)]
pub struct Weapon {
    /// Tier id: 0 = Fists; the matrix packs `kind × material` below.
    pub id: u8,
    /// Display name ("Gold Sword") — the exact item row string granted on
    /// first purchase; inventory icons key on it.
    pub name: String,
    /// (item, qty) pairs; Fists cost nothing.
    pub cost: Vec<(&'static str, u32)>,
    /// Per-attack burst the reducer applies to the chosen target.
    pub dmg: u32,
    /// Seconds between attacks (server-enforced via `attack_cd_until`).
    pub cd_secs: u64,
    /// Reach in lattice hexes; 1 = melee adjacency, 3 = bow.
    pub range_hexes: i32,
    /// §14 addendum: ammunition consumed from the stacks per shot (bows).
    /// `None` for melee — the server refuses the shot when short.
    pub ammo: Option<&'static str>,
    /// Which pre-made attack sheet the client plays for this tier.
    pub anim: WeaponAnim,
}

/// Attack animation the client plays for the equipped tier. Each variant
/// maps to a per-character pre-made attack sheet from the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponAnim {
    /// `Sword.png` slash arc (10 frames).
    Sword,
    /// `Bow and Arrow.png` draw + loose (7 frames).
    Bow,
    /// `Axe.png` chop (6 frames) — heavy two-handed strikes.
    Axe,
    /// `Sickle.png` hook swing (6 frames) — mid-reach sweeps.
    Sickle,
    /// `Pickaxe.png` drill stab (6 frames).
    Pickaxe,
    /// `Hoe.png` sweeping cut (6 frames).
    Hoe,
    /// `Shovel.png` blunt lob (5 frames).
    Shovel,
    /// REV (playtest): `Throwing items.png` arm-punch (5 frames) — Fists no
    /// longer play the sword slash.
    Fists,
}

/// Weapon row of the RPG-icons ladder: shader-friendly stat knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponKindDef {
    /// Display ("Gold Sword" → `"Gold {name}"`).
    pub name: &'static str,
    pub anim: WeaponAnim,
    /// dmg = base + step × material index.
    pub dmg_base: u32,
    pub dmg_step: u32,
    pub cd_secs: u64,
    pub range_hexes: i32,
    /// Per-buy primary material (paired with Bone/MetalPart per tier).
    pub primary: &'static str,
}

/// Attack-animatable weapon rows (Featured all have pre-made sheets).
pub const WEA_KINDS: [WeaponKindDef; 7] = [
    WeaponKindDef { name: "Sword", anim: WeaponAnim::Sword, dmg_base: 8, dmg_step: 2, cd_secs: 1, range_hexes: 1, primary: ITEM_BONE },
    WeaponKindDef { name: "Axe", anim: WeaponAnim::Axe, dmg_base: 12, dmg_step: 2, cd_secs: 2, range_hexes: 1, primary: ITEM_WOOD },
    WeaponKindDef { name: "Pickaxe", anim: WeaponAnim::Pickaxe, dmg_base: 9, dmg_step: 2, cd_secs: 2, range_hexes: 1, primary: ITEM_STONE },
    WeaponKindDef { name: "Hoe", anim: WeaponAnim::Hoe, dmg_base: 7, dmg_step: 2, cd_secs: 1, range_hexes: 2, primary: ITEM_WOOD },
    WeaponKindDef { name: "Shovel", anim: WeaponAnim::Shovel, dmg_base: 9, dmg_step: 2, cd_secs: 2, range_hexes: 1, primary: ITEM_STONE },
    WeaponKindDef { name: "Sickle", anim: WeaponAnim::Sickle, dmg_base: 8, dmg_step: 2, cd_secs: 1, range_hexes: 2, primary: ITEM_GRASS },
    WeaponKindDef { name: "Bow", anim: WeaponAnim::Bow, dmg_base: 6, dmg_step: 2, cd_secs: 2, range_hexes: 3, primary: ITEM_WOOD },
];

/// Material rows: (display tier, icon folder under `RPG icons/Weapons and
/// Armor`). The folder names keep the pack's original spellings.
pub const WEA_MATERIALS: [(&str, &str); 10] = [
    ("Wood", "1. Wood"),
    ("Copper", "2. Cooper"),
    ("Iron", "3. Iron"),
    ("Gold", "4. Gold"),
    ("Platinum", "5. Platinum"),
    ("Crimson", "6. Crimson"),
    ("Frost", "7. Frost"),
    ("Shadow", "8. Shadow"),
    ("Fairy", "9. Fairy"),
    ("Obsidian", "9. Obsidian"),
];

/// Armor pieces: (display, per-piece defense weight). Owning a piece
/// grants its weight × material scalar as flat damage reduction.
pub const ARMOR_PIECES: [(&str, u32); 4] =
    [("Helmet", 1), ("Chestplate", 2), ("Leggings", 1), ("Boots", 1)];

/// Arrow recipe cost stays put (§14.2).
pub const ARROW_COST: &[(&str, u32)] = &[(ITEM_WOOD, 2), (ITEM_STONE, 1)];
/// `craft_arrows` batch cap (reducer arity bound; UI crafts ×10 at once).
pub const ARROW_BATCH_CAP: u32 = 32;

/// Build the full ladder once: Fists (id 0) + 7 kinds × 10 materials.
fn build_weapons() -> Vec<Weapon> {
    let mut out = vec![Weapon {
        id: 0,
        name: "Fists".to_string(),
        cost: vec![],
        dmg: 4,
        cd_secs: 1,
        range_hexes: 1,
        ammo: None,
        anim: WeaponAnim::Fists,
    }];
    for (kind_i, kind) in WEA_KINDS.iter().enumerate() {
        for (mat_i, (mat_name, _dir)) in WEA_MATERIALS.iter().enumerate() {
            // REV (drop economy): the starter's Wood tiers (mat 0) craft from
            // gathered materials as before — every upgrade past Wood is
            // paid in MONSTER DROPS, Enemy loot drives base weapon
            // upgrades: raiders drop SlimeGel / Bone / MetalPart, so waves
            // themselves fund the escalation. Costs scale with the tier.
            let mut cost: Vec<(&'static str, u32)> = if mat_i == 0 {
                vec![
                    (kind.primary, 2),
                    (ITEM_METAL_PART, 1),
                ]
            } else {
                let t = mat_i as u32;
                vec![
                    (ITEM_METAL_PART, t + 1),
                    (ITEM_BONE, t),
                    (ITEM_SLIME_GEL, 2 * t),
                ]
            };
            let mut ammo = None;
            if kind.anim == WeaponAnim::Bow {
                ammo = Some(ITEM_ARROW);
                cost.push((ITEM_GRASS, mat_i as u32 + 1));
            }
            out.push(Weapon {
                id: (kind_i * 10 + mat_i + 1) as u8,
                name: format!("{mat_name} {}", kind.name),
                cost,
                dmg: kind.dmg_base + kind.dmg_step * mat_i as u32,
                cd_secs: kind.cd_secs,
                range_hexes: kind.range_hexes,
                ammo,
                anim: kind.anim,
            });
        }
    }
    out
}

static WEAPONS_LOCK: OnceLock<Vec<Weapon>> = OnceLock::new();

/// The full weapon ladder (Spec 033): Fists + every material × kind combo.
pub fn weapons() -> &'static [Weapon] {
    WEAPONS_LOCK.get_or_init(build_weapons)
}

static ARMOR_LOCK: OnceLock<Vec<ArmorDef>> = OnceLock::new();

/// One armor piece (Spec 033): a permanent item; OWNING it reduces every
/// hit by `def` (no per-slot equipping needed).
#[derive(Debug, Clone)]
pub struct ArmorDef {
    pub id: u8,
    pub name: String,
    pub cost: Vec<(&'static str, u32)>,
    pub def: u32,
}

fn build_armors() -> Vec<ArmorDef> {
    let mut out = Vec::with_capacity(40);
    for (mat_i, (mat_name, _dir)) in WEA_MATERIALS.iter().enumerate() {
        for (piece_i, (piece, weight)) in ARMOR_PIECES.iter().enumerate() {
            out.push(ArmorDef {
                id: (mat_i * 4 + piece_i) as u8,
                name: format!("{mat_name} {piece}"),
                cost: vec![
                    (ITEM_BONE, 2 * (mat_i as u32 + 1)),
                    (ITEM_STONE, weight * (mat_i as u32 + 1)),
                ],
                def: weight * (mat_i as u32 + 1),
            });
        }
    }
    out
}

pub fn armors() -> &'static [ArmorDef] {
    ARMOR_LOCK.get_or_init(build_armors)
}

pub fn armor(id: u8) -> Option<&'static ArmorDef> {
    armors().get(id as usize)
}

pub const WEAPONS_MATRIX_SIZE: usize = 70;

pub fn weapon(id: u8) -> Option<&'static Weapon> {
    weapons().iter().find(|w| w.id == id)
}

/// 8-way headings (§5.1): client facing angle r → server integer direction.
/// Direction d points at d·45° CCW from east (+X), matching the client's
/// yaw convention; the fixed vectors are ×10 fixed-point for all-integer
/// arc math. Index layout: 0 E, 1 NE, 2 N, 3 NW, 4 W, 5 SW, 6 S, 7 SE.
pub const DIR_VECS: [(i32, i32); 8] = [
    (10, 0),
    (7, 7),
    (0, 10),
    (-7, 7),
    (-10, 0),
    (-7, -7),
    (0, -10),
    (7, -7),
];

/// Client-side: facing angle (radians, 0 = east, CCW positive) → heading id.
/// The client sends this; the server maps it back to `DIR_VECS`.
pub fn heading_index(angle_rad: f32) -> i32 {
    const STEP: f32 = std::f32::consts::PI / 4.0;
    let two_pi = std::f32::consts::TAU;
    let a = angle_rad.rem_euclid(two_pi);
    let idx = ((a / STEP).round() as i64).rem_euclid(8);
    idx as i32
}

/// Integer 120° arc test (§5.1): does the vector toward the target fall
/// inside the arc centered on heading `dir`? Half-angle 60° ⇒ `dot ≥ |v|·|d|·
/// cos(60°)`. Fixed-point ×100 world vectors; all i64, no floats.
pub fn heading_allows(vx: f32, vy: f32, dir: i32) -> bool {
    let vec: (i64, i64) = ((vx * 100.0) as i64, (vy * 100.0) as i64);
    let len2 = vec.0 * vec.0 + vec.1 * vec.1;
    if len2 == 0 {
        return false;
    }
    let (dx, dy) = DIR_VECS[(((dir % 8) + 8) % 8) as usize];
    let dlen2 = (dx * dx + dy * dy) as i64;
    let dot = vec.0 * dx as i64 + vec.1 * dy as i64;
    if dot <= 0 {
        return false; // behind or perpendicular
    }
    // dot ≥ ½·|v|·|d|  ⇔  dot² ≥ ¼·len2·dlen2.
    (dot as u64).pow(2) >= (len2 as u64).saturating_mul(dlen2 as u64) / 4
}

/// One wander step for roamers (§7): deterministic pseudo-random neighbor
/// seeded by (mob_id, now, hex) — no RNG, no allocator, pure.
pub fn roamer_wander_dir(mob_id: u64, now: u64, q: i32, r: i32) -> usize {
    let seed = mob_id
        .wrapping_mul(2654435761)
        ^ now.wrapping_mul(977)
        ^ (q as u64).wrapping_mul(7)
        ^ (r as u64).wrapping_mul(13);
    (seed % 6) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_hp_grows_ten_per_level() {
        assert_eq!(player_max_hp(1), 90);
        assert_eq!(player_max_hp(0), 80, "L0 fallback floors at base");
        assert_eq!(player_max_hp(5), 130);
        assert_eq!(player_max_hp(u32::MAX), u32::MAX, "saturating at top");
    }

    #[test]
    fn weapon_table_matches_the_spec_sheet() {
        assert_eq!(weapon(0).unwrap().name, "Fists");
        assert_eq!(weapon(0).unwrap().dmg, 4);
        // Spec 032: the club/spear idea is long gone; the matrix never
        // produced a standalone club or spear row.
        let fist = weapon(0).unwrap();
        assert_eq!(fist.cost.len(), 0);
        // §14 addendum: the bow is the one ammo weapon, 3-hex reach, 2 s cadence.
        let bow = weapons()
            .iter()
            .find(|w| w.name == "Wood Bow")
            .expect("bow tier");
        assert_eq!(bow.dmg, 6);
        assert_eq!(bow.cd_secs, 2);
        assert_eq!(bow.range_hexes, 3);
        assert_eq!(
            bow.cost,
            vec![(ITEM_WOOD, 2), (ITEM_METAL_PART, 1), (ITEM_GRASS, 1)]
        );
        assert_eq!(bow.ammo, Some(ITEM_ARROW));
        // Melee tiers never consume ammo; the top of the ladder does.
        for w in weapons() {
            assert!(w.cd_secs >= 1);
            assert!(w.range_hexes >= 1);
            if w.ammo.is_some() {
                assert_eq!(w.anim, WeaponAnim::Bow, "only bow tiers carry ammo");
            }
        }
        // Spec 031: the axe is the heavy hitter on a slow cadence; the
        // sickle hooks at kite range on the fast pace.
        let axe = weapons().iter().find(|w| w.name == "Obsidian Axe").unwrap();
        assert_eq!(axe.anim, WeaponAnim::Axe);
        assert_eq!(axe.dmg, 12 + 2 * 9);
        assert_eq!(axe.cd_secs, 2);
        let sickle = weapons().iter().find(|w| w.name == "Gold Sickle").unwrap();
        assert_eq!(sickle.anim, WeaponAnim::Sickle);
        assert_eq!(sickle.range_hexes, 2);
        assert_eq!(sickle.dmg, 8 + 2 * 3);
        // Matrix ids pack kind × material without collisions.
        assert_eq!(weapons().len(), 71);
        assert_eq!(
            WEAPONS_MATRIX_SIZE,
            7 * WEA_MATERIALS.len()
        );
    }

    #[test]
    fn armor_table_grants_flat_defense() {
        assert_eq!(armors().len(), 40);
        // Wood chest = 2, helm/legs/boots = 1 → a 5-point wood set.
        let wood_set: u32 = armors()[..4].iter().map(|a| a.def).sum();
        assert_eq!(wood_set, 5);
        // Obsidian chestplate = 2 × 10 mitigation alone.
        let obsi_chest = armors().iter().find(|a| a.name == "Obsidian Chestplate").unwrap();
        assert_eq!(obsi_chest.def, 20);
        // Every piece costs something and carries a stable unique name.
        let names: std::collections::HashSet<&String> = armors().iter().map(|a| &a.name).collect();
        assert_eq!(names.len(), 40);
        for a in armors() {
            assert!(!a.cost.is_empty());
            assert!(a.def >= 1);
        }
    }

    #[test]
    fn arrow_recipes_and_batches_are_bounded() {
        assert_eq!(ARROW_COST, &[(ITEM_WOOD, 2), (ITEM_STONE, 1)]);
        assert!(ARROW_BATCH_CAP >= 10, "the UI crafts ×10 per press");
        // The cap keeps the reducer's loop short (const-bounded, no DoS arith).
        let (worst_wood, worst_stone) = (
            ARROW_COST[0].1 * ARROW_BATCH_CAP,
            ARROW_COST[1].1 * ARROW_BATCH_CAP,
        );
        assert_eq!(worst_wood, 64);
        assert_eq!(worst_stone, 32);
    }

    #[test]
    fn heading_arc_is_a_120_cone() {
        // East heading (0): a due-east vector aims dead center.
        assert!(heading_allows(10.0, 0.0, 0));
        // 45° off-axis is inside a 120° arc (half-angle 60°).
        assert!(heading_allows(1.0, 1.0, 0), "NE is 45° < 60° off east");
        // 90° off-axis is outside.
        assert!(!heading_allows(0.0, 10.0, 0), "north is 90° off east");
        assert!(!heading_allows(-10.0, 0.0, 0), "behind is out");
        // 68° off-axis sits just past the edge.
        assert!(!heading_allows(1.0, 2.5, 0), "68° > 60° half-angle");
        // Degenerate zero vector matches nothing.
        assert!(!heading_allows(0.0, 0.0, 0));
    }

    #[test]
    fn heading_index_wraps_clockwise_and_back() {
        assert_eq!(heading_index(0.0), 0);
        assert_eq!(heading_index(std::f32::consts::PI / 4.0), 1);
        assert_eq!(heading_index(std::f32::consts::FRAC_PI_2), 2);
        assert_eq!(heading_index(-std::f32::consts::FRAC_PI_2), 6);
        assert_eq!(heading_index(-3.0 * std::f32::consts::PI / 4.0), 5);
        assert_eq!(heading_index(9.0 * std::f32::consts::PI), 4, "full spins wrap");
    }

    #[test]
    fn roamer_wander_is_deterministic_and_bounded() {
        for mob_id in [1u64, 42, 12345] {
            for now in [10u64, 999] {
                let d = roamer_wander_dir(mob_id, now, 3, -2);
                assert!(d < 6, "a hex has 6 neighbors");
                assert_eq!(
                    d,
                    roamer_wander_dir(mob_id, now, 3, -2),
                    "same inputs same step"
                );
            }
        }
        // Different calls make different walks often enough to not stall.
        let set: std::collections::HashSet<i64> =
            (0..64u64).map(|t| roamer_wander_dir(7, t, 0, 0) as i64).collect();
        assert!(set.len() >= 2, "64 ticks should remap at least two dirs");
    }
}
