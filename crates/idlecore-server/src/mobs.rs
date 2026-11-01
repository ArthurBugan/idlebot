//! Raid waves and mob simulation (Spec 023 §3.5–3.6).
//!
//! `raid_tick` (30s) opens waves on eligible capitals and sweeps stale ones;
//! `tick` (1s) fires towers, marches mobs toward the capital, chews through
//! walls, credits drops, and runs the destruction cascade at HP 0.
//! Everything here is server-authoritative; the client only replays rows.

use spacetimedb::{ReducerContext, Table};
use std::collections::{HashMap, HashSet};
use idlecore_core::defense_config as cfg;
use idlecore_core::hex::HexCoord;

use crate::capital::destroy_capital;
use crate::objects::add_item;
use crate::types::{
    capital, defense_structure, hex_center, hex_coords_of, hex_distance, hex_id_of,
    hex_tile, mob, now_secs, raid_wave, territory, Capital, DefenseStructure, Mob, RaidWave,
};

const MOB_TICK_AUDIT: &str = "mob_tick";
const RAID_TICK_AUDIT: &str = "raid_tick";

// ---------------------------------------------------------------------------
// Wave lifecycle
// ---------------------------------------------------------------------------

/// Border hexes of a territory: unowned, walkable hexes adjacent to an owned
/// hex. Raiders spawn on the frontier and march inward. Deterministic order
/// (sorted) keeps spawn-point picks stable across ticks.
fn border_hexes(ctx: &ReducerContext, capital: &Capital) -> Vec<(i32, i32)> {
    let owned: HashSet<u64> = ctx
        .db
        .territory()
        .territory_by_owner()
        .filter(&capital.owner)
        .map(|t| t.hex_id)
        .collect();
    frontier_hexes(&owned, |hex_id| {
        ctx.db
            .hex_tile()
            .hex_id()
            .find(hex_id)
            .map(|t| t.terrain == "Water")
    })
}

/// Pure frontier computation (Spec 023 §3.5, unit-testable without
/// SpacetimeDB): every unowned hex adjacent to an owned one, in deterministic
/// sorted order. `terrain_of` returns `Some(true)` for water (excluded),
/// `Some(false)` for walkable land, and `None` for unmaterialized hexes
/// (they exist only where players have walked — skipped entirely).
pub fn frontier_hexes(
    owned: &HashSet<u64>,
    terrain_of: impl Fn(u64) -> Option<bool>,
) -> Vec<(i32, i32)> {
    let mut borders: Vec<(i32, i32)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for &hex_id in owned {
        let (q, r) = hex_coords_of(hex_id);
        for (nq, nr) in HexCoord::new(q, r).neighbors() {
            let nid = hex_id_of(nq, nr);
            if owned.contains(&nid) || !seen.insert(nid) {
                continue;
            }
            // Spawn on unowned walkable land only.
            if terrain_of(nid) != Some(false) {
                continue;
            }
            borders.push((nq, nr));
        }
    }
    borders.sort_unstable();
    borders
}

/// Minimum distance (hex rings) between a spawn point and its capital:
/// raiders must visibly MARCH across the frontier instead of popping next
/// to the capital (Spec 023 §3.5, rev4 — legibility).
const MIN_MARCH_HEXES: i32 = 3;

/// Push a spawn point outward (away from the capital) until it sits at
/// least [`MIN_MARCH_HEXES`] away, stepping only onto land.
fn walk_out(ctx: &ReducerContext, capital: &Capital, mut q: i32, mut r: i32) -> (i32, i32) {
    let (cq, cr) = hex_coords_of(capital.hex_id);
    for _ in 0..16 {
        if hex_distance(q, r, cq, cr) >= MIN_MARCH_HEXES {
            break;
        }
        let best = HexCoord::new(q, r)
            .neighbors()
            .into_iter()
            .filter(|&(nq, nr)| {
                ctx.db
                    .hex_tile()
                    .hex_id()
                    .find(hex_id_of(nq, nr))
                    .map(|t| t.terrain != "Water")
                    .unwrap_or(true)
            })
            .max_by_key(|&(nq, nr)| hex_distance(nq, nr, cq, cr));
        match best {
            Some((nq, nr)) => {
                q = nq;
                r = nr;
            }
            None => break, // surrounded by water/stuck: spawn here anyway
        }
    }
    (q, r)
}

/// Open one wave against a capital: spawn `wave_size` mobs composed by
/// level, spread across up to two border points, stamp `last_raid_at`.
/// Returns false when there is nowhere legal to spawn.
fn spawn_wave(ctx: &ReducerContext, capital: &Capital, now: u64) -> bool {
    let claimed = ctx
        .db
        .territory()
        .territory_by_owner()
        .filter(&capital.owner)
        .count();
    let size = cfg::wave_size(claimed, capital.level);
    let composition = cfg::wave_composition(size, capital.level);
    let borders = border_hexes(ctx, capital);
    if borders.is_empty() {
        return false;
    }
    // Up to two spawn points, pushed out to at least MIN_MARCH_HEXES so the
    // wave has a visible trek across the frontier before reaching targets.
    let points: Vec<(i32, i32)> = pick_border_points(&borders)
        .into_iter()
        .map(|(q, r)| walk_out(ctx, capital, q, r))
        .collect();

    let wave = ctx.db.raid_wave().insert(RaidWave {
        wave_id: 0,
        capital_id: capital.capital_id,
        owner: capital.owner.clone(),
        opened_at: now,
        ended_at: None,
        outcome: None,
    });
    for (i, kind_name) in composition.iter().enumerate() {
        let Some(kind) = cfg::mob_kind(kind_name) else { continue };
        let (sq, sr) = points[i % points.len()];
        let (sx, sy) = hex_center(sq, sr);
        ctx.db.mob().insert(Mob {
            mob_id: 0,
            capital_id: capital.capital_id,
            wave_id: wave.wave_id,
            kind: kind.name.to_string(),
            hex_id: hex_id_of(sq, sr),
            hex_q: sq,
            hex_r: sr,
            position_x: sx,
            position_y: sy,
            hp: kind.hp,
            step_accum_secs: 0,
        });
    }
    let mut cap_row = capital.clone();
    cap_row.last_raid_at = now;
    ctx.db.capital().capital_id().update(cap_row);
    tracing::info!(
        "WAVE-OPEN capital={} size={size} points={points:?}",
        capital.capital_id
    );
    true
}

/// Debug helper (Spec 023): force a raid wave against the caller's own
/// capital, ignoring grace/reattack timers. Owner-only — you raid yourself,
/// so this can't grief anyone. Wired to the client debug panel button.
pub fn debug_force_wave(ctx: &ReducerContext, address: &str) -> Result<(), String> {
    use crate::capital::find_capital;
    use crate::economy::find_player;
    let p = find_player(ctx, address).ok_or_else(|| "Player not found".to_string())?;
    let capital = find_capital(ctx, &p.address)
        .filter(|c| !c.destroyed)
        .ok_or_else(|| "No capital to raid — place it first".to_string())?;
    let now = now_secs(ctx);
    if spawn_wave(ctx, &capital, now) {
        crate::scheduler::audit(
            ctx,
            "debug_force_wave",
            format!("capital={}", capital.capital_id),
        );
        Ok(())
    } else {
        Err("No legal spawn point for the wave (frontier all water?)".to_string())
    }
}

/// Close a wave with an outcome marker.
fn close_wave(ctx: &ReducerContext, mut wave: RaidWave, now: u64, outcome: &str) {
    wave.ended_at = Some(now);
    wave.outcome = Some(outcome.to_string());
    ctx.db.raid_wave().wave_id().update(wave);
}

/// Pure spawn gate (Spec 023 §3.6, unit-testable): the wave eligibility part
/// of [`raid_tick`], stripped of reducer access. A capital opens a wave only
/// when no wave is open (single-wave invariant), its grace window has passed,
/// and the reattack cooldown since `last_raid_at` has elapsed.
pub fn wave_eligible(
    now: u64,
    grace_until: u64,
    last_raid_at: u64,
    has_open_wave: bool,
) -> bool {
    if has_open_wave {
        return false;
    }
    if now < grace_until {
        return false;
    }
    now.saturating_sub(last_raid_at) >= cfg::WAVE_REATTACK_SECS
}

/// Pure spawn-point selection from a sorted frontier (Spec 023 rev4):
/// up to two points spread across the list by stride — one for a single-hex
/// frontier, both ends for anything larger.
pub fn pick_border_points(borders: &[(i32, i32)]) -> Vec<(i32, i32)> {
    if borders.is_empty() {
        return Vec::new();
    }
    let take = borders.len().min(2).max(1);
    let stride = borders.len().div_ceil(take);
    borders.iter().step_by(stride).take(take).copied().collect()
}

/// Every 30s: sweep waves that outlived `WAVE_TIMEOUT` (survivors despawn,
/// no drops), then open one wave per eligible capital. Raids hit offline
/// capitals too — only the grace windows protect (user decision, 023 rev2).
pub fn raid_tick(ctx: &ReducerContext) {
    let now = now_secs(ctx);

    let stale: Vec<RaidWave> = ctx
        .db
        .raid_wave()
        .iter()
        .filter(|w| w.ended_at.is_none() && now.saturating_sub(w.opened_at) >= cfg::WAVE_TIMEOUT_SECS)
        .collect();
    for wave in stale {
        for excess in ctx
            .db
            .mob()
            .mob_by_capital()
            .filter(wave.capital_id)
            .filter(|m| m.wave_id == wave.wave_id)
        {
            ctx.db.mob().mob_id().delete(excess.mob_id);
        }
        close_wave(ctx, wave, now, "timeout");
    }

    let capitals: Vec<Capital> = ctx.db.capital().iter().filter(|c| !c.destroyed).collect();
    for capital in capitals {
        let has_open_wave = ctx
            .db
            .raid_wave()
            .wave_by_capital()
            .filter(capital.capital_id)
            .any(|w| w.ended_at.is_none());
        if wave_eligible(now, capital.grace_until, capital.last_raid_at, has_open_wave) {
            spawn_wave(ctx, &capital, now);
        }
    }
    crate::scheduler::audit(
        ctx,
        RAID_TICK_AUDIT,
        format!("raid sweep done at {now}"),
    );
}

// ---------------------------------------------------------------------------
// 1s combat simulation
// ---------------------------------------------------------------------------

/// Pure engagement pick (Spec 023 §3.5, unit-testable): the nearest structure
/// within melee adjacency wins the mob's attention; otherwise the capital
/// when it is adjacent; otherwise both are None (keep marching).
pub fn engagement_target(
    nearby: &[(u64, i32)], // (structure_id, hex distance from mob)
    capital_dist: i32,
) -> (Option<u64>, bool) {
    if let Some(&(sid, _)) = nearby
        .iter()
        .filter(|(_, d)| *d <= 1)
        .min_by_key(|&(id, d)| (d, id))
    {
        return (Some(sid), false);
    }
    if capital_dist <= 1 {
        return (None, true);
    }
    (None, false)
}

/// Every 1 second: tower fire, mob march + melee, damage bookkeeping, wave
/// ends, destruction cascade.
pub fn tick(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    let towers_all: Vec<DefenseStructure> = ctx.db.defense_structure().iter().collect();

    // ---- Tower fire: nearest mob in range per tower takes dps damage ----
    let mut mob_damage: HashMap<u64, u32> = HashMap::new();
    for tower in towers_all.iter() {
        let Some(kind) = cfg::defense_kind(&tower.kind) else { continue };
        if kind.dps == 0 || kind.range_hexes == 0 {
            continue;
        }
        // Spec 024: leveled towers hit harder (+25% dps per level).
        let dps = cfg::structure_dps(kind, tower.level);
        let (tq, tr) = hex_coords_of(tower.hex_id);
        let target = ctx
            .db
            .mob()
            .iter()
            .filter(|m| m.hp > mob_damage.get(&m.mob_id).copied().unwrap_or(0))
            .filter(|m| hex_distance(m.hex_q, m.hex_r, tq, tr) <= kind.range_hexes as i32)
            .min_by_key(|m| (hex_distance(m.hex_q, m.hex_r, tq, tr), m.mob_id));
        if let Some(target) = target {
            *mob_damage.entry(target.mob_id).or_insert(0) += dps;
        }
    }

    // ---- Deaths from tower fire: remove rows + credit drops/xp ----
    let capitals: HashMap<u64, Capital> = ctx
        .db
        .capital()
        .iter()
        .map(|c| (c.capital_id, c))
        .collect();
    let mob_rows: Vec<Mob> = ctx.db.mob().iter().collect();
    for mob in mob_rows.iter() {
        let dmg = mob_damage.get(&mob.mob_id).copied().unwrap_or(0);
        if dmg == 0 {
            continue;
        }
        kill_or_wound(ctx, mob, dmg, &capitals);
    }

    // ---- Mob melee + marching (skip mobs dead this tick) ----
    let mut struct_damage: HashMap<u64, u32> = HashMap::new();
    let mut capital_damage: HashMap<u64, u32> = HashMap::new();
    let tower_hexes: HashMap<u64, (i32, i32)> = towers_all
        .iter()
        .map(|s| (s.structure_id, hex_coords_of(s.hex_id)))
        .collect();

    for mob in mob_rows.iter() {
        if mob_damage.contains_key(&mob.mob_id) {
            continue; // fell to tower fire this tick
        }
        let Some(kind) = cfg::mob_kind(&mob.kind) else {
            ctx.db.mob().mob_id().delete(mob.mob_id);
            continue;
        };
        let Some(capital) = capitals.get(&mob.capital_id) else {
            ctx.db.mob().mob_id().delete(mob.mob_id);
            continue;
        };
        if capital.destroyed {
            ctx.db.mob().mob_id().delete(mob.mob_id);
            continue;
        }
        let (cq, cr) = hex_coords_of(capital.hex_id);
        let capital_dist = hex_distance(mob.hex_q, mob.hex_r, cq, cr);

        let nearby: Vec<(u64, i32)> = tower_hexes
            .iter()
            .map(|(sid, hy)| (*sid, hex_distance(mob.hex_q, mob.hex_r, hy.0, hy.1)))
            .collect();
        let (target_structure, attacking_capital) = engagement_target(&nearby, capital_dist);

        if let Some(sid) = target_structure {
            *struct_damage.entry(sid).or_insert(0) += kind.dps;
        } else if attacking_capital {
            *capital_damage.entry(capital.capital_id).or_insert(0) += kind.dps;
        } else {
            march(ctx, mob, kind.step_interval_secs, cq, cr);
        }
    }

    // Apply structure damage: destroy fallen walls/towers, respawn chests.
    let struct_hits = struct_damage.len();
    let capital_hits = capital_damage.len();
    for (sid, dmg) in struct_damage {
        if let Some(mut s) = ctx.db.defense_structure().structure_id().find(sid) {
            if s.hp > dmg {
                s.hp -= dmg;
                ctx.db.defense_structure().structure_id().update(s);
            } else {
                let kind = cfg::defense_kind(&s.kind);
                // A fallen chest first empties its vault back into the
                // owner's inventory (Spec 024 §2.3 — loss bounded to the
                // build cost).
                if s.kind == cfg::ITEM_STORAGE_CHEST {
                    crate::buildings::refund_chest(ctx, sid, &s.owner);
                }
                ctx.db.defense_structure().structure_id().delete(sid);
                // While the capital survives, leveled structures leave
                // rubble that rebuilds at half their level (min L1).
                if let Some(kind) = kind {
                    if capitals.get(&s.capital_id).is_some_and(|c| !c.destroyed) {
                        let level = crate::buildings::half_level(s.level);
                        let mut reborn = s.clone();
                        reborn.level = level;
                        reborn.total_xp = 0;
                        reborn.hp = cfg::structure_max_hp(kind, level);
                        ctx.db.defense_structure().insert(reborn);
                    }
                }
                tracing::info!("STRUCTURE-FELL structure#{sid}");
            }
        }
    }
    // Apply capital damage; 0 HP triggers the destruction cascade.
    for (cid, dmg) in capital_damage {
        if let Some(caprow) = capitals.get(&cid).cloned() {
            if caprow.hp > dmg {
                let mut s = caprow.clone();

                s.hp -= dmg;

                ctx.db.capital().capital_id().update(s);
            } else {
                destroy_capital(ctx, caprow.clone(), now);
            }
        }
    }

    // Wave end: a wave with no living mobs left is "cleared" if its capital
    // still stands — towers repair to full (MVP rule, Spec 023 §3.4).
    let cleared: Vec<RaidWave> = ctx
        .db
        .raid_wave()
        .iter()
        .filter(|w| w.ended_at.is_none())
        .filter(|w| !ctx.db.mob().iter().any(|m| m.wave_id == w.wave_id))
        .collect();
    for wave in cleared {
        let cid = wave.capital_id;
        let outcome = match capitals.get(&cid) {
            Some(c) if !c.destroyed => "cleared",
            _ => "capital_destroyed",
        };
        close_wave(ctx, wave, now, outcome);
        if outcome == "cleared" {
            // Capital survived: towers stand repaired against their leveled
            // max HP (Spec 023 §3.4 + Spec 024 level scaling).
            for s in ctx.db.defense_structure().structure_by_capital().filter(cid) {
                let Some(kind) = cfg::defense_kind(&s.kind) else { continue };
                let mut row = s.clone();
                row.hp = cfg::structure_max_hp(kind, row.level);
                ctx.db.defense_structure().structure_id().update(row);
            }
        }
    }

    crate::scheduler::audit(
        ctx,
        MOB_TICK_AUDIT,
        format!(
            "tower_shots={} struct_hits={} capital_hits={} at {now}",
            mob_damage.len(),
            struct_hits,
            capital_hits
        ),
    );
}

/// Apply melee/tower damage to a mob; at 0 HP the mob dies — drops go to
/// the capital's defender and capital XP accrues (drop roll deterministic
/// per mob id, Spec 023 §3.3).
fn kill_or_wound(
    ctx: &ReducerContext,
    mob: &Mob,
    damage: u32,
    capitals: &HashMap<u64, Capital>,
) {
    if mob.hp > damage {
        let mut m = mob.clone();

        m.hp -= damage;

        ctx.db.mob().mob_id().update(m);

        return;
    }
    ctx.db.mob().mob_id().delete(mob.mob_id);
    let Some(kind) = cfg::mob_kind(&mob.kind) else { return };
    let Some(capital) = capitals.get(&mob.capital_id) else { return };

    if (u64::from(mob.mob_id) % 100) < u64::from(kind.drop_chance_pct) {
        add_item(ctx, &capital.owner, kind.drop_item, u64::from(kind.drop_qty));
    }
    crate::capital::capital_add_xp(
        ctx,
        capital,
        cfg::CAPITAL_XP_PER_MOB,
        now_secs(ctx),
    );
    tracing::info!(
        "MOB-KILL kind={} wave={} drops={}",
        kind.name,
        mob.wave_id,
        kind.drop_item
    );
}

/// March one hex toward the capital every `interval` seconds (greedy axial
/// descent), pinning the position to the destination hex center so the
/// client glides between centers like it does for remote players.
fn march(ctx: &ReducerContext, mob: &Mob, interval: u32, target_q: i32, target_r: i32) {
    let mut row = mob.clone();
    row.step_accum_secs += 1;
    if row.step_accum_secs < interval {
        ctx.db.mob().mob_id().update(row);
        return;
    }
    let next = step_toward_hex(mob.hex_q, mob.hex_r, target_q, target_r);
    let (x, y) = hex_center(next.0, next.1);
    row.hex_q = next.0;
    row.hex_r = next.1;
    row.hex_id = hex_id_of(next.0, next.1);
    row.position_x = x;
    row.position_y = y;
    row.step_accum_secs = 0;
    ctx.db.mob().mob_id().update(row);
}

/// Pure one-hex greedy step toward a target (unit-tested: distance must
/// strictly decrease; this is how raiders close on the capital).
pub fn step_toward_hex(q: i32, r: i32, target_q: i32, target_r: i32) -> (i32, i32) {
    HexCoord::new(q, r)
        .neighbors()
        .into_iter()
        .min_by_key(|&(nq, nr)| hex_distance(nq, nr, target_q, target_r))
        .unwrap_or((q, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_STRUCTS: [(u64, i32); 0] = [];

    #[test]
    fn mobs_melee_the_nearest_structure_before_the_capital() {
        // A wall one hex away stops the mob even when the capital is
        // adjacent — walls are the shield, priority structure > capital.
        let nearby = [(7u64, 1)];
        let (target, capital) = engagement_target(&nearby, 1);
        assert_eq!(target, Some(7));
        assert!(!capital);
    }

    #[test]
    fn mobs_only_melee_structures_within_one_hex() {
        // A tower 2 hexes away is out of melee reach: keep marching.
        let nearby = [(7u64, 2)];
        let (target, capital) = engagement_target(&nearby, 3);
        assert_eq!(target, None);
        assert!(!capital, "capital 3 hexes away is not reachable");
    }

    #[test]
    fn mobs_hit_the_capital_when_adjacent_and_unobstructed() {
        let (target, capital) = engagement_target(&NO_STRUCTS, 1);
        assert_eq!(target, None);
        assert!(capital);
        // Same hex counts as adjacency (distance 0).
        let (_, capital0) = engagement_target(&NO_STRUCTS, 0);
        assert!(capital0);
        // Nothing in range → both None (keep marching).
        let (t, c) = engagement_target(&NO_STRUCTS, 4);
        assert_eq!(t, None);
        assert!(!c);
    }

    #[test]
    fn ties_pick_the_deterministic_structure() {
        // Two walls both one hex away: the lower id wins, always.
        let nearby = [(99u64, 1), (42u64, 1)];
        let (target, _) = engagement_target(&nearby, 1);
        assert_eq!(target, Some(42));
        // A strictly closer structure beats a lower id at greater distance.
        let near2 = [(1u64, 0), (2u64, 1)];
        let (target2, _) = engagement_target(&near2, 1);
        assert_eq!(target2, Some(1));
    }

    #[test]
    fn march_steps_strictly_toward_the_capital() {
        // From 5 hexes out, each step MUST reduce the distance (raiders never
        // stall — this is the "mobs walk closer" guarantee).
        let mut q = 5;
        let mut r = 0;
        let dist0 = hex_distance(q, r, 0, 0);
        for _ in 0..dist0 {
            let before = hex_distance(q, r, 0, 0);
            let (nq, nr) = step_toward_hex(q, r, 0, 0);
            assert!(
                hex_distance(nq, nr, 0, 0) < before,
                "step ({q},{r}) -> ({nq},{nr}) must close the gap"
            );
            q = nq;
            r = nr;
        }
        assert_eq!((q, r), (0, 0), "march reaches the capital");
        // Diagonal approach from another direction also closes.
        let (mq, mr) = step_toward_hex(-3, 4, 0, 0);
        assert!(hex_distance(mq, mr, 0, 0) < hex_distance(-3, 4, 0, 0));
    }

    // -- T3.2: scheduler invariants ----------------------------------------

    fn capital_at(grace_until: u64, last_raid_at: u64) -> Capital {
        Capital {
            capital_id: 1,
            owner: "addr-A".to_string(),
            hex_id: 0,
            hp: 100,
            level: 1,
            total_xp: 0,
            grace_until,
            placed_at: 0,
            last_raid_at,
            destroyed: false,
        }
    }

    #[test]
    fn wave_grace_suppresses_spawns() {
        let cap = capital_at(5_000, 0);
        // Inside the grace window: no wave opens, whatever the reattack clock.
        assert!(!wave_eligible(4_999, cap.grace_until, cap.last_raid_at, false));
        // The instant grace expires the capital becomes attackable.
        assert!(wave_eligible(5_000, cap.grace_until, cap.last_raid_at, false));
    }

    #[test]
    fn single_wave_is_an_invariant() {
        let cap = capital_at(0, 0);
        // An open wave blocks any second wave, even long after grace.
        assert!(!wave_eligible(999_999, cap.grace_until, cap.last_raid_at, true));
        // Same capital without an open wave is eligible again.
        assert!(wave_eligible(cfg::WAVE_REATTACK_SECS, cap.grace_until, 0, false));
    }

    #[test]
    fn wave_reattack_window_counts_from_last_raid() {
        let cap = capital_at(0, cfg::WAVE_REATTACK_SECS);
        // One second before the cooldown elapses: blocked.
        assert!(!wave_eligible(
            2 * cfg::WAVE_REATTACK_SECS - 1,
            cap.grace_until,
            cap.last_raid_at,
            false
        ));
        // Exactly at the boundary: allowed.
        assert!(wave_eligible(
            2 * cfg::WAVE_REATTACK_SECS,
            cap.grace_until,
            cap.last_raid_at,
            false
        ));
        // A brand-new capital (never raided) is eligible immediately after grace.
        assert!(wave_eligible(
            cfg::WAVE_REATTACK_SECS.max(60),
            cap.grace_until,
            capital_at(60, 0).last_raid_at,
            false
        ));
    }

    #[test]
    fn frontier_hexes_skip_water_unowned_unmaterialized_and_duplicates() {
        use std::collections::HashSet;
        let owned: HashSet<u64> = [hex_id_of(0, 0), hex_id_of(1, 0)]
            .into_iter()
            .collect();
        // Frontier candidates around the two owned hexes: (1,-1) / (2,-1) /
        // (2,0) / (0,-1) / (-1,0) / (-1,1) land, (0,1) water, (1,1)
        // unmaterialized (None → skipped).
        let terrain = |id: u64| -> Option<bool> {
            let c = HexCoord::from_id(id);
            if (c.q, c.r) == (0, 1) {
                return Some(true);
            }
            if (c.q, c.r) == (1, 1) {
                return None;
            }
            Some(false)
        };
        let borders = frontier_hexes(&owned, terrain);
        assert_eq!(
            borders,
            vec![(-1, 0), (-1, 1), (0, -1), (1, -1), (2, -1), (2, 0)],
            "sorted, water and unmaterialized skipped, shared frontier deduped"
        );
        // Every border is unowned and truly adjacent to an owned hex.
        for &(q, r) in &borders {
            assert!(!owned.contains(&hex_id_of(q, r)), "border ({q},{r}) must be unowned");
            let is_adjacent = HexCoord::new(q, r)
                .neighbors()
                .iter()
                .any(|&(nq, nr)| owned.contains(&hex_id_of(nq, nr)));
            assert!(is_adjacent, "border ({q},{r}) not adjacent to owned land");
        }
    }

    #[test]
    fn pick_border_points_strides_two_ends() {
        // Single-hex frontier → exactly one spawn point.
        assert_eq!(pick_border_points(&[(3, 4)]), vec![(3, 4)]);
        // Two hexes → both, no stride gap.
        assert_eq!(pick_border_points(&[(0, 2), (2, 0)]), vec![(0, 2), (2, 0)]);
        // Large frontier → two points: first + stride end.
        let five: Vec<(i32, i32)> = (0..5).map(|i| (i, 0)).collect();
        assert_eq!(pick_border_points(&five), vec![(0, 0), (3, 0)]);
        assert_eq!(pick_border_points(&[]), Vec::<(i32, i32)>::new());
    }
}
