//! Tower-defense layer configuration (Spec 023) — shared by server and client
//!
//! Single source of truth for the raid economy: mob kinds, defense structures,
//! capital stats, land-claim rules and wave pacing. The server enforces all of
//! it; the client only renders what these numbers predict.

/// Inventory item names used by the tower-defense layer.
pub const ITEM_CAPITAL_CORE: &str = "CapitalCore";
pub const ITEM_METAL_PART: &str = "MetalPart";
pub const ITEM_BONE: &str = "Bone";
pub const ITEM_SLIME_GEL: &str = "SlimeGel";
pub const ITEM_BONE_BARRICADE: &str = "BoneBarricade";
pub const ITEM_ARROW_TOWER: &str = "ArrowTower";
pub const ITEM_BALLISTA: &str = "Ballista";
pub const ITEM_STORAGE_CHEST: &str = "StorageChest";
// REV4 buildable expansion (walls / towers / traps; utility + flavor later).
pub const ITEM_WOOD_WALL: &str = "WoodWall";
pub const ITEM_IRON_WALL: &str = "IronWall";
pub const ITEM_LANTERN_TOWER: &str = "LanternTower";
pub const ITEM_COCONUT_TOWER: &str = "CoconutTower";
pub const ITEM_FROST_TOWER: &str = "FrostTower";
pub const ITEM_SPIKE_TRAP: &str = "SpikeTrap";
pub const ITEM_WEB_TRAP: &str = "WebTrap";
pub const ITEM_FIRE_TRAP: &str = "FireTrap";
/// Pre-existing items the defense costs spend (mirrors server `types.rs`).
pub const ITEM_WOOD: &str = "Wood";
pub const ITEM_STONE: &str = "Stone";
/// Bowstring fiber — the same gathered Grass item the bench crafts spend.
pub const ITEM_GRASS: &str = "Grass";
/// Spec 027 §14 addendum: bow ammunition, crafted in the weapon forge.
pub const ITEM_ARROW: &str = "Arrow";

// ---------------------------------------------------------------------------
// Capital city
// ---------------------------------------------------------------------------

/// Capital HP at level 1; each level adds `CAPITAL_HP_PER_LEVEL`.
pub const CAPITAL_HP_BASE: u32 = 100;
pub const CAPITAL_HP_PER_LEVEL: u32 = 25;
/// Minimum hex distance between any two placed capitals. Scales with the
/// territory size (see [`CLAIM_RADIUS_BASE`]) so a fresh capital can never
/// sit inside a rival's effective claim disc: two full level-1 discs
/// (max radius 91) can span 182 hex tiles plus a walking gap.
pub const MIN_CAPITAL_SPACING: i32 = 200;
/// Claimable ring around the capital at level 1; each level adds two.
/// REV9: the claimable region is a SQUARE in world space (half-side =
/// radius · HEX_SIZE·√3 wu), not a hex disc — so "radius" here is the
/// per-axis straight-line reach in hex steps. REV8 raised the initial
/// AREA ×10 (radius 26 → 83, 9.92×); REV9 reshaped it square, which adds
/// the disc's lost corners (area ×4/π ≈ 1.27).
pub const CLAIM_RADIUS_BASE: i32 = 83;
/// Claim radius hard cap (prevents infinite territory): level 5+ plateaus
/// at 91 hex tiles = 25 117 hexes (base + 2/level, same growth as ever).
pub const CLAIM_RADIUS_MAX: i32 = 91;
/// Land-claim cost per hex (consumed from the inventory).
pub const CLAIM_WOOD_COST: u64 = 5;
pub const CLAIM_STONE_COST: u64 = 3;
/// Chebyshev radius of the square a fresh capital auto-owns for free
/// (33×33 tiles ≈ 277 wu half-axis at level): starter buildable area so
/// the first structures go up without claim-grinding. Only land is
/// granted; water inside it stays claimable-by-nobody as usual.
pub const CAPITAL_STARTER_RADIUS: i32 = 16;
/// Sect 022 action-cooldown reuse: one claim every 5s is fine, but keep the
/// claim cadence a little stricter so territory spreads by walking, not spam.
pub const CLAIM_COOLDOWN_SECS: u64 = 5;
/// No raids before this much time has passed since capital placement.
pub const GRACE_FIRST_RAID_SECS: u64 = 30 * 60;
/// Reset raid immunity window after the capital is destroyed and re-placed.
pub const GRACE_REBUILD_SECS: u64 = 24 * 3600;
/// Capital XP granted per mob killed (levels use the standard 100·L² curve).
pub const CAPITAL_XP_PER_MOB: u64 = 10;

/// Max level a defense structure reaches by XP transfer (Spec 024 §2.2).
pub const STRUCTURE_LEVEL_MAX: u8 = 5;
/// HP gained per structure level, in percent of the kind base.
pub const STRUCTURE_HP_STEP_PCT: u32 = 30;
/// DPS gained per structure level, in percent of the kind base (towers only).
pub const STRUCTURE_DPS_STEP_PCT: u32 = 25;

/// XP the structure needs to go from `level` to `level + 1` (100·L² curve,
/// same family as the player/capital: L1→L2 100, L2→L3 400, …).
pub fn structure_xp_for_next_level(level: u8) -> u64 {
    100 * (level as u64).pow(2)
}

/// Max HP of a structure at `level` (+30%/level over the kind base).
pub fn structure_max_hp(kind: &DefenseKind, level: u8) -> u32 {
    let l = u32::from(level.max(1)).min(u32::from(STRUCTURE_LEVEL_MAX));
    ((kind.hp as u64 * (100 + (STRUCTURE_HP_STEP_PCT as u64) * u64::from(l - 1))) / 100) as u32
}

/// Tower DPS at `level` (+25%/level over the kind base, floored).
pub fn structure_dps(kind: &DefenseKind, level: u8) -> u32 {
    let l = u32::from(level.max(1)).min(u32::from(STRUCTURE_LEVEL_MAX));
    ((kind.dps as u64 * (100 + (STRUCTURE_DPS_STEP_PCT as u64) * u64::from(l - 1))) / 100) as u32
}

/// Spend player surplus XP into a structure's level bucket (Spec 024 §2.2):
/// takes at most `spendable`, levels up when the bucket crosses the
/// threshold. Returns `(level, bucket, xp_spent)`. No-op (spent 0) once the
/// structure is at [`STRUCTURE_LEVEL_MAX`].
pub fn structure_transfer_xp(level: u8, bucket: u64, spendable: u64) -> (u8, u64, u64) {
    if level >= STRUCTURE_LEVEL_MAX {
        return (STRUCTURE_LEVEL_MAX, bucket, 0);
    }
    let (mut level, mut bucket, mut spent) = (level, bucket, 0);
    loop {
        let need = structure_xp_for_next_level(level);
        let take = (need - bucket).min(spendable.saturating_sub(spent));
        bucket += take;
        spent += take;
        if bucket < need {
            break;
        }
        level += 1;
        bucket = 0;
        if level >= STRUCTURE_LEVEL_MAX {
            break;
        }
    }
    (level, bucket, spent)
}

/// Max claim radius for a capital level (base 83, +2/level, capped at 91).
pub fn claim_radius(level: u32) -> i32 {
    CLAIM_RADIUS_BASE
        .saturating_add((level.saturating_sub(1) as i32).saturating_mul(2))
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
/// Claimed hexes that saturate the wave-size pressure term (`wave_size`
/// bottoms out at 2 and hits the 12-cap at 160 claimed hexes — most of the
/// 217-hex L1 disc — so pressure still escalates over the whole territory).
pub const WAVE_SIZE_DIVISOR: u32 = 16;
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
    /// Attack reach: melee kinds need adjacency (1); ranged kinds hit from
    /// up to this many hexes away without closing (Spec 025 §3).
    pub range_hexes: i32,
    /// Bonus dps vs wall structures (sapper — Spear Goblin).
    pub wall_dps: u32,
    /// Splash damage: main target takes full dps, every OTHER structure
    /// within 1 hex of the mob takes half (Bomb Goblin).
    pub splash: bool,
    /// (drop item, chance %) — credited to defending player on kill.
    pub drop_item: &'static str,
    /// Drop quantity is `drop_qty` when the chance-% roll hits.
    pub drop_chance_pct: u32,
    pub drop_qty: u32,
    /// Spec 036 - THE BEAT: seconds between telegraphed player-strikes.
    /// The mob winds one beat before `next_strike_at` lands; per-kind
    /// periods keep each archetype in a readable rhythm (Biter 4, Sapper
    /// 4, Skirmisher 5, Bomber 7, Bloom 6, Boss 3).
    pub strike_period_secs: u32,
}

/// Spec 025 §3 roster — real enemy art, distinct mechanics. March rule and
/// 1s combat tick stay from Spec 023; new knobs only add behavior.
/// REV2: slime + myconid color-recolor tiers layer in as stat elites.
pub const MOB_KINDS: [MobKind; 15] = [
    MobKind {
        name: "Slime",
        hp: 22,
        dps: 3,
        step_interval_secs: 2,
        min_capital_level: 1,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 35,
        drop_qty: 1,
        strike_period_secs: 4,
    },
    MobKind {
        name: "Blue Slime",
        hp: 30,
        dps: 4,
        step_interval_secs: 2,
        min_capital_level: 1,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 55,
        drop_qty: 1,
        strike_period_secs: 4,
    },
    // Elite slime color tiers (Spec 025 rev2): pink is the sprinter, pupple
    // the mid tank, black the heavy charger. Same plain-mechanic family,
    // stat-scaled — the art recolors ship free.
    MobKind {
        name: "Pink Slime",
        hp: 40,
        dps: 5,
        step_interval_secs: 1,
        min_capital_level: 2,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 45,
        drop_qty: 1,
        strike_period_secs: 4,
    },
    MobKind {
        name: "Pupple Slime",
        hp: 70,
        dps: 6,
        step_interval_secs: 2,
        min_capital_level: 3,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 55,
        drop_qty: 1,
        strike_period_secs: 4,
    },
    MobKind {
        name: "Black Slime",
        hp: 130,
        dps: 10,
        step_interval_secs: 4,
        min_capital_level: 4,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 70,
        drop_qty: 2,
        strike_period_secs: 4,
    },
    MobKind {
        name: "Spear Goblin",
        hp: 55,
        dps: 7,
        step_interval_secs: 3,
        min_capital_level: 2,
        range_hexes: 1,
        wall_dps: 5,
        splash: false,
        drop_item: ITEM_BONE,
        drop_chance_pct: 60,
        drop_qty: 1,
        strike_period_secs: 4,
    },
    MobKind {
        name: "Archer Goblin",
        hp: 40,
        dps: 6,
        step_interval_secs: 2,
        min_capital_level: 2,
        range_hexes: 3,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_BONE,
        drop_chance_pct: 40,
        drop_qty: 2,
        strike_period_secs: 5,
    },
    MobKind {
        name: "Myconid",
        hp: 70,
        dps: 5,
        step_interval_secs: 4,
        min_capital_level: 3,
        range_hexes: 2,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 45,
        drop_qty: 2,
        strike_period_secs: 5,
    },
    // Myconid color tiers (Spec 025 rev2): every recolor is a tougher
    // spore-caster; red is the myconid elite that opens at capital L5.
    MobKind {
        name: "Myconid Blue",
        hp: 90,
        dps: 6,
        step_interval_secs: 4,
        min_capital_level: 4,
        range_hexes: 2,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 50,
        drop_qty: 2,
        strike_period_secs: 5,
    },
    MobKind {
        name: "Myconid Pink",
        hp: 85,
        dps: 8,
        step_interval_secs: 5,
        min_capital_level: 5,
        range_hexes: 2,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 55,
        drop_qty: 2,
        strike_period_secs: 5,
    },
    MobKind {
        name: "Myconid Purple",
        hp: 100,
        dps: 8,
        step_interval_secs: 5,
        min_capital_level: 5,
        range_hexes: 3,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 60,
        drop_qty: 3,
        strike_period_secs: 5,
    },
    MobKind {
        name: "Myconid Red",
        hp: 150,
        dps: 12,
        step_interval_secs: 6,
        min_capital_level: 5,
        range_hexes: 3,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 90,
        drop_qty: 4,
        strike_period_secs: 5,
    },
    MobKind {
        name: "Bomb Goblin",
        hp: 45,
        dps: 10,
        step_interval_secs: 2,
        min_capital_level: 3,
        range_hexes: 2,
        wall_dps: 0,
        splash: true,
        drop_item: ITEM_METAL_PART,
        drop_chance_pct: 50,
        drop_qty: 1,
        strike_period_secs: 7,
    },
    // Rooted turret: never marches — spawns inside the territory ring and
    // spits from up to 3 hexes.
    MobKind {
        name: "Venom Bloom",
        hp: 90,
        dps: 8,
        step_interval_secs: 0,
        min_capital_level: 4,
        range_hexes: 3,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 80,
        drop_qty: 3,
        strike_period_secs: 6,
    },
    MobKind {
        name: "Big Slime",
        hp: 260,
        dps: 14,
        step_interval_secs: 5,
        min_capital_level: 4,
        range_hexes: 1,
        wall_dps: 0,
        splash: false,
        drop_item: ITEM_SLIME_GEL,
        drop_chance_pct: 100,
        drop_qty: 5,
        strike_period_secs: 3,
    },
];

/// Look up a mob kind by name.
pub fn mob_kind(name: &str) -> Option<&'static MobKind> {
    MOB_KINDS.iter().find(|k| k.name == name)
}

// ---------------------------------------------------------------------------
// Spec 036 — THE BEAT: archetypes + strike contract
// ---------------------------------------------------------------------------

/// The five behavioral archetypes (spec §3.4) every stat kind regroups
/// into, plus the boss. Client reads none of this — it's the server's
/// state-machine selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobArchetype {
    /// Rushers: on strike, lunge 1 hex toward the player.
    Biter,
    /// Stand-off shooters: after each strike, kite 1 hex away.
    Skirmisher,
    /// Greedy melee that chews walls; vs player, plain contract.
    Sapper,
    /// Area denier: the strike also splashes adjacent structures.
    Bomber,
    /// Rooted zone threat: never schedules; its wind paints the zone.
    Bloom,
    /// Big Slime's personal rhythm (spec §3.5).
    Boss,
}

/// Kind → archetype map (spec §3.4 table, exact): slime lines are Biters,
/// archer goblin + myconid lines are Skirmishers, Spear Goblin is the
/// Sapper, Bomb Goblin the Bomber, Venom Bloom the Bloom, Big Slime the
/// Boss.
pub fn mob_archetype(kind_name: &str) -> Option<MobArchetype> {
    match kind_name {
        "Slime" | "Blue Slime" | "Pink Slime" | "Pupple Slime" | "Black Slime" => {
            Some(MobArchetype::Biter)
        }
        "Archer Goblin" | "Myconid" | "Myconid Blue" | "Myconid Pink" | "Myconid Purple"
        | "Myconid Red" => Some(MobArchetype::Skirmisher),
        "Spear Goblin" => Some(MobArchetype::Sapper),
        "Bomb Goblin" => Some(MobArchetype::Bomber),
        "Venom Bloom" => Some(MobArchetype::Bloom),
        "Big Slime" => Some(MobArchetype::Boss),
        _ => None,
    }
}

/// The 1-beat wind: the strike resolves `next_strike_at - 1 >= now` ticks
/// after arming (pure constant, not a knob).
pub const WINDUP_BEATS: u64 = 1;

/// `strike_burst` (pure, unit-tested): the discrete damage one telegraphed
/// strike deals, replacing the per-tick dps aura. `dps × 2`, floored at 4
/// (spec §3.2 note: standing in a windup stings more than one aura tick,
/// but leaving after the wind is a real answer).
pub fn strike_burst(dps: u32) -> u32 {
    (dps * 2).max(4)
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
///
/// REV6: territory grew ~10× (radius 8 ⇒ a 217-hex L1 disc), so the old
/// `claimed/2` divisor saturated the 12-mob cap at 12 claimed hexes — every
/// real territory fought max pressure. `/16` now spans the whole disc:
/// L1 caps out at ~`WAVE_SIZE_PRESSURE_CAP_HEXES` claims, and the level
/// multiplier `1.15^(L-1)` stays the main growth lever below the cap.
pub fn wave_size(claimed_hexes: usize, capital_level: u32) -> usize {
    let base = 2.0 + claimed_hexes as f32 / WAVE_SIZE_DIVISOR as f32;
    let scaled = base * 1.15_f32.powi(capital_level.saturating_sub(1) as i32);
    (scaled.ceil() as usize).clamp(2, WAVE_MAX_MOBS as usize)
}

/// Deterministic composition for a wave of `size` mobs at a capital level:
/// cycles available kinds so higher levels naturally field the heavier ones.
pub fn wave_composition(size: usize, capital_level: u32) -> Vec<&'static str> {
    let kinds = mob_kinds_for_level(capital_level);
    (0..size).map(|i| kinds[i % kinds.len()].name).collect()
}

/// Boss cadence (Spec 025 §4): every 4th wave (0-based index 3, 7, …) against
/// a capital of L4+ leads with a Big Slime. Deterministic, no RNG.
pub fn boss_wave(wave_index: u64, capital_level: u32) -> bool {
    capital_level >= 4 && wave_index % 4 == 3
}

// ---------------------------------------------------------------------------
// REV3 §14 — named threat patterns (hand-tuned ladders per wave index)
// ---------------------------------------------------------------------------

/// Build a pattern composition for `size` mobs from `eligible` kinds: named
/// picks fall back to the first eligible kind when gated out, and the wave
/// never empties or over-fills.
fn pattern_kinds(
    counts: &[(&'static str, usize)],
    eligible: &[&'static str],
    size: usize,
) -> Vec<&'static str> {
    let fallback = *eligible.first().unwrap_or(&"Slime");
    let mut out = Vec::with_capacity(size);
    for (name, want) in counts {
        if eligible.contains(name) {
            for _ in 0..*want {
                if out.len() < size {
                    out.push(*name);
                }
            }
        }
    }
    while out.len() < size {
        out.push(fallback);
    }
    out
}

/// Named pattern for one wave (deterministic, integer). Cadences pick the
/// first match: Scout(s) opening, then Demolition > Volley > Sap Team >
/// Swarm, each gated by its minimum capital level. `None` keeps the plain
/// cycle from [`mob_kinds_for_level`] (§3 "cycle"); the boss cadence
/// ([`boss_wave`]) stays independent of patterns, per §4.
pub fn wave_pattern(
    wave_index: u64,
    size: usize,
    capital_level: u32,
) -> Option<(&'static str, Vec<&'static str>)> {
    let eligible = eligible_names(capital_level);

    // Scout Line (§14): wave 0 of a ladder — pure fodder, teaches the loop.
    if wave_index % 8 == 0 {
        let shields = size / 2;
        return Some((
            "Scout Line",
            pattern_kinds(
                &[("Slime", size - shields), ("Blue Slime", shields)],
                &eligible,
                size,
            ),
        ));
    }
    // Demolition: the bomb line breaks a passive wall funnel (≥ L3).
    if wave_index % 7 == 5 && capital_level >= 3 {
        let bombs = size.min(2);
        return Some((
            "Demolition",
            pattern_kinds(
                &[("Bomb Goblin", bombs), ("Spear Goblin", size - bombs)],
                &eligible,
                size,
            ),
        ));
    }
    // Volley: ranged pressure that outruns slow towers (≥ L2).
    if wave_index % 5 == 2 && capital_level >= 2 {
        let archers = ((size + 2) / 3).max(1).min(size);
        return Some((
            "Volley",
            pattern_kinds(
                &[("Archer Goblin", archers), ("Blue Slime", size - archers)],
                &eligible,
                size,
            ),
        ));
    }
    // Sap Team: the wall-chew line makes barricades matter (≥ L2).
    if wave_index % 4 == 1 && capital_level >= 2 {
        let spears = size / 2 + size % 2;
        return Some((
            "Sap Team",
            pattern_kinds(
                &[("Spear Goblin", spears), ("Slime", size - spears)],
                &eligible,
                size,
            ),
        ));
    }
    // Swarm: dense fodder for focus-fire practice (every 3rd wave).
    if wave_index % 3 == 2 {
        let out: Vec<&str> = (0..size)
            .map(|i| if i % 2 == 1 { "Blue Slime" } else { "Slime" })
            .map(|name| {
                if eligible.contains(&name) { name } else { *eligible.first().unwrap_or(&"Slime") }
            })
            .collect();
        return Some(("Swarm", out));
    }
    None
}

fn eligible_names(level: u32) -> Vec<&'static str> {
    mob_kinds_for_level(level)
        .into_iter()
        .map(|k| k.name)
        .collect()
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
    /// REV4 traps: burst damage dealt once, when a raider steps onto the
    /// trap's hex (the trap is then consumed). 0 = not a spike trap.
    pub trap_dmg: u32,
    /// REV4 traps: seconds a raider is pinned in place after stepping onto
    /// the trap's hex (trap consumed). 0 = not a web trap.
    pub pin_secs: u32,
    /// REV4 traps: burn-zone traps tick `dps` to every raider within
    /// `range_hexes` for this many seconds, then burn out (the structure's
    /// `hp` DOES double as the fuel budget for the bar+burnout). 0 = not.
    pub burn_secs: u32,
    /// REV4 towers: mobs within this melee radius march at half speed.
    /// 0 = no slow.
    pub slow_radius: u32,
}

/// Spike burst that consumes the trap (one-shots Slime 22 HP fodder).
pub const TRAP_SPIKE_DMG: u32 = 40;

/// True for any consumable trap kind (never rebuilt from rubble).
pub fn is_trap(kind: &DefenseKind) -> bool {
    kind.trap_dmg > 0 || kind.pin_secs > 0 || kind.burn_secs > 0
}

pub const DEFENSE_KINDS: [DefenseKind; 12] = [
    DefenseKind {
        name: ITEM_BONE_BARRICADE,
        cost: &[(ITEM_BONE, 6)],
        hp: 60,
        range_hexes: 0,
        dps: 0,
        is_wall: true,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_ARROW_TOWER,
        cost: &[(ITEM_WOOD, 8), (ITEM_STONE, 4), (ITEM_METAL_PART, 1)],
        hp: 80,
        range_hexes: 2,
        dps: 10,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_BALLISTA,
        cost: &[(ITEM_WOOD, 6), (ITEM_METAL_PART, 4), (ITEM_BONE, 8)],
        hp: 60,
        range_hexes: 3,
        dps: 22,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    // Spec 024 §2.3: storage chest — vault for items on owned soil;
    // destroyed by raids after its contents are refunded to the owner.
    DefenseKind {
        name: ITEM_STORAGE_CHEST,
        cost: &[(ITEM_WOOD, 8), (ITEM_METAL_PART, 2)],
        hp: 60,
        range_hexes: 0,
        dps: 0,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    // ---- REV4 walls: wood is cheap filler, iron is the heavy tier ----
    DefenseKind {
        name: ITEM_WOOD_WALL,
        cost: &[(ITEM_WOOD, 5)],
        hp: 70,
        range_hexes: 0,
        dps: 0,
        is_wall: true,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_IRON_WALL,
        cost: &[(ITEM_METAL_PART, 6), (ITEM_STONE, 4)],
        hp: 160,
        range_hexes: 0,
        dps: 0,
        is_wall: true,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    // ---- REV4 towers: lantern = short-range burst, coconut = long arc,
    // frost = weak dps + march slowdown aura ----
    DefenseKind {
        name: ITEM_LANTERN_TOWER,
        cost: &[(ITEM_WOOD, 4), (ITEM_BONE, 6)],
        hp: 70,
        range_hexes: 2,
        dps: 18,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_COCONUT_TOWER,
        cost: &[(ITEM_WOOD, 6), (ITEM_METAL_PART, 3)],
        hp: 90,
        range_hexes: 3,
        dps: 14,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_FROST_TOWER,
        cost: &[(ITEM_STONE, 6), (ITEM_BONE, 4)],
        hp: 80,
        range_hexes: 2,
        dps: 8,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 2,
    },
    // ---- REV4 traps: consumable, never chewed, trigger by proximity ----
    DefenseKind {
        name: ITEM_SPIKE_TRAP,
        cost: &[(ITEM_BONE, 4)],
        hp: 10,
        range_hexes: 0,
        dps: 0,
        is_wall: false,
        trap_dmg: TRAP_SPIKE_DMG,
        pin_secs: 0,
        burn_secs: 0,
        slow_radius: 0,
    },
    DefenseKind {
        name: ITEM_WEB_TRAP,
        cost: &[(ITEM_WOOD, 3)],
        hp: 10,
        range_hexes: 0,
        dps: 0,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 5,
        burn_secs: 0,
        slow_radius: 0,
    },
    // Burn fuel = hp: the bar also shows burnout; every tick spends 1 hp.
    DefenseKind {
        name: ITEM_FIRE_TRAP,
        cost: &[(ITEM_WOOD, 3), (ITEM_BONE, 3)],
        hp: 10,
        range_hexes: 1,
        dps: 6,
        is_wall: false,
        trap_dmg: 0,
        pin_secs: 0,
        burn_secs: 10,
        slow_radius: 0,
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
        assert_eq!(claim_radius(1), 83);
        assert_eq!(claim_radius(2), 85);
        assert_eq!(claim_radius(5), 91);
        assert_eq!(claim_radius(99), 91, "hard cap");
        // The radius value and ×10 history stay from REV8 (radius 26 = 2107
        // hex-disc ≈ radius 83's disc area). REV9 renders the claimable
        // region as a WORLD SQUARE with half-side radius·HEX_SIZE·√3 wu —
        // disc math is kept here as the sizing reference only.
        assert_eq!(3 * 83 * 83 + 3 * 83 + 1, 20_917);
        assert_eq!(3 * 26 * 26 + 3 * 26 + 1, 2107);
        assert_eq!(20_917 / 2107, 9, "initial claimable area ~×10 (9.92×)");
        assert_eq!(3 * 91 * 91 + 3 * 91 + 1, 25_117);
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
    fn every_kind_has_a_beat_period_and_archetype() {
        // Spec 036 §3.4: every kind winds and strikes in a readable rhythm.
        for kind in MOB_KINDS.iter() {
            assert!(
                (3..=10).contains(&kind.strike_period_secs),
                "{} period {} outside the 3..=10 band",
                kind.name,
                kind.strike_period_secs
            );
            assert!(
                mob_archetype(kind.name).is_some(),
                "{} must map to an archetype",
                kind.name
            );
        }
        // The spec table's exact periods.
        let pinned: &[(&str, u32)] = &[
            ("Slime", 4),
            ("Blue Slime", 4),
            ("Pink Slime", 4),
            ("Pupple Slime", 4),
            ("Black Slime", 4),
            ("Spear Goblin", 4),
            ("Archer Goblin", 5),
            ("Myconid", 5),
            ("Myconid Blue", 5),
            ("Myconid Pink", 5),
            ("Myconid Purple", 5),
            ("Myconid Red", 5),
            ("Bomb Goblin", 7),
            ("Venom Bloom", 6),
            ("Big Slime", 3),
        ];
        for (name, period) in pinned {
            assert_eq!(mob_kind(name).unwrap().strike_period_secs, *period);
        }
        // Archetype shape rules: the bloom is the only rooted one; the boss
        // is the fastest rhythm.
        assert_eq!(mob_archetype("Venom Bloom"), Some(MobArchetype::Bloom));
        assert_eq!(mob_archetype("Big Slime"), Some(MobArchetype::Boss));
        assert_eq!(mob_archetype("Bomb Goblin"), Some(MobArchetype::Bomber));
        assert_eq!(mob_archetype("Spear Goblin"), Some(MobArchetype::Sapper));
        assert_eq!(strike_burst(3), 6);
        assert_eq!(strike_burst(2), 4, "floored at 4");
        assert_eq!(strike_burst(14), 28);
    }

    #[test]
    fn mob_kinds_unlock_by_capital_level() {
        let l1 = mob_kinds_for_level(1);
        assert_eq!(l1.len(), 2, "both base slimes open the roster (Spec 025 §3)");
        assert_eq!(l1[0].name, "Slime");
        assert_eq!(l1[1].name, "Blue Slime");
        assert_eq!(mob_kinds_for_level(2).len(), 5, "+ pink slime, Spear/Archer");
        assert_eq!(mob_kinds_for_level(3).len(), 8, "+ pupple slime, Myconid/Bomb");
        assert_eq!(mob_kinds_for_level(4).len(), 12, "+ black slime, Myconid Blue, bloom, boss");
        assert_eq!(mob_kinds_for_level(5).len(), 15, "+ myconid pink/purple/red");
        assert!(mob_kind("Slime").is_some());
        assert!(mob_kind("Big Slime").is_some());
        assert!(mob_kind("Black Slime").is_some());
        assert!(mob_kind("Myconid Red").is_some());
        assert!(mob_kind("Scavenger").is_none(), "Spec 023 roster retired");
        assert!(mob_kind("Skeleton").is_none(), "no invented kinds");
    }

    #[test]
    fn mob_roster_mechanic_knobs_are_sane() {
        for kind in &MOB_KINDS {
            // Melee kinds reach 1 hex; ranged kinds reach more; nothing has
            // zero reach (a rooted bloom still bites at ring 3).
            assert!(
                kind.range_hexes >= 1,
                "{} must be able to hit something",
                kind.name
            );
            assert!(
                kind.step_interval_secs > 0 || kind.name == "Venom Bloom",
                "only the rooted bloom never marches, got {} step={}",
                kind.name,
                kind.step_interval_secs
            );
            assert!(
                kind.drop_chance_pct <= 100 && kind.drop_qty >= 1,
                "{} drop roll unsafe",
                kind.name
            );
        }
        // Sapper + splash are unique to their goblins.
        assert_eq!(mob_kind("Spear Goblin").unwrap().wall_dps, 5);
        assert!(MOB_KINDS.iter().filter(|k| k.wall_dps > 0).count() == 1);
        assert!(mob_kind("Bomb Goblin").unwrap().splash);
        assert!(MOB_KINDS.iter().filter(|k| k.splash).count() == 1);
        // The boss is the big stat-stick, rarely: highest hp.
        let boss = mob_kind("Big Slime").unwrap();
        assert!(MOB_KINDS.iter().all(|k| k.hp <= boss.hp));
    }

    #[test]
    fn boss_cadence_is_every_fourth_wave_from_l4() {
        assert!(!boss_wave(0, 4));
        assert!(!boss_wave(1, 4));
        assert!(!boss_wave(2, 4));
        assert!(boss_wave(3, 4), "4th wave indexes 3/8/12…");
        assert!(boss_wave(7, 4));
        assert!(!boss_wave(3, 3), "pre-L4 capitals never field the boss");
    }

    #[test]
    fn wave_size_scales_with_pressure_and_caps() {
        // REV6 divisor 16: 12 claimed hexes are now only a starter of ~3,
        // instead of saturating the cap.
        assert_eq!(wave_size(12, 1), 3, "(2 + 12/16).ceil() = (2.75).ceil()");
        // (2 + 4/16) = 2.25 → ceil 3.
        assert_eq!(wave_size(4, 1), 3);
        // 1.15^2 ≈ 1.3225 → (2.75)·1.3225 = 3.637 → ceil 4.
        assert_eq!(wave_size(12, 3), 4);
        // Pressure still reaches the cap within the L1 disc (217 hexes):
        // 160 claimed → 2 + 10 = 12.
        assert_eq!(wave_size(160, 1), WAVE_MAX_MOBS as usize);
        assert_eq!(wave_size(216, 5), WAVE_MAX_MOBS as usize, "stays capped");
        assert_eq!(wave_size(0, 1), 2, "never empty");
    }

    #[test]
    fn wave_composition_unlocks_kinds_in_order() {
        assert_eq!(wave_composition(2, 1), ["Slime", "Blue Slime"]);
        // L2 cycles three kinds once their tier opens.
        assert_eq!(
            wave_composition(3, 2),
            ["Slime", "Blue Slime", "Pink Slime"]
        );
        let all = wave_composition(15, 5);
        assert!(all.contains(&"Myconid Red"), "{all:?}");
        assert!(all.contains(&"Venom Bloom"), "{all:?}");
    }

    #[test]
    fn wave_patterns_fire_on_cadence_and_preserve_size() {
        // Wave 0: Scout Line opener, pure fodder.
        let (name, comp) = wave_pattern(0, 5, 1).unwrap();
        assert_eq!(name, "Scout Line");
        assert_eq!(comp.len(), 5);
        assert!(comp.iter().all(|k| *k == "Slime" || *k == "Blue Slime"));
        // Scout repeats every 8.
        assert_eq!(wave_pattern(8, 3, 1).unwrap().0, "Scout Line");
        // Sap Team on %4 == 1 at L2+, gated at L1.
        assert_eq!(wave_pattern(1, 4, 2).unwrap().0, "Sap Team");
        assert!(wave_pattern(1, 4, 1).is_none(), "gated at L1");
        // An index that matches Sap (%4==1) but not the higher-priority
        // cadences (Demolition %7==5, Volley %5==2).
        let (name, comp) = wave_pattern(9, 4, 3).unwrap();
        assert_eq!(name, "Sap Team");
        assert_eq!(comp.len(), 4);
        // Volley on %5 == 2, level-gated (at L1 that index falls back to
        // the Swarm cadence instead, which is ungated).
        assert_eq!(wave_pattern(2, 7, 2).unwrap().0, "Volley");
        assert_ne!(
            wave_pattern(2, 7, 1).unwrap().0,
            "Volley",
            "gated at L1 — falls back to a lower cadence, never Volley"
        );
        // Demolition on %7 == 5, ≥ L3; bombs capped at 2.
        let (name, comp) = wave_pattern(12, 4, 3).unwrap();
        assert_eq!(name, "Demolition");
        assert_eq!(comp.iter().filter(|k| **k == "Bomb Goblin").count(), 2);
        // Swarm fodder on %3 == 2 (index, at L1, avoiding every
        // higher-priority cadence: 11 hits Scout? no — Demolition/Volley/
        // Sap all miss, Swarm is the only match).
        let (name, comp) = wave_pattern(11, 3, 1).unwrap();
        assert_eq!(name, "Swarm");
        assert!(comp.iter().all(|k| k.ends_with("Slime") || k.ends_with("Blue Slime")));
        // Boss cadence is untouched by patterns.
        assert!(boss_wave(3, 4));
        assert!(!boss_wave(3, 3));
    }

    #[test]
    fn wave_patterns_never_emit_locked_kinds() {
        // A pattern must never emit a kind the capital cannot field: every
        // kind in every composition of indices 0..24 exists in the roster
        // and meets min_capital_level for that level.
        for level in 1..=5u32 {
            for idx in 0..24u64 {
                if let Some((name, comp)) = wave_pattern(idx, 5, level) {
                    assert!(
                        matches!(
                            name,
                            "Scout Line" | "Sap Team" | "Volley" | "Demolition" | "Swarm"
                        ),
                        "{name} not in the named set"
                    );
                    for kind in &comp {
                        let k = mob_kind(kind)
                            .unwrap_or_else(|| panic!("{name} emitted unknown kind {kind}"));
                        assert!(
                            k.min_capital_level <= level,
                            "{name} emitted {kind} (needs L{}), {level}",
                            k.min_capital_level
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn defense_kinds_are_buildable_and_walled() {
        assert_eq!(DEFENSE_KINDS.len(), 12, "4 legacy + 8 REV4 expansion kinds");
        assert!(defense_kind("ArrowTower").is_some());
        assert!(defense_kind("BoneBarricade").unwrap().is_wall);
        assert_eq!(defense_kind("ArrowTower").unwrap().range_hexes, 2);
        assert_eq!(defense_kind("Ballista").unwrap().range_hexes, 3);
        assert!(defense_kind("Castle").is_none(), "no invented kinds");
        // The chest is a passive buildable: no attack, no wall.
        let chest = defense_kind("StorageChest").unwrap();
        assert!(!chest.is_wall);
        assert_eq!(chest.dps, 0);
        assert_eq!(chest.range_hexes, 0);
        assert_eq!(chest.hp, 60);
    }

    /// REV4 buildable expansion — every kind's config stays sane and the
    /// rows stay strictly category-separated (wall / tower / trap).
    #[test]
    fn new_buildable_kinds_stay_sane() {
        // All 12 kinds resolve + have positive HP.
        for k in &DEFENSE_KINDS {
            assert!(defense_kind(k.name).is_some(), "{} resolves", k.name);
            assert!(k.hp > 0, "{} hp", k.name);
            assert!(!k.cost.is_empty(), "{} cost", k.name);
        }
        // Walls: exactly the three wall kinds, no attack, no trap bits.
        let walls: Vec<&str> = DEFENSE_KINDS
            .iter()
            .filter(|k| k.is_wall)
            .map(|k| k.name)
            .collect();
        assert_eq!(
            walls,
            vec![ITEM_BONE_BARRICADE, ITEM_WOOD_WALL, ITEM_IRON_WALL]
        );
        for w in walls.iter() {
            let k = defense_kind(w).unwrap();
            assert_eq!(k.dps, 0);
            assert_eq!(k.trap_dmg + k.pin_secs + k.burn_secs, 0);
        }
        // Towers: track range/dps, carry zero trap flags.
        for t in [ITEM_ARROW_TOWER, ITEM_BALLISTA, ITEM_LANTERN_TOWER, ITEM_COCONUT_TOWER, ITEM_FROST_TOWER] {
            let k = defense_kind(t).unwrap();
            assert!(!k.is_wall);
            assert!(k.dps > 0 && k.range_hexes >= 2, "{t} dps/range");
            assert!(!is_trap(k), "{t} is not a trap");
        }
        // Frost is the only slow aura; ice does not melt walls.
        let frost = defense_kind(ITEM_FROST_TOWER).unwrap();
        assert_eq!(frost.slow_radius, 2);
        assert!(DEFENSE_KINDS.iter().filter(|k| k.slow_radius > 0).count() == 1);
        // Traps: consumable, cheap, never walls, never level-dps generic.
        for t in [ITEM_SPIKE_TRAP, ITEM_WEB_TRAP, ITEM_FIRE_TRAP] {
            let k = defense_kind(t).unwrap();
            assert!(!k.is_wall && is_trap(k), "{t}");
        }
        let spike = defense_kind(ITEM_SPIKE_TRAP).unwrap();
        assert!(spike.trap_dmg >= mob_kind("Slime").unwrap().hp, "spike one-shots fodder");
        assert_eq!(spike.pin_secs, 0);
        let web = defense_kind(ITEM_WEB_TRAP).unwrap();
        assert_eq!(web.pin_secs, 5);
        assert_eq!(web.trap_dmg, 0);
        // Fire trap ticks dps while burning; fuel 10 = burnout budget.
        let fire = defense_kind(ITEM_FIRE_TRAP).unwrap();
        assert_eq!(fire.dps, 6);
        assert_eq!(fire.range_hexes, 1);
        assert_eq!(fire.hp, fire.burn_secs, "hp doubles as fuel");
        // Chest is untouched by the trap flags (utility).
        assert!(!is_trap(defense_kind(ITEM_STORAGE_CHEST).unwrap()));
    }

    #[test]
    fn structure_levels_scale_hp_and_dps() {        let tower = defense_kind("ArrowTower").unwrap();
        // HP: +30%/level (80 → 104 → … → 176 at the L5 cap).
        assert_eq!(structure_max_hp(tower, 0), 80, "legacy rows read as L1");
        assert_eq!(structure_max_hp(tower, 1), 80);
        assert_eq!(structure_max_hp(tower, 2), 104);
        assert_eq!(structure_max_hp(tower, 5), 176);
        assert_eq!(structure_max_hp(tower, 9), 176, "over-cap level clamps");
        // DPS: +25%/level floored (10 → 12 → 15 → 17 → 20).
        assert_eq!(structure_dps(tower, 1), 10);
        assert_eq!(structure_dps(tower, 2), 12);
        assert_eq!(structure_dps(tower, 5), 20);
        // Walls and chests never gain DPS.
        assert_eq!(structure_dps(defense_kind("StorageChest").unwrap(), 5), 0);
        assert_eq!(structure_dps(defense_kind("BoneBarricade").unwrap(), 5), 0);
    }

    #[test]
    fn structure_xp_curve_follows_100l2() {
        assert_eq!(structure_xp_for_next_level(1), 100);
        assert_eq!(structure_xp_for_next_level(2), 400);
        assert_eq!(structure_xp_for_next_level(4), 1600);
    }

    #[test]
    fn structure_transfer_spends_surplus_and_levels() {
        // Nothing spare → pure partial fill.
        assert_eq!(structure_transfer_xp(1, 0, 50), (1, 50, 50));
        // Threshold crossed → level-up; any remainder keeps filling the
        // next ring (one call always consumes the entire offer).
        assert_eq!(structure_transfer_xp(1, 0, 150), (2, 50, 150));
        // Big transfer crosses multiple rings (100 + 400 + 100 left).
        assert_eq!(structure_transfer_xp(1, 0, 600), (3, 100, 600));
        // Pre-filled bucket tips over into the next level immediately.
        assert_eq!(structure_transfer_xp(1, 90, 60), (2, 50, 60));
        // Capped structures keep their bucket and spend nothing.
        assert_eq!(structure_transfer_xp(5, 777, 9999), (5, 777, 0));
        assert_eq!(structure_transfer_xp(9, 0, 1), (5, 0, 0), "over-cap rows stop at 5");
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
        // Arrow tower (10 dps) kills a Slime (22 hp) in 3 ticks.
        let slime = mob_kind("Slime").unwrap();
        let tower = defense_kind("ArrowTower").unwrap();
        assert_eq!(slime.hp.div_ceil(tower.dps), 3);
        // Ballista (22) drops a Myconid (70) in 4 ticks.
        let myconid = mob_kind("Myconid").unwrap();
        let ballista = defense_kind("Ballista").unwrap();
        assert_eq!(myconid.hp.div_ceil(ballista.dps), 4);
        // The boss (260) takes 10 ballista shots + change — worth a siege.
        assert_eq!(mob_kind("Big Slime").unwrap().hp / ballista.dps, 11);
    }
}
