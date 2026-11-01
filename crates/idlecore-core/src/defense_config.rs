//! Tower-defense layer configuration (Spec 023) — shared by server and client
//!
//! Single source of truth for the raid economy: mob kinds, defense structures,
//! capital stats, land-claim rules and wave pacing. The server enforces all of
//! it; the client only renders what these numbers predict.

/// Inventory item names used by the tower-defense layer.
pub const ITEM_CAPITAL_CORE: &str = "CapitalCore";
pub const ITEM_METAL_PART: &str = "MetalPart";
pub const ITEM_BONE: &str = "Bone";
pub const ITEM_BONE_BARRICADE: &str = "BoneBarricade";
pub const ITEM_ARROW_TOWER: &str = "ArrowTower";
pub const ITEM_BALLISTA: &str = "Ballista";
/// Pre-existing items the defense costs spend (mirrors server `types.rs`).
pub const ITEM_WOOD: &str = "Wood";
pub const ITEM_STONE: &str = "Stone";

// ---------------------------------------------------------------------------
// Capital city
// ---------------------------------------------------------------------------

/// Capital HP at level 1; each level adds `CAPITAL_HP_PER_LEVEL`.
pub const CAPITAL_HP_BASE: u32 = 100;
pub const CAPITAL_HP_PER_LEVEL: u32 = 25;
/// Minimum hex distance between any two placed capitals.
pub const MIN_CAPITAL_SPACING: i32 = 8;
/// Claimed hexes around the capital at level 1; each level adds one.
pub const CLAIM_RADIUS_BASE: i32 = 2;
/// Claim radius hard cap (prevents infinite territory).
pub const CLAIM_RADIUS_MAX: i32 = 5;
/// Land-claim cost per hex (consumed from the inventory).
pub const CLAIM_WOOD_COST: u64 = 5;
pub const CLAIM_STONE_COST: u64 = 3;
/// Sect 022 action-cooldown reuse: one claim every 5s is fine, but keep the
/// claim cadence a little stricter so territory spreads by walking, not spam.
pub const CLAIM_COOLDOWN_SECS: u64 = 5;
/// No raids before this much time has passed since capital placement.
pub const GRACE_FIRST_RAID_SECS: u64 = 30 * 60;
/// Reset raid immunity window after the capital is destroyed and re-placed.
pub const GRACE_REBUILD_SECS: u64 = 24 * 3600;
/// Capital XP granted per mob killed (levels use the standard 100·L² curve).
pub const CAPITAL_XP_PER_MOB: u64 = 10;

/// Max claim radius for a capital level (base 2, +1/level, capped at 5).
pub fn claim_radius(level: u32) -> i32 {
    CLAIM_RADIUS_BASE
        .saturating_add(level.saturating_sub(1) as i32)
        .min(CLAIM_RADIUS_MAX)
}

/// Capital max HP for its level (never below the base).
pub fn capital_max_hp(level: u32) -> u32 {
    CAPITAL_HP_BASE + CAPITAL_HP_PER_LEVEL.saturating_mul(level.saturating_sub(1))
}

/// Capital level thresholds on the standard progression curve (100·L²).
pub fn capital_xp_for_level(level: u32) -> u64 {
    100 * u64::from(level.saturating_mul(level))
}

/// Level for a total capital XP amount (curtailed at the curve's front edge).
pub fn capital_level_from_xp(total_xp: u64) -> u32 {
    let mut level = 1u32;
    while capital_xp_for_level(level + 1) <= total_xp {
        level += 1;
    }
    level
}

// ---------------------------------------------------------------------------
// Mobs
// ---------------------------------------------------------------------------

/// One raid's mob budget.
pub const WAVE_MAX_MOBS: u32 = 12;
/// Minimum minutes between waves on the same capital.
pub const WAVE_REATTACK_SECS: u64 = 10 * 60;
/// A raid forcefields no longer than this; survivors despawn without drops.
pub const WAVE_TIMEOUT_SECS: u64 = 15 * 60;

/// A mob archetype (Spec 023 §3.5). All values integer; combat runs on 1s ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobKind {
    pub name: &'static str,
    pub hp: u32,
    /// Damage dealt per 1s tick (melee to structures/capital).
    pub dps: u32,
    /// Seconds between one-hex step marching toward the capital.
    pub step_interval_secs: u32,
    /// Minimum capital level before this kind joins waves.
    pub min_capital_level: u32,
    /// (drop item, chance %) — credited to defending player on kill.
    pub drop_item: &'static str,
    /// Drop quantity is `drop_qty` when the chance-% roll hits.
    pub drop_chance_pct: u32,
    pub drop_qty: u32,
}

pub const MOB_KINDS: [MobKind; 3] = [
    MobKind {
        name: "Scavenger",
        hp: 30,
        dps: 4,
        step_interval_secs: 2,
        min_capital_level: 1,
        drop_item: ITEM_BONE,
        drop_chance_pct: 40,
        drop_qty: 1,
    },
    MobKind {
        name: "BoneWalker",
        hp: 80,
        dps: 8,
        step_interval_secs: 3,
        min_capital_level: 2,
        drop_item: ITEM_BONE,
        drop_chance_pct: 90,
        drop_qty: 2,
    },
    MobKind {
        name: "RustHusk",
        hp: 140,
        dps: 12,
        step_interval_secs: 4,
        min_capital_level: 3,
        drop_item: ITEM_METAL_PART,
        drop_chance_pct: 50,
        drop_qty: 1,
    },
];

/// Look up a mob kind by name.
pub fn mob_kind(name: &str) -> Option<&'static MobKind> {
    MOB_KINDS.iter().find(|k| k.name == name)
}

/// Mob kinds eligible for a capital level (unlock order = table order).
pub fn mob_kinds_for_level(level: u32) -> Vec<&'static MobKind> {
    MOB_KINDS
        .iter()
        .filter(|k| k.min_capital_level <= level)
        .collect()
}

/// Wave size from pressure: territory size and capital level
/// (Spec 023 §3.5), capped at [`WAVE_MAX_MOBS`].
pub fn wave_size(claimed_hexes: usize, capital_level: u32) -> usize {
    let base = 2.0 + claimed_hexes as f32 / 2.0;
    let scaled = base * 1.15_f32.powi(capital_level.saturating_sub(1) as i32);
    (scaled.ceil() as usize).clamp(2, WAVE_MAX_MOBS as usize)
}

/// Deterministic composition for a wave of `size` mobs at a capital level:
/// cycles available kinds so higher levels naturally field the heavier ones.
pub fn wave_composition(size: usize, capital_level: u32) -> Vec<&'static str> {
    let kinds = mob_kinds_for_level(capital_level);
    (0..size).map(|i| kinds[i % kinds.len()].name).collect()
}

// ---------------------------------------------------------------------------
// Defense structures
// ---------------------------------------------------------------------------

/// One tower/wall archetype (Spec 023 §3.4). Costs use inventory item names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefenseKind {
    pub name: &'static str,
    /// Inventory expense to build, as (item, qty) pairs, listed once.
    pub cost: &'static [(&'static str, u64)],
    pub hp: u32,
    /// Attack reach in hex distance from the structure's hex (0 = no attack).
    pub range_hexes: u32,
    /// Damage dealt per 1s tick to mobs in range.
    pub dps: u32,
    /// Walls are melee targets mobs must chew through to pass.
    pub is_wall: bool,
}

pub const DEFENSE_KINDS: [DefenseKind; 3] = [
    DefenseKind {
        name: ITEM_BONE_BARRICADE,
        cost: &[(ITEM_BONE, 6)],
        hp: 60,
        range_hexes: 0,
        dps: 0,
        is_wall: true,
    },
    DefenseKind {
        name: ITEM_ARROW_TOWER,
        cost: &[(ITEM_WOOD, 8), (ITEM_STONE, 4), (ITEM_METAL_PART, 1)],
        hp: 80,
        range_hexes: 2,
        dps: 10,
        is_wall: false,
    },
    DefenseKind {
        name: ITEM_BALLISTA,
        cost: &[(ITEM_WOOD, 6), (ITEM_METAL_PART, 4), (ITEM_BONE, 8)],
        hp: 60,
        range_hexes: 3,
        dps: 22,
        is_wall: false,
    },
];

/// Look up a structure kind by name.
pub fn defense_kind(name: &str) -> Option<&'static DefenseKind> {
    DEFENSE_KINDS.iter().find(|k| k.name == name)
}

/// Repair rate when a structure is fixed with Wood (Phase 2; kept for parity).
pub const REPAIR_HP_PER_WOOD: u32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_radius_grows_by_level_and_caps() {
        assert_eq!(claim_radius(1), 2);
        assert_eq!(claim_radius(2), 3);
        assert_eq!(claim_radius(3), 4);
        assert_eq!(claim_radius(4), 5);
        assert_eq!(claim_radius(99), 5, "hard cap");
    }

    #[test]
    fn capital_hp_grows_per_level() {
        assert_eq!(capital_max_hp(1), 100);
        assert_eq!(capital_max_hp(2), 125);
    }

    #[test]
    fn capital_level_curve_follows_100_l2() {
        assert_eq!(capital_level_from_xp(0), 1);
        assert_eq!(capital_level_from_xp(399), 1);
        assert_eq!(capital_xp_for_level(2), 400);
        assert_eq!(capital_level_from_xp(400), 2);
        assert_eq!(capital_xp_for_level(3), 900);
        assert_eq!(capital_level_from_xp(1000), 3);
    }

    #[test]
    fn mob_kinds_unlock_by_capital_level() {
        let l1 = mob_kinds_for_level(1);
        assert_eq!(l1.len(), 1);
        assert_eq!(l1[0].name, "Scavenger");
        let l2 = mob_kinds_for_level(2);
        assert_eq!(l2.len(), 2);
        assert!(mob_kinds_for_level(3).len() == 3);
        assert!(mob_kind("Scavenger").is_some());
        assert!(mob_kind("Skeleton").is_none(), "no invented kinds");
    }

    #[test]
    fn wave_size_scales_with_pressure_and_caps() {
        assert_eq!(wave_size(1, 1), 3, "(2 + 0.5).ceil()");
        assert_eq!(wave_size(4, 1), 4);
        // 1.15^2 ≈ 1.3225 → (2 + 1)·1.3225 = 3.9675 → ceil 4.
        assert_eq!(wave_size(2, 3), 4);
        // Territory of 20 hexes at L5 far exceeds the cap.
        assert_eq!(wave_size(20, 5), WAVE_MAX_MOBS as usize);
        assert_eq!(wave_size(0, 1), 2, "never empty");
    }

    #[test]
    fn wave_composition_unlocks_kinds_in_order() {
        assert_eq!(wave_composition(2, 1), ["Scavenger", "Scavenger"]);
        let mixed = wave_composition(4, 2);
        assert!(mixed.contains(&"BoneWalker"), "{mixed:?}");
        let all = wave_composition(6, 3);
        assert!(all.contains(&"RustHusk"), "{all:?}");
    }

    #[test]
    fn defense_kinds_are_buildable_and_walled() {
        assert_eq!(DEFENSE_KINDS.len(), 3);
        assert!(defense_kind("ArrowTower").is_some());
        assert!(defense_kind("BoneBarricade").unwrap().is_wall);
        assert_eq!(defense_kind("ArrowTower").unwrap().range_hexes, 2);
        assert_eq!(defense_kind("Ballista").unwrap().range_hexes, 3);
        assert!(defense_kind("Castle").is_none(), "no invented kinds");
    }

    #[test]
    fn every_defense_cost_only_existing_items() {
        for kind in DEFENSE_KINDS {
            assert!(!kind.cost.is_empty(), "{} cost empty", kind.name);
            for (item, qty) in kind.cost {
                assert!(*qty > 0, "{} cost {item} x{qty}", kind.name);
            }
        }
    }

    #[test]
    fn damage_downs_mobs_within_expected_ticks() {
        // Arrow tower (10 dps) kills a Scavenger (30 hp) in 3 ticks.
        let scav = mob_kind("Scavenger").unwrap();
        let tower = defense_kind("ArrowTower").unwrap();
        assert_eq!(scav.hp.div_ceil(tower.dps), 3);
        // Ballista (22) drops a RustHusk (140) in 7 ticks.
        let husk = mob_kind("RustHusk").unwrap();
        let ballista = defense_kind("Ballista").unwrap();
        assert_eq!(husk.hp.div_ceil(ballista.dps), 7);
    }
}
