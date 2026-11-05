//! Raid waves and mob simulation (Spec 023 §3.5–3.6).
//!
//! `raid_tick` (30s) opens waves on eligible capitals and sweeps stale ones;
//! `tick` (1s) fires towers, marches mobs toward the capital, chews through
//! walls, credits drops, and runs the destruction cascade at HP 0.
//! Everything here is server-authoritative; the client only replays rows.

use spacetimedb::{ReducerContext, Table};
use std::collections::{HashMap, HashSet};
use idlecore_core::defense_config as cfg;
use idlecore_core::grid::TileCoord;

use crate::capital::destroy_capital;
use crate::combat::{kill_or_wound};
use crate::types::{
    capital, combat_event, defense_structure, tile_center, hex_coords_of, tile_distance,
    tile_id_of, hex_tile, mob, now_secs, raid_wave, territory, Capital, DefenseStructure, Mob,
    RaidWave, CE_BOMB, CE_CAPITAL_HIT, CE_MELEE, CE_SHOT, CE_SPLASH, CE_STRUCT_FELL, CE_TRAP,
};

const MOB_TICK_AUDIT: &str = "mob_tick";
const RAID_TICK_AUDIT: &str = "raid_tick";

// ---------------------------------------------------------------------------
// Wave lifecycle
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The path (Spec 035) — raiders walk the fixed dirt line
// ---------------------------------------------------------------------------

/// March target for one raider on the FIXED path (pure, unit-testable):
/// `path` is the stamped trace, spawn hex first, the capital hex last.
///
/// REV (lane discipline): the raider carrying `path_idx` must reach
/// `path[path_idx]` IN ORDER before advancing further. Standing ON the
/// trace is progress — the position itself derives the claimed node, so a
/// shove onto a later node just continues from there (on the lane).
/// OFF-trace (a knock or a player chase): the target is the NEAREST node
/// at or before the claimed index — never a later one — so the walk back
/// cannot cut a bend across the grass or skip ahead. Ties → spawn side.
/// CORNER clamp (playtest): landing PAST the claim (a shove or a chase
/// hop onto a later node near a fold) must not jump progress — the
/// target is the previous node, so the skipped bend is re-tread.
///
/// An empty path = legacy capital without a stamped trace → the caller
/// keeps the old free march at the capital hex itself.
pub fn raid_lane_target(
    mob_q: i32,
    mob_r: i32,
    path: &[(i32, i32)],
    path_idx: u32,
) -> (usize, (i32, i32)) {
    if path.is_empty() {
        // Defensive: the caller keeps the legacy free march for a traceless
        // capital; this branch only exists so the pure fn can't panic.
        return (0, (mob_q, mob_r));
    }
    let last = path.len().saturating_sub(1);
    let cap = (path_idx as usize).min(last);
    // Standing ON the trace: walk the nodes IN ORDER.
    if let Some(j) = path.iter().position(|&n| n == (mob_q, mob_r)) {
        if j > cap {
            // Landed past the claim: step back to the previous node and
            // walk the lane back down to the claim — never skip ahead.
            return (cap, path[j - 1]);
        }
        let next = (j + 1).min(last);
        return (next, path[next]);
    }
    // Off-trace: the nearest node at or BEFORE the claimed index.
    let mut best: (i32, usize) = (i32::MAX, 0);
    for (i, &(nq, nr)) in path[..=cap].iter().enumerate() {
        let d = tile_distance(mob_q, mob_r, nq, nr);
        if (d, i) < best {
            best = (d, i);
        }
    }
    (best.1, path[best.1])
}

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
    frontier_tiles(&owned, |hex_id| {
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
pub fn frontier_tiles(
    owned: &HashSet<u64>,
    terrain_of: impl Fn(u64) -> Option<bool>,
) -> Vec<(i32, i32)> {
    let mut borders: Vec<(i32, i32)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for &hex_id in owned {
        let (q, r) = hex_coords_of(hex_id);
        for (nq, nr) in TileCoord::new(q, r).neighbors() {
            let nid = tile_id_of(nq, nr);
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
        if tile_distance(q, r, cq, cr) >= MIN_MARCH_HEXES {
            break;
        }
        let best = TileCoord::new(q, r)
            .neighbors()
            .into_iter()
            .filter(|&(nq, nr)| {
                ctx.db
                    .hex_tile()
                    .hex_id()
                    .find(tile_id_of(nq, nr))
                    .map(|t| t.terrain != "Water")
                    .unwrap_or(true)
            })
            .max_by_key(|&(nq, nr)| tile_distance(nq, nr, cq, cr));
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
    // Boss cadence (Spec 025 §4): every 4th wave leads with a Big Slime.
    let wave_index = ctx
        .db
        .raid_wave()
        .wave_by_capital()
        .filter(capital.capital_id)
        .count() as u64;
    // REV3 §14: named threat pattern first (hand-tuned ladder), plain
    // kind-cycle fallback when the index fires no pattern; the boss rule
    // still overrides the lead slot.
    let mut composition: Vec<&'static str> =
        cfg::wave_pattern(wave_index, size, capital.level)
            .map(|(_, comp)| comp)
            .unwrap_or_else(|| cfg::wave_composition(size, capital.level));
    if cfg::boss_wave(wave_index, capital.level) {
        if composition.is_empty() {
            composition.push("Big Slime");
        } else {
            composition[0] = "Big Slime";
        }
    }
    // Rooted blooms (Spec 025 §4) sprout inside the territory, at ≤2 hexes
    // from their capital, instead of marching over the frontier.
    let (cq0, cr0) = hex_coords_of(capital.hex_id);
    let mut inner_spots: Vec<(i32, i32)> = ctx
        .db
        .territory()
        .territory_by_owner()
        .filter(&capital.owner)
        .map(|t| hex_coords_of(t.hex_id))
        .filter(|&(q, r)| tile_distance(q, r, cq0, cr0) <= 2)
        .collect();
    inner_spots.sort_unstable();
    let mut inner_iter = inner_spots.into_iter();

    // Spec 035 S3: classic TD — every raider enters through the capital's
    // FIXED spawn point (stamped at placement). REV: presence of a stamped
    // march trace (empty = legacy) decides; (0, 0) is a legal spawn hex.
    let points: Vec<(i32, i32)> = if !capital.path_hexes.is_empty() {
        vec![(capital.spawn_q, capital.spawn_r)]
    } else {
        let borders = border_hexes(ctx, capital);
        if borders.is_empty() {
            return false;
        }
        pick_border_points(&borders)
            .into_iter()
            .map(|(q, r)| walk_out(ctx, capital, q, r))
            .collect()
    };

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
        // Rooted kinds spawn on an inner territory hex; everyone else on a
        // pushed-out border point. REV: raiders pop in STAGGERED — raider i
        // launches `i` ticks after the wave opened, so each previous mob
        // already marched one hex down the lane (a walking line, no pile).
        let (sq, sr, spawn_at) = if kind.step_interval_secs == 0 {
            let (iq, ir) = inner_iter.next().unwrap_or(points[i % points.len()]);
            (iq, ir, now)
        } else {
            (
                points[i % points.len()].0,
                points[i % points.len()].1,
                now + i as u64,
            )
        };
        let (sx, sy) = tile_center(sq, sr);
        ctx.db.mob().insert(Mob {
            mob_id: 0,
            capital_id: capital.capital_id,
            wave_id: wave.wave_id,
            kind: kind.name.to_string(),
            hex_id: tile_id_of(sq, sr),
            hex_q: sq,
            hex_r: sr,
            position_x: sx,
            position_y: sy,
            hp: kind.hp,
            step_accum_secs: 0,
            pinned_until_secs: 0,
            aggro_target: String::new(),
            aggro_until_secs: 0,
            pack_id: 0,
            born_at_secs: 0,
            spawn_at_secs: spawn_at,
            // Lane discipline: the mob stands on path[0] and must reach
            // path[1] next (legacy capitals without a trace start flat 0).
            path_idx: if capital.path_hexes.is_empty() { 0 } else { 1 },
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

/// Slot sentinel (REV3 §10 fx table): the event lands at the hex center,
/// not on one of the 4-per-tile slot cells.
const SLOT_CENTER_X: i32 = -1;
const SLOT_CENTER_Y: i32 = -1;

/// Public one-liner for the tower-fire tracer event (REV3 §10): the tick
/// loop is the only caller, kept on `mobs` to reuse the same table.
pub(crate) fn emit_tower_event(
    ctx: &ReducerContext,
    tower: &crate::types::DefenseStructure,
    target_hex_id: u64,
    now: u64,
) {
    let wave_id = ctx
        .db
        .raid_wave()
        .wave_by_capital()
        .filter(tower.capital_id)
        .find(|w| w.ended_at.is_none())
        .map(|w| w.wave_id)
        .unwrap_or(0);
    ctx.db.combat_event().insert(crate::types::CombatEvent {
        event_id: 0,
        capital_id: tower.capital_id,
        wave_id,
        tick_at: now,
        kind: CE_SHOT.to_string(),
        attacker_mob: 0, // 0 = tower shot (client discriminator)
        target_hex_id,
        target_slot_x: SLOT_CENTER_X,
        target_slot_y: SLOT_CENTER_Y,
        detail: "tower".to_string(),
    });
}

/// Event tag per attacker (§13 fx table): the bomb lobs, ranged kinds shoot
/// their column, everyone else melees.
fn attack_kind_tag(kind: &cfg::MobKind, target_is_wall: bool) -> &'static str {
    let _ = target_is_wall;
    if kind.splash {
        CE_BOMB
    } else if kind.range_hexes > 1 {
        CE_SHOT
    } else {
        CE_MELEE
    }
}

/// Write one combat_event row (Spec 025 REV3 §10): the client's fx feed.
/// Fire-and-forget — never part of a simulation result.
fn emit_event(
    ctx: &ReducerContext,
    capital_id: u64,
    wave_id: u64,
    now: u64,
    kind: &str,
    attacker_mob: u64,
    target_hex_id: u64,
    target_slot: (i32, i32),
    detail: &str,
) {
    ctx.db.combat_event().insert(crate::types::CombatEvent {
        event_id: 0,
        capital_id,
        wave_id,
        tick_at: now,
        kind: kind.to_string(),
        attacker_mob,
        target_hex_id,
        target_slot_x: target_slot.0,
        target_slot_y: target_slot.1,
        detail: detail.to_string(),
    });
}

/// Ring offsets for melee encircle spots (§12) — deterministic order around
/// a target tile, wind-rose with corners first.
const ENCIRCLE_RING: [(i32, i32); 8] = [
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
];

/// Pure encircle pick (§12): the `rank`-th melee attacker of one target
/// takes the `rank`-th ring tile (wrapping) — a swarm reads as surrounding
/// the target instead of stacking into one pixel.
pub fn encircle_spot(rank: u32) -> (i32, i32) {
    ENCIRCLE_RING[(rank % ENCIRCLE_RING.len() as u32) as usize]
}

/// Pure blocked-aware greedy step (REV3 §11): of the candidate neighbors,
/// tiles in `blocked` (standing walls, the capital hex) are unpas­sable; of
/// the rest the one nearest the target wins, and ONLY strictly-closing
/// steps count (a fully walled ring returns `None` → the caller chews the
/// nearest wall instead of bubbling around the plaza forever).
pub fn step_toward_hex_avoiding(
    q: i32,
    r: i32,
    target_q: i32,
    target_r: i32,
    blocked: &std::collections::HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    let cur = tile_distance(q, r, target_q, target_r);
    TileCoord::new(q, r)
        .neighbors()
        .into_iter()
        .filter(|&(nq, nr)| !blocked.contains(&(nq, nr)))
        .filter(|&(nq, nr)| tile_distance(nq, nr, target_q, target_r) < cur)
        .min_by_key(|&(nq, nr)| tile_distance(nq, nr, target_q, target_r))
}

/// Pure nearest-wall pick (§11): when every greedy step is blocked, the mob
/// attacks the wall hex nearest to it within `max_dist` (ties → nearest to
/// its march target → lowest structure id). Fully integer, no RNG. A `max`
/// of 1 keeps the chew in melee reach — nothing chews from afar.
pub fn nearest_wall_target(
    walls: &[(u64, i32, i32)], // (structure_id, hex q, hex r)
    from: (i32, i32),
    target: (i32, i32),
    max_dist: i32,
) -> Option<u64> {
    walls
        .iter()
        .copied()
        .filter(|&(_, wq, wr)| tile_distance(from.0, from.1, wq, wr) <= max_dist)
        .min_by_key(|&(sid, wq, wr)| {
            (
                tile_distance(from.0, from.1, wq, wr),
                tile_distance(wq, wr, target.0, target.1),
                sid,
            )
        })
        .map(|(sid, _, _)| sid)
}

/// Pure engagement pick (Spec 023 §3.5 + Spec 025 §4, unit-testable): the
/// nearest structure within the mob's attack `range` wins the mob's
/// attention; otherwise the capital when within range; otherwise both are
/// None (keep marching). Melee kinds pass 1, ranged kinds their reach.
pub fn engagement_target(
    nearby: &[(u64, i32)], // (structure_id, hex distance from mob)
    capital_dist: i32,
    range: i32,
) -> (Option<u64>, bool) {
    if let Some(&(sid, _)) = nearby
        .iter()
        .filter(|(_, d)| *d <= range)
        .min_by_key(|&(id, d)| (d, id))
    {
        return (Some(sid), false);
    }
    // REV (Spec 035 playtest): the capital is a CONTACT target — every kind,
    // melee or ranged, must WALK the dirt path until it touches the capital
    // (adjacent hex / the capital hex itself) before it can damage it.
    // Tower stand-offs keep `range`; the capital does not.
    if capital_dist <= 1 {
        return (None, true);
    }
    (None, false)
}

/// Pure sapper damage (Spec 025 §4): a wall-bonus kind pays `wall_dps` extra
/// while chewing a wall structure (the chest counts as a wall too? No — the
/// `is_wall` flag gates it; only barricades are chew-through targets).
pub fn sapper_dps(base: u32, wall_dps: u32, target_is_wall: bool) -> u32 {
    if target_is_wall {
        base + wall_dps
    } else {
        base
    }
}

/// Pure splash pick (Spec 025 §4): every OTHER structure within 1 hex of the
/// attacker (deterministic ascending structure-id order).
pub fn splash_targets(
    target: u64,
    nearby: &[(u64, i32)], // (structure_id, hex distance from the attacker)
) -> Vec<u64> {
    let mut hits: Vec<u64> = nearby
        .iter()
        .filter(|(id, d)| *id != target && *d <= 1)
        .map(|(id, _)| *id)
        .collect();
    hits.sort_unstable();
    hits
}

/// Pure attack-damage helper: `kind.dps` (+ wall bonus when the victim is a
/// wall), i.e. `sapper_dps` specialized on a `MobKind`.
pub fn engagement_dps(kind: &cfg::MobKind, target_is_wall: bool) -> u32 {
    sapper_dps(kind.dps, kind.wall_dps, target_is_wall)
}

/// Every 1 second: player/roamer passes (Spec 027 combat), tower fire, mob
/// march + melee, damage bookkeeping, wave ends, destruction cascade.
pub fn tick(ctx: &ReducerContext) {
    crate::combat::tick(ctx);
    let now = now_secs(ctx);
    let towers_all: Vec<DefenseStructure> = ctx.db.defense_structure().iter().collect();    // REV4 traps: burn zones tick dps to EVERY raider in range for a fuel
    // budget (hp), while spike/web traps are consumed the moment a raider
    // steps onto their hex (handled in the march pass below).
    let traps_burn: Vec<DefenseStructure> = towers_all
        .iter()
        .filter(|s| {
            cfg::defense_kind(&s.kind)
                .is_some_and(|k| k.burn_secs > 0 && k.dps > 0)
        })
        .cloned()
        .collect();
    let traps_step: Vec<(u64, i32, i32, u32, u32, &'static str)> = towers_all
        .iter()
        .filter(|s| {
            cfg::defense_kind(&s.kind).is_some_and(|k| k.trap_dmg > 0 || k.pin_secs > 0)
        })
        .map(|s| {
            let (q, r) = hex_coords_of(s.hex_id);
            let k = cfg::defense_kind(&s.kind).unwrap();
            (
                s.structure_id,
                q,
                r,
                k.trap_dmg,
                k.pin_secs,
                k.name,
            )
        })
        .collect();

    // ---- Tower fire: nearest mob in range per tower takes dps damage ----
    let mut mob_damage: HashMap<u64, u32> = HashMap::new();
    // Spec 027 §7: who gets the drop if this damage kills a roamer —
    // towers/traps write their owner here (raid mobs never read it).
    let mut damage_credit: HashMap<u64, String> = HashMap::new();
    for tower in towers_all.iter() {
        // Traps are excluded: they trigger by proximity, they never snipe.
        if cfg::defense_kind(&tower.kind).is_none_or(|k| k.dps == 0 || k.burn_secs > 0) {
            continue;
        }
        let kind = cfg::defense_kind(&tower.kind).unwrap();
        if kind.range_hexes == 0 {
            continue;
        }
        // Spec 024: leveled towers hit harder (+25% dps per level).
        let (tq, tr) = hex_coords_of(tower.hex_id);
        let target = ctx
            .db
            .mob()
            .iter()
            .filter(|m| m.hp > mob_damage.get(&m.mob_id).copied().unwrap_or(0))
            .filter(|m| crate::types::mob_is_active(m, now))
            .filter(|m| tile_distance(m.hex_q, m.hex_r, tq, tr) <= kind.range_hexes as i32)
            .min_by_key(|m| (tile_distance(m.hex_q, m.hex_r, tq, tr), m.mob_id));
        if let Some(target) = target {
            *mob_damage.entry(target.mob_id).or_insert(0) += cfg::structure_dps(kind, tower.level);
            // Roamer kills credit the tower owner (Spec 027 §7).
            damage_credit
                .entry(target.mob_id)
                .or_insert_with(|| tower.owner.clone());
            // Tracer fx (REV3 §10/§13): towers shoot too; the client tells
            // tower shots from mob shots by attacker_mob == 0.
            crate::mobs::emit_tower_event(ctx, tower, target.hex_id, now);
        }
    }
    // Burn traps hit every raider in range each second, spending 1 hp of
    // fuel per tick; at 0 fuel the fire burns out.
    for trap in traps_burn.iter() {
        let kind = cfg::defense_kind(&trap.kind).unwrap();
        let (tq, tr) = hex_coords_of(trap.hex_id);
        let dps = cfg::structure_dps(kind, trap.level);
        for m in ctx.db.mob().iter().filter(|m| {
            crate::types::mob_is_active(m, now)
                && tile_distance(m.hex_q, m.hex_r, tq, tr) <= kind.range_hexes as i32
        }) {
            *mob_damage.entry(m.mob_id).or_insert(0) += dps;
            // Roamer kills credit the trap owner (Spec 027 §7).
            damage_credit
                .entry(m.mob_id)
                .or_insert_with(|| trap.owner.clone());
            // One impact bloom on the trap hex speaks for the zone.
            emit_event(
                ctx,
                trap.capital_id,
                0,
                now,
                CE_TRAP,
                0,
                trap.hex_id,
                (SLOT_CENTER_X, SLOT_CENTER_Y),
                "fire",
            );
        }
        if trap.hp > 1 {
            let mut row = trap.clone();
            row.hp -= 1;
            ctx.db.defense_structure().structure_id().update(row);
        } else {
            ctx.db.defense_structure().structure_id().delete(trap.structure_id);
            // Burnout crumble fx: same event kind the client already renders.
            emit_event(
                ctx,
                trap.capital_id,
                0,
                now,
                CE_STRUCT_FELL,
                0,
                trap.hex_id,
                (trap.slot_x, trap.slot_y),
                &trap.kind,
            );
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
        kill_or_wound(ctx, mob, dmg, &capitals, damage_credit.get(&mob.mob_id).map(|s| s.as_str()));
    }

    // ---- Mob melee + marching (skip mobs dead this tick) ----
    let mut struct_damage: HashMap<u64, u32> = HashMap::new();
    let mut capital_damage: HashMap<u64, u32> = HashMap::new();
    let tower_hexes: HashMap<u64, (i32, i32, bool)> = towers_all
        .iter()
        // REV4 traps are NOT melee targets: they trigger by step/burn and
        // are consumed; raiders march straight onto them.
        .filter(|s| {
            cfg::defense_kind(&s.kind)
                .is_some_and(|k| !cfg::is_trap(k))
        })
        .map(|s| {
            let (q, r) = hex_coords_of(s.hex_id);
            (
                s.structure_id,
                (q, r, cfg::defense_kind(&s.kind).map(|k| k.is_wall).unwrap_or(false)),
            )
        })
        .collect();

    // Standing walls are impassable (Spec 025 REV3 §11): a march step onto a
    // wall hex is rejected and the mob chews that wall instead. The capital
    // hex is blocked too — attackers stop on the ring, not on the roof.
    let blocked: std::collections::HashSet<(i32, i32)> = tower_hexes
        .values()
        .filter(|(_, _, w)| *w)
        .map(|(hq, hr, _)| (*hq, *hr))
        .chain(
            /* riding the capital hex looks wrong */
            capitals.values().map(|c| {
                let (q, r) = hex_coords_of(c.hex_id);
                (q, r)
            }),
        )
        .collect();
    // Frost slow aura spots (REV4) — (hex, radius) per built frost tower.
    let frost_spots: Vec<(i32, i32, u32)> = towers_all
        .iter()
        .filter(|s| {
            cfg::defense_kind(&s.kind)
                .is_some_and(|k| k.slow_radius > 0)
        })
        .map(|s| {
            let (q, r) = hex_coords_of(s.hex_id);
            (
                q,
                r,
                cfg::defense_kind(&s.kind).map(|k| k.slow_radius).unwrap_or(0),
            )
        })
        .collect();

    // Pass 1 — engagement ranks for the encircle ring (REV3 §12): melee
    // attackers of one target take deterministic ring spots by mob id.
    let mut rings: HashMap<u64, u32> = HashMap::new();
    {
        let mut counts: HashMap<(u64, bool), u32> = HashMap::new();
        for mob in mob_rows
            .iter()
            .filter(|m| !mob_damage.contains_key(&m.mob_id))
            // Roamers + aggroed hunters never encircle structures (§7/§8).
            .filter(|m| !(m.capital_id == 0 || m.aggro_until_secs > now))
        {
            let Some(kind) = cfg::mob_kind(&mob.kind) else { continue };
            if kind.range_hexes != 1 {
                continue; // ranged kinds hold their stand-off, no ring
            }
            let Some(capital) = capitals.get(&mob.capital_id) else { continue };
            if capital.destroyed {
                continue;
            }
            let (cq, cr) = hex_coords_of(capital.hex_id);
            let nearby: Vec<(u64, i32)> = tower_hexes
                .iter()
                .map(|(sid, (hq, hr, _))| (*sid, tile_distance(mob.hex_q, mob.hex_r, *hq, *hr)))
                .collect();
            let (target_structure, attacking_capital) =
                engagement_target(&nearby, tile_distance(mob.hex_q, mob.hex_r, cq, cr), 1);
            if target_structure.is_none() && !attacking_capital {
                continue;
            }
            let key = (target_structure.unwrap_or(0), attacking_capital);
            let rank = counts.get(&key).copied().unwrap_or(0);
            counts.insert(key, rank + 1);
            rings.insert(mob.mob_id, rank);
        }
    }

    // Spec 035 REV: decode each capital's stamped march trace once per tick.
    let path_nodes: HashMap<u64, Vec<(i32, i32)>> = capitals
        .values()
        .map(|c| {
            (
                c.capital_id,
                c.path_hexes.iter().map(|&h| hex_coords_of(h)).collect(),
            )
        })
        .collect();

    for mob in mob_rows.iter() {
        if mob_damage.contains_key(&mob.mob_id) {
            continue; // fell to tower fire this tick
        }
        if !crate::types::mob_is_active(mob, now) {
            continue; // line spawn REV: not launched yet — no logic at all
        }
        // Roamers (Spec 027 §7) marched/striked in `combat::roamer_pass` —
        // the raid pipeline owns capital-raiders only.
        if mob.capital_id == 0 {
            continue;
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
        // Spec 027 §8: an aggroed raider hunts its player — structures and
        // the capital lose its attention (opt-in retaliation, 20 s stamp).
        // Attack-freeze: a landed swing holds this tick's chase step — raiders
        // strike OR move, never both on the same tick.
        if let Some((pq, pr)) = crate::combat::hunting_player_hex(ctx, mob, now) {
            let struck = crate::combat::raider_strike(ctx, mob, now);
            if !struck && kind.step_interval_secs > 0 {
                let frozen = frost_spots.iter().any(|(hx, hr, radius)| {
                    tile_distance(mob.hex_q, mob.hex_r, *hx, *hr) <= *radius as i32
                });
                // Frost aura (REV4): doubled cadence — hunters included.
                let interval = if frozen {
                    kind.step_interval_secs * 2
                } else {
                    kind.step_interval_secs
                };
                match march(ctx, mob, interval, pq, pr, &blocked, &traps_step) {
                    MarchOutcome::Stepped => {}
                    MarchOutcome::WalledIn(_) => {
                        // Walled in while hunting: the wall eats the swing
                        // time (no dps — walls don't bleed from a chase).
                    }
                    MarchOutcome::Trapped {
                        structure_id,
                        hex_id,
                        trap_dmg,
                        pin_secs,
                        trap_kind,
                    } => {
                        // Credit a roamer-killing trap to its owner (§7).
                        let owner = towers_all
                            .iter()
                            .find(|s| s.structure_id == structure_id)
                            .map(|s| s.owner.clone());
                        if let Some(t) = ctx.db.defense_structure().structure_id().find(structure_id) {
                            ctx.db.defense_structure().structure_id().delete(t.structure_id);
                        }
                        emit_event(
                            ctx,
                            capital.capital_id,
                            mob.wave_id,
                            now,
                            CE_TRAP,
                            mob.mob_id,
                            hex_id,
                            (SLOT_CENTER_X, SLOT_CENTER_Y),
                            trap_kind,
                        );
                        if trap_dmg > 0 {
                            let mut fallen = mob.clone();
                            fallen.hex_id = hex_id;
                            kill_or_wound(ctx, &fallen, trap_dmg, &capitals, owner.as_deref());
                            continue;
                        }
                        if pin_secs > 0 {
                            if let Some(mut fresh) = ctx.db.mob().mob_id().find(mob.mob_id) {
                                fresh.pinned_until_secs = now + u64::from(pin_secs);
                                ctx.db.mob().mob_id().update(fresh);
                            }
                        }
                    }
                }
            }
            continue; // rings/repositioning don't apply while hunting
        }
        let (cq, cr) = hex_coords_of(capital.hex_id);
        let capital_dist = tile_distance(mob.hex_q, mob.hex_r, cq, cr);

        let nearby: Vec<(u64, i32)> = tower_hexes
            .iter()
            .map(|(sid, (hq, hr, _))| (*sid, tile_distance(mob.hex_q, mob.hex_r, *hq, *hr)))
            .collect();
        let (target_structure, attacking_capital) =
            engagement_target(&nearby, capital_dist, kind.range_hexes);

        // Attack-event metadata (REV3 §10): which fx kind + payload detail.
        let event = |kind_tag: &str, detail: &str, tgt: (u64, (i32, i32))| {
            emit_event(
                ctx,
                capital.capital_id,
                mob.wave_id,
                now,
                kind_tag,
                mob.mob_id,
                tgt.0,
                tgt.1,
                detail,
            );
        };

        if let Some(sid) = target_structure {
            // Sapper (Spec 025 §4): +wall_dps while chewing a wall.
            let is_wall = tower_hexes.get(&sid).is_some_and(|(_, _, w)| *w);
            let dmg = engagement_dps(kind, is_wall);
            *struct_damage.entry(sid).or_insert(0) += dmg;
            let (tq, tr) = tower_hexes
                .get(&sid)
                .map(|&(hq, hr, _)| (hq, hr))
                .unwrap_or((mob.hex_q, mob.hex_r));
            event(
                attack_kind_tag(kind, is_wall),
                kind.name,
                (tile_id_of(tq, tr), (SLOT_CENTER_X, SLOT_CENTER_Y)),
            );
            // Bomb (Spec 025 §4): splash half dps onto every OTHER structure
            // adjacent to the mob.
            if kind.splash {
                let splash = (kind.dps / 2).max(1);
                for other in splash_targets(sid, &nearby) {
                    *struct_damage.entry(other).or_insert(0) += splash;
                    let (sq2, sr2) = tower_hexes
                        .get(&other)
                        .map(|&(hq, hr, _)| (hq, hr))
                        .unwrap_or((mob.hex_q, mob.hex_r));
                    event(
                        CE_SPLASH,
                        kind.name,
                        (tile_id_of(sq2, sr2), (SLOT_CENTER_X, SLOT_CENTER_Y)),
                    );
                }
            }
        } else if attacking_capital {
            *capital_damage.entry(capital.capital_id).or_insert(0) += kind.dps;
            event(
                attack_kind_tag(kind, false),
                kind.name,
                (capital.hex_id, (SLOT_CENTER_X, SLOT_CENTER_Y)),
            );
        } else if kind.step_interval_secs > 0 {
            // Web-trap pin (REV4): a pinned raider holds its hex.
            if mob.pinned_until_secs > now {
                continue;
            }
            // Frost aura (REV4): raiders inside any frost tower's slow
            // radius march every other tick.
            let interval = if frost_spots.iter().any(|(hx, hr, radius)| {
                tile_distance(mob.hex_q, mob.hex_r, *hx, *hr) <= *radius as i32
            }) {
                kind.step_interval_secs * 2
            } else {
                kind.step_interval_secs
            };
            // Rooted kinds (Venom Bloom, step 0) never march. Otherwise
            // march; a fully walled-in mob chews the nearest wall (REV3 §11).
            // A step onto a trap hex (REV4) springs the trap instead.
            // Spec 035 S2: raiders march the FIXED stamped trace — the
            // march target is the next trace hex, so `march()`'s
            // strictly-closing greedy can only step onto it (walls/traps
            // machinery intact). REV: the trace is the stamped curved path,
            // not the straight spawn→capital line.
            let path_slice = path_nodes
                .get(&capital.capital_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            // REV (lane discipline): the raider advances through the trace
            // nodes IN ORDER, carrying its claimed next index (persisted on
            // the Mob). Off-trace returns stay at or before that index.
            let (lane_mob, mq, mr) = if path_slice.is_empty() {
                (mob.clone(), cq, cr)
            } else {
                let (idx, (tq, tr)) =
                    raid_lane_target(mob.hex_q, mob.hex_r, path_slice, mob.path_idx);
                let mut m2 = mob.clone();
                m2.path_idx = idx as u32;
                // march() rewrites the whole row (step accumulation), so the
                // claimed index persists with the same write.
                (m2, tq, tr)
            };
            match march(ctx, &lane_mob, interval, mq, mr, &blocked, &traps_step) {
                MarchOutcome::Stepped => {}
                MarchOutcome::WalledIn(Some(sid)) => {
                    *struct_damage.entry(sid).or_insert(0) += engagement_dps(kind, true);
                    event(
                        CE_MELEE,
                        kind.name,
                        (mob.hex_id, (SLOT_CENTER_X, SLOT_CENTER_Y)),
                    );
                }
                MarchOutcome::WalledIn(None) => {}
                // REV4: a step onto a trap hex springs it — spike bursts
                // kill/wound the raider, a web pins it; either consumes the
                // trap (cleared by hand here so the rubble pass skips it).
                MarchOutcome::Trapped {
                    structure_id,
                    hex_id,
                    trap_dmg,
                    pin_secs,
                    trap_kind,
                } => {
                    if let Some(t) = ctx.db.defense_structure().structure_id().find(structure_id) {
                        ctx.db.defense_structure().structure_id().delete(t.structure_id);
                    }
                    event(
                        CE_TRAP,
                        trap_kind,
                        (hex_id, (SLOT_CENTER_X, SLOT_CENTER_Y)),
                    );
                    if trap_dmg > 0 {
                        let mut fallen = mob.clone();
                        fallen.hex_id = hex_id;
                        kill_or_wound(ctx, &fallen, trap_dmg, &capitals, None);
                        continue; // dead this tick — skip the ring reposition
                    }
                    if pin_secs > 0 {
                        if let Some(mut fresh) = ctx.db.mob().mob_id().find(mob.mob_id) {
                            fresh.pinned_until_secs = now + pin_secs as u64;
                            ctx.db.mob().mob_id().update(fresh);
                        }
                    }
                }
            }
        }
        // Encircle reposition (REV3 §12): melee attackers within one step of
        // their ring spot take it (the client glides). Only marching melee
        // kinds get ringed; ranged hold their stand-off.
        if let Some(rank) = rings.get(&mob.mob_id).copied() {
            let (tq, tr) = if attacking_capital {
                (cq, cr)
            } else if let Some(sid) = target_structure {
                tower_hexes
                    .get(&sid)
                    .map(|&(hq, hr, _)| (hq, hr))
                    .unwrap_or((mob.hex_q, mob.hex_r))
            } else {
                (mob.hex_q, mob.hex_r)
            };
            let (spotx, spoty) = encircle_spot(rank);
            let (sq, sr) = (tq + spotx, tr + spoty);
            if (mob.hex_q, mob.hex_r) != (sq, sr)
                && tile_distance(mob.hex_q, mob.hex_r, sq, sr) <= 1
                && kind.step_interval_secs > 0
            {
                let mut row = mob.clone();
                row.hex_id = tile_id_of(sq, sr);
                row.hex_q = sq;
                row.hex_r = sr;
                let (mx, my) = tile_center(sq, sr);
                row.position_x = mx;
                row.position_y = my;
                ctx.db.mob().mob_id().update(row);
            }
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
                // REV4: consumable traps never respawn from rubble.
                if let Some(kind) = kind {
                    if !cfg::is_trap(kind)
                        && capitals.get(&s.capital_id).is_some_and(|c| !c.destroyed)
                    {
                        let level = crate::buildings::half_level(s.level);
                        let mut reborn = s.clone();
                        reborn.level = level;
                        reborn.total_xp = 0;
                        reborn.hp = cfg::structure_max_hp(kind, level);
                        ctx.db.defense_structure().insert(reborn);
                    }
                }
                tracing::info!("STRUCTURE-FELL structure#{sid}");
                // Rubble crumble fx (REV3 §13 "struct_fell").
                emit_event(
                    ctx,
                    s.capital_id,
                    0,
                    now,
                    CE_STRUCT_FELL,
                    0,
                    s.hex_id,
                    (s.slot_x, s.slot_y),
                    &s.kind,
                );
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
                // Capital flash + shake + wave toast (REV3 §13 "capital_hit").
                emit_event(
                    ctx,
                    cid,
                    0,
                    now,
                    CE_CAPITAL_HIT,
                    0,
                    caprow.hex_id,
                    (SLOT_CENTER_X, SLOT_CENTER_Y),
                    "hit",
                );
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

/// March one hex toward the capital every `interval` seconds (greedy axial
/// descent), pinning the position to the destination hex center so the
/// client glides between centers like it does for remote players.
///
/// REV3 §11: standing walls and the capital hex are impassable — a step
/// onto a blocked hex never happens. When NO strictly-closing step stays
/// open the mob is walled in: the caller chews the nearest wall instead of
/// bubbling (so a wall line funnels raiders and the sapper bonus matters).
enum MarchOutcome {
    Stepped,
    /// Fully walled in — `Some(sid)` names the nearest wall to chew.
    WalledIn(Option<u64>),
    /// The step landed on a live trap (REV4): field carries the structure id,
    /// the trap's hex, burst damage (0 = pin-only trap), pin seconds and the
    /// trap kind name for the combat-event detail.
    Trapped {
        structure_id: u64,
        hex_id: u64,
        trap_dmg: u32,
        pin_secs: u32,
        trap_kind: &'static str,
    },
}

fn march(
    ctx: &ReducerContext,
    mob: &Mob,
    interval: u32,
    target_q: i32,
    target_r: i32,
    blocked: &std::collections::HashSet<(i32, i32)>,
    traps_step: &[(u64, i32, i32, u32, u32, &'static str)],
) -> MarchOutcome {
    let mut row = mob.clone();
    row.step_accum_secs += 1;
    if row.step_accum_secs < interval {
        ctx.db.mob().mob_id().update(row);
        return MarchOutcome::Stepped;
    }
    row.step_accum_secs = 0;
    let next = step_toward_hex_avoiding(mob.hex_q, mob.hex_r, target_q, target_r, blocked);
    if let Some(next) = next {
        let (x, y) = tile_center(next.0, next.1);
        row.hex_q = next.0;
        row.hex_r = next.1;
        row.hex_id = tile_id_of(next.0, next.1);
        row.position_x = x;
        row.position_y = y;
        ctx.db.mob().mob_id().update(row);
        // REV4: spring a live spike/web trap sitting on the destination hex.
        if let Some(trap) = traps_step.iter().find(|(_, q, r, _, _, _)| (q, r) == (&next.0, &next.1)) {
            let &(structure_id, _, _, trap_dmg, pin_secs, trap_kind) = trap;
            return MarchOutcome::Trapped {
                structure_id,
                hex_id: tile_id_of(next.0, next.1),
                trap_dmg,
                pin_secs,
                trap_kind,
            };
        }
        return MarchOutcome::Stepped;
    }
    // Every strictly-closing step is a standing wall: name the wall to chew
    // (REV3 §11, deterministic tie-break by distance then structure id).
    // The chew keeps the mob's march cadence — one chew per interval.
    let walls: Vec<(u64, i32, i32)> = ctx
        .db
        .defense_structure()
        .iter()
        .filter(|s| cfg::defense_kind(&s.kind).is_some_and(|k| k.is_wall))
        .map(|s| {
            let (q, r) = hex_coords_of(s.hex_id);
            (s.structure_id, q, r)
        })
        .collect();
    let chewed = nearest_wall_target(
        &walls,
        (mob.hex_q, mob.hex_r),
        (target_q, target_r),
        1,
    );
    ctx.db.mob().mob_id().update(row);
    MarchOutcome::WalledIn(chewed)
}

/// Pure one-hex greedy step toward a target (unit-tested: distance must
/// strictly decrease; this is how raiders close on the capital).
pub fn step_toward_hex(q: i32, r: i32, target_q: i32, target_r: i32) -> (i32, i32) {
    TileCoord::new(q, r)
        .neighbors()
        .into_iter()
        .min_by_key(|&(nq, nr)| tile_distance(nq, nr, target_q, target_r))
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
        let (target, capital) = engagement_target(&nearby, 1, 1);
        assert_eq!(target, Some(7));
        assert!(!capital);
    }

    #[test]
    fn mobs_only_melee_structures_within_one_hex() {
        // A tower 2 hexes away is out of melee reach: keep marching.
        let nearby = [(7u64, 2)];
        let (target, capital) = engagement_target(&nearby, 3, 1);
        assert_eq!(target, None);
        assert!(!capital, "capital 3 hexes away is not reachable");
    }

    #[test]
    fn archers_hit_structures_their_measly_melee_kind_would_miss() {
        // Same stand-off (2 hexes): melee kind marches on, the ranged one
        // (Archer Goblin, reach 3) starts shooting.
        let nearby = [(7u64, 2)];
        assert_eq!(engagement_target(&nearby, 3, 1).0, None);
        let archer = cfg::mob_kind("Archer Goblin").unwrap();
        let (target, _) = engagement_target(&nearby, 3, archer.range_hexes);
        assert_eq!(target, Some(7), "ranged mobs open fire without closing");
        // Reach 3 still misses a target 4 hexes away.
        let far = [(7u64, 4)];
        assert_eq!(engagement_target(&far, 9, archer.range_hexes).0, None);
    }

    #[test]
    fn mobs_hit_the_capital_when_adjacent_and_unobstructed() {
        let (target, capital) = engagement_target(&NO_STRUCTS, 1, 1);
        assert_eq!(target, None);
        assert!(capital);
        // Same hex counts as adjacency (distance 0).
        let (_, capital0) = engagement_target(&NO_STRUCTS, 0, 1);
        assert!(capital0);
        // Nothing in range → both None (keep marching).
        let (t, c) = engagement_target(&NO_STRUCTS, 4, 1);
        assert_eq!(t, None);
        assert!(!c);
        // REV (playtest): ranged mobs do NOT snipe the capital from their
        // stand-off — the capital needs TOUCH (dist ≤ 1), so archers walk
        // the dirt path all the way in.
        let (_, capital_far) = engagement_target(&NO_STRUCTS, 2, 3);
        assert!(!capital_far, "archers never snipe the capital at range");
    }

    #[test]
    fn ties_pick_the_deterministic_structure() {
        // Two walls both one hex away: the lower id wins, always.
        let nearby = [(99u64, 1), (42u64, 1)];
        let (target, _) = engagement_target(&nearby, 1, 1);
        assert_eq!(target, Some(42));
        // A strictly closer structure beats a lower id at greater distance.
        let near2 = [(1u64, 0), (2u64, 1)];
        let (target2, _) = engagement_target(&near2, 1, 1);
        assert_eq!(target2, Some(1));
    }

    #[test]
    fn sapper_bonus_only_applies_to_walls() {
        let spear = cfg::mob_kind("Spear Goblin").unwrap();
        assert_eq!(engagement_dps(spear, true), spear.dps + spear.wall_dps);
        assert_eq!(engagement_dps(spear, false), spear.dps, "towers pay full");
        // Non-sapper kinds never get the wall bonus.
        let slime = cfg::mob_kind("Slime").unwrap();
        assert_eq!(engagement_dps(slime, true), slime.dps);
    }

    #[test]
    fn splash_hits_every_other_adjacent_structure() {
        // The bomb (10 dps) hits its target at full value; neighbors take 5.
        let bomb = cfg::mob_kind("Bomb Goblin").unwrap();
        let nearby = [(42u64, 0), (7u64, 1), (99u64, 1), (100u64, 2)];
        let target = engagement_target(&nearby, 3, bomb.range_hexes).0.unwrap();
        assert_eq!(target, 42);
        assert_eq!(splash_targets(target, &nearby), vec![7, 99], "42 excluded, 100 out of reach");
        // Self-targeting waves only splash nothing when alone.
        assert_eq!(splash_targets(42, &[(42u64, 0)]), Vec::<u64>::new());
    }

    #[test]
    fn march_steps_strictly_toward_the_capital() {
        // From 5 hexes out, each step MUST reduce the distance (raiders never
        // stall — this is the "mobs walk closer" guarantee).
        let mut q = 5;
        let mut r = 0;
        let dist0 = tile_distance(q, r, 0, 0);
        for _ in 0..dist0 {
            let before = tile_distance(q, r, 0, 0);
            let (nq, nr) = step_toward_hex(q, r, 0, 0);
            assert!(
                tile_distance(nq, nr, 0, 0) < before,
                "step ({q},{r}) -> ({nq},{nr}) must close the gap"
            );
            q = nq;
            r = nr;
        }
        assert_eq!((q, r), (0, 0), "march reaches the capital");
        // Diagonal approach from another direction also closes.
        let (mq, mr) = step_toward_hex(-3, 4, 0, 0);
        assert!(tile_distance(mq, mr, 0, 0) < tile_distance(-3, 4, 0, 0));
    }

    #[test]
    fn march_steps_around_walls_and_report_walled_in() {
        use std::collections::HashSet;
        // REV3 §11: a wall dead ahead is stepped AROUND when another
        // strictly-closing tile exists; a full wall ring is WalledIn(None).
        let mut blocked = HashSet::new();
        blocked.insert((4, 0));
        // March (5,0) → (0,0) with the direct hex walled: the path closes via
        // (4,±1) or (4,1)... any strictly-closing open tile wins.
        let mut q = 5;
        let mut r = 0;
        for _ in 0..5 {
            let before = tile_distance(q, r, 0, 0);
            let next = step_toward_hex_avoiding(q, r, 0, 0, &blocked);
            let (nq, nr) = next.expect("an open strictly-closing tile exists");
            assert!(tile_distance(nq, nr, 0, 0) < before);
            assert!(
                !blocked.contains(&(nq, nr)),
                "march never steps onto a wall"
            );
            q = nq;
            r = nr;
        }
        assert_eq!((q, r), (0, 0));
        // Full funnel ring: every closing tile blocked ⇒ None (chew rule).
        let mut wallring = HashSet::new();
        for dq in -1..=1 {
            for dr in -1..=1 {
                if (dq, dr) != (0, 0) {
                    wallring.insert((dq, dr));
                }
            }
        }
        assert_eq!(step_toward_hex_avoiding(0, 0, 0, 5, &wallring), None);
        // Nearest-wall pick: within chew range 1 only; ties by structure id.
        let walls = [(42u64, 1, 0), (7u64, 1, 0), (99u64, 3, 0)];
        assert_eq!(nearest_wall_target(&walls, (0, 0), (0, 5), 1), Some(7));
        assert_eq!(
            nearest_wall_target(&walls, (0, 0), (0, 5), 2),
            Some(7),
            "3 out is beyond chew range even when nearest overall"
        );
        // No wall in reach at all → None.
        assert_eq!(nearest_wall_target(&walls, (0, 0), (0, 5), 0), None);
    }

    #[test]
    fn encircle_ring_spots_are_deterministic_by_rank() {
        // REV3 §12: rank r of the attackers always takes the r-th ring tile,
        // wrapping after 8; the ring itself is the wind-rose with corners
        // lead — a swarm surrounds its target instead of pixel-stackking.
        assert_eq!(encircle_spot(0), (-1, -1));
        assert_eq!(encircle_spot(1), (1, -1));
        assert_eq!(encircle_spot(7), (0, 1));
        assert_eq!(encircle_spot(8), (-1, -1), "wraps");
        assert_eq!(encircle_spot(16), (-1, -1), "wraps plurally");
        for rank in 0..16u32 {
            let (dx, dy) = encircle_spot(rank);
            assert_eq!(dx.abs().max(dy.abs()), 1, "spot {rank} is ring-adjacent");
        }
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
            spawn_q: i32::MIN,
            spawn_r: i32::MIN,
            path_hexes: Vec::new(),
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

    // -- Spec 035: the fixed dirt path -----------------------------------
    // REV: the march trace is the STORED curved path; tests drive it with
    // hex_line as a stand-in node list (the marching rule is identical).

    #[test]
    fn path_target_walks_the_line_node_by_node() {
        let (sq, sr, cq, cr) = (0, 0, 6, 2);
        let line = idlecore_core::grid::hex_line(sq, sr, cq, cr);
        // Standing on the spawn: claimed = line[1] (spawn starts at id 1).
        assert_eq!(raid_lane_target(sq, sr, &line, 1), (1, line[1]));
        // Mid-trace: the next node, never the capital directly.
        let mid = line[3];
        assert_eq!(raid_lane_target(mid.0, mid.1, &line, 4), (4, line[4]));
        // One node before the gate: the capital hex itself.
        let near = line[line.len() - 2];
        assert_eq!(raid_lane_target(near.0, near.1, &line, (line.len() - 1) as u32), (line.len() - 1, (cq, cr)));
        // Standing ON the capital: hold (claimed stays the last node).
        assert_eq!(raid_lane_target(cq, cr, &line, (line.len() - 1) as u32), (line.len() - 1, (cq, cr)));
    }

    #[test]
    fn lane_reentry_never_skips_past_the_claim() {
        let line = idlecore_core::grid::hex_line(0, 0, 6, 0);
        let (cq, cr) = (6, 0);
        // A knocked raider near a LATER node (idx 5) but with claimed = 2:
        // the nearest node AT OR BEFORE 2 wins — no skip-ahead over the bend.
        let (idx, target) = raid_lane_target(5, -1, &line, 2);
        assert!(idx <= 2, "off-trace re-entry may not claim beyond {idx}");
        assert!(target != (cq, cr) || idx <= 2);
        // Off-trace with the claim reachable: the NEAREST node at or before
        // the claim (ties → spawn side), computed like the impl.
        let (idx, target) = raid_lane_target(2, -1, &line, 3);
        let (d, i) = line[..=3]
            .iter()
            .enumerate()
            .map(|(i, &(nq, nr))| (tile_distance(2, -1, nq, nr), i))
            .min()
            .unwrap();
        assert_eq!(idx, i);
        assert_eq!(target, line[i]);
        assert_eq!(d, tile_distance(2, -1, target.0, target.1));
    }

    #[test]
    fn off_trace_claim_walks_back_instead_of_forward() {
        let line = idlecore_core::grid::hex_line(0, 0, 6, 0);
        // Chased far off: claimed 3 untouched → the walk back targets a node
        // at or BEFORE line[3] (Chebyshev ties resolve to the spawn side).
        let (idx, target) = raid_lane_target(9, 9, &line, 3);
        assert!(idx <= 3, "target index {idx} never beyond the claim");
        assert_eq!(target, line[idx]);
        // Nearer-than-the-claim, gone BEYOND it: (4,-1) is uniquely closest
        // to the claimed node — still the claim, never node 5/6.
        let (idx, target) = raid_lane_target(4, -1, &line, 3);
        assert_eq!(idx, 3);
        assert_eq!(target, line[3]);
    }

    #[test]
    fn legacy_capital_without_spawn_keeps_free_march() {
        // Empty trace: the caller keeps the old free march at the capital
        // hex; the pure lane fn defensively echoes the mob's own hex.
        assert_eq!(raid_lane_target(2, 3, &[], 0), (0, (2, 3)));
    }

    #[test]
    fn landing_past_the_claim_re_treads_the_bend() {
        // A shove or chase hop near a fold can land a raider exactly on a
        // LATER node. Progress must not jump: walk back one node, claim
        // unchanged — the skipped bend is re-tread instead of cut.
        let line = idlecore_core::grid::hex_line(0, 0, 6, 0);
        // Standing on node 2 while the claim is still 1: back to node 1.
        assert_eq!(raid_lane_target(line[2].0, line[2].1, &line, 1), (1, line[1]));
        // Standing on the node two past the claim: also one node back...
        let (idx, target) = raid_lane_target(line[3].0, line[3].1, &line, 1);
        assert_eq!(idx, 1, "claim never jumps past a shove");
        assert_eq!(target, line[2]);
        // Standing exactly ON the claim: that arrival advances normally.
        assert_eq!(raid_lane_target(line[1].0, line[1].1, &line, 1), (2, line[2]));
    }

    /// March-sim WITH a mid-lane knockback: a raider shoved forward across
    /// a bend must walk the lane back and regain the claim, never reach
    /// the capital having skipped nodes without standing on them first.
    #[test]
    fn knockback_never_lets_a_raider_skip_a_bend() {
        for seed in 1..25u64 {
            let mut s = seed * 9_317;
            let mut rand = move || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            let path = crate::capital::generate_raid_path(8, 4, 0, 0, |_, _| true, &mut rand);
            let cap = *path.last().unwrap();
            let blocked: std::collections::HashSet<(i32, i32)> =
                [(cap.0, cap.1)].into_iter().collect();
            let mut mob = path[0];
            let mut claim: u32 = 1;
            let mut steps = 0;
            while steps < 300 {
                if steps == 6 {
                    // One king-step shove sideways at a bend: the knock may
                    // land off-lane or on a later node — both must re-chase.
                    let side = [(0, -1), (-1, 0), (1, 0), (0, 1), (-1, -1), (1, 1)]
                        [((rand() >> 4) % 6) as usize];
                    mob = (mob.0 + side.0, mob.1 + side.1);
                }
                let (mq, mr) = mob;
                let old_claim = claim;
                let (idx, (tq, tr)) = raid_lane_target(mq, mr, &path, claim);
                claim = idx as u32;
                // The claim only advances when the mob stands ON that node —
                // a shove landing on a later node must clamp it backward.
                assert!(
                    idx as u32 <= old_claim + 1,
                    "seed {seed}: claim jumped from a shove (idx {idx} > old {old_claim})"
                );
                if (tq, tr) == (mq, mr) {
                    break; // arrived (capital hex) or wedged
                }
                let Some(next) = step_toward_hex_avoiding(mq, mr, tq, tr, &blocked) else {
                    break; // walked into the capital's blocked ring
                };
                mob = next;
                steps += 1;
            }
            let d = tile_distance(mob.0, mob.1, cap.0, cap.1);
            assert!(
                d <= 1,
                "seed {seed}: knocked raider ended {d} hexes from its capital (path {path:?})"
            );
        }
    }

    #[test]
    fn path_target_at_the_capital_stays_put() {
        // A raider standing on the capital hex never marches away.
        let line = idlecore_core::grid::hex_line(0, 0, 4, 2);
        assert_eq!(raid_lane_target(4, 2, &line, (line.len() - 1) as u32), (line.len() - 1, (4, 2)));
    }

    /// March-sim over a STAMPED curved trace, mirroring the server tick:
    /// nearest-node target + strictly-closing step, capital hex blocked.
    #[test]
    fn stamped_trace_marches_arrive_at_the_capital_ring() {
        for seed in 1..25u64 {
            let mut s = seed * 3_771;
            let mut rand = move || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            let path = crate::capital::generate_raid_path(8, 4, 0, 0, |_, _| true, &mut rand);
            let cap = *path.last().unwrap();
            let blocked: std::collections::HashSet<(i32, i32)> =
                [(cap.0, cap.1)].into_iter().collect();
            let mut mob = path[0];
            let mut claim: u32 = 1;
            let mut steps = 0;
            let mut on_trace = true;
            while steps < 200 {
                let (mq, mr) = mob;
                let (idx, (tq, tr)) = raid_lane_target(mq, mr, &path, claim);
                claim = idx as u32;
                if (tq, tr) == (mq, mr) {
                    break;
                }
                let Some(next) = step_toward_hex_avoiding(mq, mr, tq, tr, &blocked) else {
                    break;
                };
                // Every landed hex must be a trace node (the lane follows it).
                assert!(
                    path.contains(&next),
                    "seed {seed}: mob left the trace at {next:?} (target {tq},{tr}) path {path:?}"
                );
                mob = next;
                steps += 1;
                if mob == cap {
                    break;
                }
            }
            let d = tile_distance(mob.0, mob.1, cap.0, cap.1);
            assert!(
                d <= 1,
                "seed {seed}: mob ended {d} hexes from its capital, never touching it (path {path:?})"
            );
            assert!(steps > 0, "seed {seed}: mob never moved");
            let _ = on_trace;
        }
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
    fn frontier_tiles_skip_water_unowned_unmaterialized_and_duplicates() {
        use std::collections::HashSet;
        let owned: HashSet<u64> = [tile_id_of(0, 0), tile_id_of(1, 0)]
            .into_iter()
            .collect();
        // Frontier candidates around the two owned hexes: (1,-1) / (2,-1) /
        // (2,0) / (0,-1) / (-1,0) / (-1,1) land, (0,1) water, (1,1)
        // unmaterialized (None → skipped).
        let terrain = |id: u64| -> Option<bool> {
            let c = TileCoord::from_id(id);
            if (c.x, c.y) == (0, 1) {
                return Some(true);
            }
            if (c.x, c.y) == (1, 1) {
                return None;
            }
            Some(false)
        };
        let borders = frontier_tiles(&owned, terrain);
        let frontier_expected = vec![
            (-1, -1), (-1, 0), (-1, 1), (0, -1), (1, -1), (2, -1), (2, 0), (2, 1),
        ];
        assert_eq!(
            borders,
            frontier_expected,
            "sorted, water and unmaterialized skipped, shared frontier deduped"
        );
        // Every border is unowned and truly adjacent to an owned hex.
        for &(q, r) in &borders {
            assert!(!owned.contains(&tile_id_of(q, r)), "border ({q},{r}) must be unowned");
            let is_adjacent = TileCoord::new(q, r)
                .neighbors()
                .iter()
                .any(|&(nq, nr)| owned.contains(&tile_id_of(nq, nr)));
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
