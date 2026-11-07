//! Capital City and land claim (Spec 023 §3.1–3.3).
//!
//! Placing the Capital is founding the base: it consumes the Capital Core
//! item, owns its hex outright, and opens the claim radius. Land claims
//! extend that territory hex-by-hex (adjacency + radius + resource cost).
//! All territory/defense checks are server-authoritative; the raid lifecycle
//! (once D3/D4 land) targets the capital row via `destroy_capital`.

use spacetimedb::{ReducerContext, Table};
use idlecore_core::defense_config as cfg;
use idlecore_core::defense_config::{ITEM_CAPITAL_CORE, ITEM_STONE, ITEM_WOOD};

use crate::economy::find_player;
use crate::objects::{add_item, count_item, remove_item};
use crate::types::{
    capital, defense_structure, tile_distance, tile_id_of, now_secs, player, raid_wave, territory,
    hex_tile, Capital, DefenseStructure, Territory, ACTION_COOLDOWN_SECS,
};
use crate::world::ensure_hex;

/// Object kind of the capital on the world sheet (client marker).
pub const OBJ_CAPITAL: &str = "Capital";

// ---------------------------------------------------------------------------
// Raid path (Spec 035)
// ---------------------------------------------------------------------------

/// Classic-TD path length floor: the spawn is pushed out to this many hexes
/// so the dirt trace reads as a path instead of a pad glued to the base.
/// Playtest REV #3: the spawn still read too close — the walk-out target is
/// now RANDOM in `PATH_TARGET_MIN..=PATH_TARGET_MAX` (6–9), so each
/// placement makes its own length AND its own bend-count.
pub const MIN_PATH_HEXES: i32 = 6;
pub const PATH_TARGET_MIN: i32 = 6;
pub const PATH_TARGET_MAX: i32 = 9;
/// Random walk-out target range: `PATH_TARGET_MIN + rand % range`.
pub const PATH_TARGET_SPAN: i32 = PATH_TARGET_MAX - PATH_TARGET_MIN + 1;
/// Bend (sidestep) budget: random in `2..=5` at placement. Sidesteps are
/// same-distance steps that bend the trace without flattening it.
pub const CURVE_MIN: i32 = 2;
pub const CURVE_SPAN: i32 = 4;

/// Pure random curved raid-path generator (Spec 035 REV): a WALK-OUT from
/// the capital hex with limited sidesteps, returned REVERSED so the path
/// runs spawn-first, capital-last. Because every step was strictly-farther
/// or sideways from the capital, the reversed trace monotonically closes in
/// — curved, no repetition, and it never back-tracks. Land-only steps
/// (`walkable`, unowned land), water skipped entirely. `rand` gets a
/// uniform source whenever it wants one (reducer: `ctx.rng()`; tests: a
/// seeded LCG). Terminates: strictly-farther steps always exist on unshown
/// terrain (`walkable` defaults true on unmaterialized hexes; the caller
/// does that), and the walk-out stops at `target_len` with the sidestep
/// budget bounded.
pub(crate) fn generate_raid_path(
    target_len: i32,
    sidesteps_left: i32,
    cq: i32,
    cr: i32,
    walkable: impl Fn(i32, i32) -> bool,
    rand: &mut impl FnMut() -> u64,
) -> Vec<(i32, i32)> {
    let dist = |q: i32, r: i32| tile_distance(q, r, cq, cr);
    let mut out: Vec<(i32, i32)> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut budget = sidesteps_left;
    let mut cur = (cq, cr);
    visited.insert(cur);
    out.push(cur);

    #[allow(clippy::while_let_loop)]
    while dist(cur.0, cur.1) < target_len {
        let mut side = Vec::new();
        let mut fwd = Vec::new();
        for nq in cur.0 - 1..=cur.0 + 1 {
            for nr in cur.1 - 1..=cur.1 + 1 {
                let cand = (nq, nr);
                if cand == cur || !visited.insert(cand) || !walkable(nq, nr) {
                    continue;
                }
                let d = dist(nq, nr);
                let cur_d = dist(cur.0, cur.1);
                if d > cur_d {
                    fwd.push(cand);
                } else if d == cur_d {
                    side.push(cand);
                }
            }
        }
        // Bend when the roll says so and budget remains; otherwise push out.
        let step = if !side.is_empty() && budget > 0 && rand() & 1 == 0 {
            budget -= 1;
            side[(rand() % side.len() as u64) as usize]
        } else if !fwd.is_empty() {
            fwd[(rand() % fwd.len() as u64) as usize]
        } else if !side.is_empty() {
            // Forced detour also burns budget so the walk stays bounded.
            budget -= 1;
            side[(rand() % side.len() as u64) as usize]
        } else {
            break;
        };
        cur = step;
        out.push(cur);
    }

    // Spawn→capital order: the walk-out was capital→spawn, so reverse.
    out.reverse();
    out
}

/// Reducer wiring: roll the walk-out length (`PATH_TARGET_MIN..=PATH_TARGET_MAX`)
/// and the bend budget from `ctx.rng()`, then walk. Returns the spawn hex
/// plus the hex-id list to stamp on the Capital row. Steps are land-only
/// (unmaterialized hexes count as land, like `walk_out`) and skip owned hexes
/// so the raiders do not spawn inside a claim.
pub(crate) fn stamp_raid_path(
    ctx: &ReducerContext,
    hex_id: u64,
) -> ((i32, i32), Vec<u64>) {
    use spacetimedb::rand::Rng;
    let target = PATH_TARGET_MIN + (ctx.rng().gen_range(0..PATH_TARGET_SPAN as u64) as i32);
    let sidesteps = CURVE_MIN + (ctx.rng().gen_range(0..CURVE_SPAN as u64) as i32);
    let (cq, cr) = crate::types::hex_coords_of(hex_id);
    let mut rand = move || ctx.rng().gen_range(0..1_000_000u64);
    let walkable = |q: i32, r: i32| {
        let id = tile_id_of(q, r);
        ctx.db
            .hex_tile()
            .hex_id()
            .find(id)
            .map(|t| t.terrain != "Water")
            .unwrap_or(true)
            && !is_owned(ctx, id)
    };
    let path = generate_raid_path(target, sidesteps, cq, cr, walkable, &mut rand);
    let (sq, sr) = path[0];
    ((sq, sr), path.iter().map(|&(q, r)| tile_id_of(q, r)).collect())
}

// ---------------------------------------------------------------------------
// Pure validators (unit-testable without SpacetimeDB)
// ---------------------------------------------------------------------------

/// Error reason for a rejected claim (None = allowed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    NotAdjacent,
    OutOfRadius,
    Water,
}

impl std::fmt::Display for ClaimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimError::NotAdjacent => write!(f, "Claims must touch your territory"),
            ClaimError::OutOfRadius => write!(f, "That hex is beyond your claim radius — level up the capital"),
            ClaimError::Water => write!(f, "Water cannot be claimed"),
        }
    }
}

/// Pure claim gate: the candidate hex must be walkable (land), adjacent
/// (hex distance 1) to an owned hex, and within the capital's claim radius.
pub fn claim_error(
    candidate: (i32, i32),
    candidate_is_water: bool,
    owned: &[(i32, i32)],
    capital: (i32, i32),
    level: u32,
) -> Option<ClaimError> {
    if candidate_is_water {
        return Some(ClaimError::Water);
    }
    let adjacent = owned
        .iter()
        .any(|&o| tile_distance(o.0, o.1, candidate.0, candidate.1) == 1);
    if !adjacent {
        return Some(ClaimError::NotAdjacent);
    }
    let radius = cfg::claim_radius(level);
    // REV9: the claimable region is a SQUARE in world space (axis-aligned
    // |dx|/|dy| gate) instead of a hex disc. Half-side = radius × the
    // adjacent-hex world step (HEX_SIZE·√3), so the straight-line reach on
    // each axis matches the old disc; the corners reach farther (area
    // ×4/π ≈ 1.27).
    let size = idlecore_core::world_gen::WorldGenConfig::TILE_SIZE;
    // Square gate: half-side = radius tile edges (17.32 wu each), so the
    // straight-line reach on each axis matches the radius count in steps.
    let half_side = radius as f32 * size;
    let (cwx, cwy) = idlecore_core::grid::WorldGrid::tile_center(capital.0, capital.1, size);
    let (pwx, pwy) =
        idlecore_core::grid::WorldGrid::tile_center(candidate.0, candidate.1, size);
    if (pwx - cwx).abs() > half_side || (pwy - cwy).abs() > half_side {
        return Some(ClaimError::OutOfRadius);
    }
    None
}

/// Pure spacing check for a new capital placement: every live capital must
/// sit at least [`cfg::MIN_CAPITAL_SPACING`] hexes away.
pub fn capital_spacing_ok(candidate: (i32, i32), live_capitals: &[(i32, i32)]) -> bool {
    live_capitals
        .iter()
        .all(|&c| tile_distance(c.0, c.1, candidate.0, candidate.1) >= cfg::MIN_CAPITAL_SPACING)
}

/// Tiles of the farmer-starter square a fresh capital auto-owns for free:
/// every tile within [`cfg::CAPITAL_STARTER_RADIUS`] Chebyshev steps of the
/// capital, the capital itself excluded (it is owned separately). Pure —
/// the placement reducer still re-checks water/ownership per tile.
pub fn starter_square_tiles(capital: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let (cq, cr) = capital;
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (q, r) = (cq + dx, cr + dy);
            if (q, r) != capital {
                tiles.push((q, r));
            }
        }
    }
    tiles
}

// ---------------------------------------------------------------------------
// Row helpers
// ---------------------------------------------------------------------------

/// Capital owned by the address, if any (destroyed rows included).
pub fn find_capital(ctx: &ReducerContext, owner: &str) -> Option<Capital> {
    ctx.db.capital().capital_by_owner().filter(owner).next()
}

/// Capital sitting exactly on a hex (the one the raids march on).
pub fn find_capital_at(ctx: &ReducerContext, hex_id: u64) -> Option<Capital> {
    ctx.db
        .capital()
        .iter()
        .find(|c| c.hex_id == hex_id && !c.destroyed)
}

/// Whether a hex is owned (by anyone).
pub fn is_owned(ctx: &ReducerContext, hex_id: u64) -> bool {
    ctx.db.territory().hex_id().find(hex_id).is_some()
}

/// Owner address of a hex's territory (None when unclaimed).
pub fn owner_of(ctx: &ReducerContext, hex_id: u64) -> Option<String> {
    ctx.db
        .territory()
        .hex_id()
        .find(hex_id)
        .map(|t| t.owner)
}

fn touch(ctx: &ReducerContext, address: &str, now: u64) {
    if let Some(mut p) = find_player(ctx, address) {
        p.last_action_at = now;
        ctx.db.player().address().update(p);
    }
}

fn cooldown_left(p: &crate::types::Player, now: u64) -> Result<(), String> {
    if now.saturating_sub(p.last_action_at) < ACTION_COOLDOWN_SECS {
        return Err(format!("Action cooldown ({ACTION_COOLDOWN_SECS}s)"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reducer bodies
// ---------------------------------------------------------------------------

/// Place (or re-place after destruction) the Capital City on a legal hex:
/// land, unclaimed, well away from every other capital, consuming the
/// Capital Core. The hex becomes the first territory.
pub fn place_capital(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
) -> Result<(), String> {
    let now = now_secs(ctx);
    let p = find_player(ctx, address).ok_or_else(|| "Player not found".to_string())?;

    let (hq, hr) = crate::types::hex_coords_of(hex_id);
    ensure_hex(ctx, hq, hr);
    let tile = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;
    if tile.terrain == "Water" {
        return Err("A capital cannot stand on water".to_string());
    }
    // One live capital per owner, ever. Destroyed rows are re-placed in place.
    let slot = find_capital(ctx, &p.address);
    if let Some(existing) = &slot {
        if !existing.destroyed {
            return Err("You already have a capital".to_string());
        }
    }

    // Spacing against every other live capital.
    let others: Vec<(i32, i32)> = ctx
        .db
        .capital()
        .iter()
        .filter(|c| !c.destroyed && c.owner != p.address)
        .map(|c| crate::types::hex_coords_of(c.hex_id))
        .collect();
    if !capital_spacing_ok((hq, hr), &others) {
        return Err(format!(
            "Capitals need at least {} hexes between each other",
            cfg::MIN_CAPITAL_SPACING
        ));
    }
    // The founding hex must be free land.
    if is_owned(ctx, hex_id) {
        return Err("That hex is already claimed".to_string());
    }

    // Consume the core (fresh grant or the refunded one after destruction).
    if count_item(ctx, &p.address, ITEM_CAPITAL_CORE) == 0 {
        return Err("You need the Capital Core to found a city".to_string());
    }
    remove_item(ctx, &p.address, ITEM_CAPITAL_CORE, 1);

    let grace = if slot.is_some() {
        cfg::GRACE_REBUILD_SECS
    } else {
        cfg::GRACE_FIRST_RAID_SECS
    };
    let mut capital_id = None;
    match slot {
        Some(mut row) => {
            row.hex_id = hex_id;
            row.hp = cfg::capital_max_hp(1);
            row.level = 1;
            row.total_xp = 0;
            row.grace_until = now + grace;
            row.last_raid_at = 0;
            row.placed_at = now;
            row.destroyed = false;
            // Spec 035 S1: the path re-rolls (length, bends, direction) at
            // every re-placement.
            let ((sq, sr), path) = stamp_raid_path(ctx, hex_id);
            row.spawn_q = sq;
            row.spawn_r = sr;
            row.path_hexes = path;
            capital_id = Some(row.capital_id);
            ctx.db.capital().capital_id().update(row);
        }
        None => {
            let mut tx = ctx.db.capital().insert(Capital {
                capital_id: 0,
                owner: p.address.clone(),
                hex_id,
                hp: cfg::capital_max_hp(1),
                level: 1,
                total_xp: 0,
                grace_until: now + grace,
                placed_at: now,
                last_raid_at: 0,
                destroyed: false,
                spawn_q: 0,
                spawn_r: 0,
                path_hexes: Vec::new(),
            });
            // Fresh placement: the first open wave needs the path sent once.
            let ((sq, sr), path) = stamp_raid_path(ctx, hex_id);
            tx.spawn_q = sq;
            tx.spawn_r = sr;
            tx.path_hexes = path;
            ctx.db.capital().capital_id().update(tx.clone());
            capital_id = Some(tx.capital_id);
        }
    }
    // The capital hex is owned outright (no claim cost), and the
    // starter square around it is granted free land: every land tile
    // within cfg::CAPITAL_STARTER_RADIUS that nobody owns yet. Water
    // tiles and tiles owned by anyone are left out.
    for (q, r) in crate::capital::starter_square_tiles((hq, hr), cfg::CAPITAL_STARTER_RADIUS) {
        let tile_id = tile_id_of(q, r);
        if is_owned(ctx, tile_id) {
            continue;
        }
        ensure_hex(ctx, q, r);
        if ctx
            .db
            .hex_tile()
            .hex_id()
            .find(tile_id)
            .map(|t| t.terrain == "Water")
            .unwrap_or(true)
        {
            continue;
        }
        ctx.db.territory().insert(Territory {
            hex_id: tile_id,
            owner: p.address.clone(),
            capital_id: capital_id.unwrap_or(0),
            claimed_at: now,
        });
    }
    touch(ctx, &p.address, now);
    tracing::info!("PLACE-CAPITAL {address} @hex {hex_id}");
    Ok(())
}

/// Claim an unclaimed hex for territory: adjacency to owned land, radius
/// from the capital, 5 Wood + 3 Stone. Reuses the global action cooldown.
pub fn claim_land(ctx: &ReducerContext, address: &str, hex_id: u64) -> Result<(), String> {
    let now = now_secs(ctx);
    let p = find_player(ctx, address).ok_or_else(|| "Player not found".to_string())?;
    cooldown_left(&p, now)?;

    let cap = find_capital(ctx, &p.address)
        .filter(|c| !c.destroyed)
        .ok_or_else(|| "Place your capital first".to_string())?;

    let (hq, hr) = crate::types::hex_coords_of(hex_id);
    ensure_hex(ctx, hq, hr);
    let tile = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;

    let owned: Vec<(i32, i32)> = ctx
        .db
        .territory()
        .territory_by_owner()
        .filter(&p.address)
        .map(|t| crate::types::hex_coords_of(t.hex_id))
        .collect();
    if owned.iter().any(|&o| tile_id_of(o.0, o.1) == hex_id) {
        return Err("You already own that hex".to_string());
    }
    let (cq, cr) = crate::types::hex_coords_of(cap.hex_id);
    if let Some(err) = claim_error((hq, hr), tile.terrain == "Water", &owned, (cq, cr), cap.level) {
        return Err(err.to_string());
    }

    // Pay the claim cost — verify the whole bundle before spending anything.
    for (item, qty, label) in [
        (ITEM_WOOD, cfg::CLAIM_WOOD_COST, "wood"),
        (ITEM_STONE, cfg::CLAIM_STONE_COST, "stone"),
    ] {
        if count_item(ctx, &p.address, item) < qty {
            return Err(format!("Claiming needs {qty} {label} per hex"));
        }
    }
    remove_item(ctx, &p.address, ITEM_WOOD, cfg::CLAIM_WOOD_COST);
    remove_item(ctx, &p.address, ITEM_STONE, cfg::CLAIM_STONE_COST);

    ctx.db.territory().insert(Territory {
        hex_id,
        owner: p.address.clone(),
        capital_id: cap.capital_id,
        claimed_at: now,
    });
    touch(ctx, &p.address, now);
    tracing::info!("CLAIM-LAND {address} @hex {hex_id}");
    Ok(())
}

/// Destruction cascade (Spec 023 §3.1): the capital is lost, every claimed
/// hex is released, every structure is removed, and the Capital Core is
/// refunded so the player can re-found later. Runs when raid damage drops
/// the capital HP to 0.
pub fn destroy_capital(ctx: &ReducerContext, mut cap: Capital, now: u64) {
    cap.destroyed = true;
    cap.hp = 0;
    cap.grace_until = now + cfg::GRACE_REBUILD_SECS;
    ctx.db.capital().capital_id().update(cap.clone());

    let owner = cap.owner.clone();
    // Territory is released wholesale (no decay — user decision).
    let lands: Vec<u64> = ctx
        .db
        .territory()
        .territory_by_owner()
        .filter(&cap.owner)
        .map(|t| t.hex_id)
        .collect();
    for hex_id in lands {
        ctx.db.territory().hex_id().delete(hex_id);
    }
    // Structures are erased (they were part of lost territory); chest
    // contents go back to the owner first so the loss is bounded.
    let structures: Vec<DefenseStructure> = ctx
        .db
        .defense_structure()
        .structure_by_capital()
        .filter(cap.capital_id)
        .collect();
    for s in &structures {
        if s.kind == cfg::ITEM_STORAGE_CHEST {
            crate::buildings::refund_chest(ctx, s.structure_id, &cap.owner);
        }
    }
    for s in structures {
        ctx.db.defense_structure().structure_id().delete(s.structure_id);
    }
    // Any open wave against this capital closes immediately.
    for mut wave in ctx
        .db
        .raid_wave()
        .wave_by_capital()
        .filter(cap.capital_id)
        .filter(|w| w.ended_at.is_none())
    {
        wave.ended_at = Some(now);
        wave.outcome = Some("capital_destroyed".to_string());
        ctx.db.raid_wave().wave_id().update(wave);
    }
    // Refund the core so the owner can re-found after the quarantine.
    add_item(ctx, &owner, ITEM_CAPITAL_CORE, 1);
    tracing::warn!("CAPITAL-DESTROYED {owner} @hex {}", cap.hex_id);
}

/// Grant capital XP and apply level-ups (called by the mob tick on kills).
/// Levels raise max HP and heal the delta proportionally — a mid-raid
/// level-up must never heal the capital full.
pub fn capital_add_xp(ctx: &ReducerContext, cap: &Capital, xp: u64, _now: u64) {
    let mut row = cap.clone();
    row.total_xp = row.total_xp.saturating_add(xp);
    let new_level = cfg::capital_level_from_xp(row.total_xp);
    if new_level > row.level {
        let old_max = cfg::capital_max_hp(row.level);
        let new_max = cfg::capital_max_hp(new_level);
        row.hp = (row.hp + (new_max - old_max)).min(new_max);
        row.level = new_level;
        tracing::info!("CAPITAL-LEVELUP {} -> L{new_level}", row.owner);
    }
    ctx.db.capital().capital_id().update(row);
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlecore_core::grid::TileCoord;

    fn id(q: i32, r: i32) -> (i32, i32) {
        (TileCoord::new(q, r).x, TileCoord::new(q, r).y)
    }

    #[test]
    fn claim_requires_adjacency_to_owned_hex() {
        let owned = vec![id(0, 0), id(1, 0)];
        // Diagonal near (2,0): (3,0) is adjacent to (2,0)? not owned.
        assert_eq!(
            claim_error(id(3, 0), false, &owned, id(0, 0), 1),
            Some(ClaimError::NotAdjacent)
        );
        // (1, 1) touches (1,0). OK.
        assert_eq!(claim_error(id(1, 1), false, &owned, id(0, 0), 1), None);
        // (2,0) is adjacent to (1,0). Radius from capital (0,0) is 2 — ok.
        assert_eq!(claim_error(id(2, 0), false, &owned, id(0, 0), 1), None);
        // (0,3): adjacent to owned? no.
        assert_eq!(
            claim_error(id(0, 3), false, &owned, id(0, 0), 1),
            Some(ClaimError::NotAdjacent)
        );
    }

    #[test]
    fn claim_radius_gates_far_hexes() {
        // REV10 square: half-side = radius tile edges (17.32 wu each).
        // L1 radius 83 → half-side 1437.6 wu; L5 cap 91 → 1576.2 wu.
        let owned = vec![id(0, 0), id(0, 100)];
        // (0,101) is 1749 wu out on the z axis: beyond even the L5 cap
        // square (1576.2 wu).
        assert_eq!(
            claim_error(id(0, 101), false, &owned, id(0, 0), 1),
            Some(ClaimError::OutOfRadius)
        );
        assert_eq!(
            claim_error(id(0, 101), false, &owned, id(0, 0), 5),
            Some(ClaimError::OutOfRadius),
            "z 1749 wu > cap half-side 1576.2 wu"
        );
        // (0,88) is 1524 wu out: rejected at L1 (1437.6), inside at L5.
        // Owned (0,89) keeps the adjacency gate open for it.
        assert_eq!(
            claim_error(id(0, 88), false, &vec![id(0, 0), id(0, 89)], id(0, 0), 1),
            Some(ClaimError::OutOfRadius)
        );
        assert_eq!(claim_error(id(0, 88), false, &vec![id(0, 0), id(0, 89)], id(0, 0), 5), None);
        // (0,82) is 1420.3 wu out: inside the L1 square.
        assert_eq!(
            claim_error(id(0, 82), false, &vec![id(0, 0), id(0, 83)], id(0, 0), 1),
            None
        );
        // Same gate on the x axis: (84,0) is 1454.9 wu east at L1, inside
        // the L5 square.
        let row = vec![id(0, 0), id(83, 0)];
        assert_eq!(
            claim_error(id(84, 0), false, &row, id(0, 0), 1),
            Some(ClaimError::OutOfRadius)
        );
        assert_eq!(claim_error(id(84, 0), false, &row, id(0, 0), 5), None);
        // Water is never claimable, whatever the radius.
        assert_eq!(
            claim_error(id(0, 1), true, &owned, id(0, 0), 5),
            Some(ClaimError::Water)
        );
    }

    #[test]
    fn claims_keep_minimum_distance_between_capitals() {
        // REV10 spacing: 50 tile steps Chebyshev between capitals
        // (scale-independent tuning; claim squares at cap can still touch).
        assert!(capital_spacing_ok(id(50, 0), &[id(0, 0)]));
        assert!(capital_spacing_ok(id(0, 50), &[id(0, 0)]), "axis-aligned");
        assert!(!capital_spacing_ok(id(49, 0), &[id(0, 0)]));
        // Empty world accepts anything.
        assert!(capital_spacing_ok(id(0, 0), &[]));
        // Several capitals each enforce the gap.
        let live = vec![id(0, 0), id(100, 0)];
        assert!(!capital_spacing_ok(id(25, 0), &live));
        assert!(
            !capital_spacing_ok(id(98, 0), &live),
            "2 tiles from (100,0)"
        );
        // (50,0): 50 from (0,0) and 50 from (100,0) — legal.
        assert!(capital_spacing_ok(id(50, 0), &live));
        // (52,0): 52 from origin, 48 from (100,0) — denied.
        assert!(!capital_spacing_ok(id(52, 0), &live));
        // (150,0): 150 from origin, 50 from (100,0) — legal.
        assert!(capital_spacing_ok(id(150, 0), &live));
    }

    #[test]
    fn claim_cost_is_wood_and_stone() {
        assert_eq!(cfg::CLAIM_WOOD_COST, 5);
        assert_eq!(cfg::CLAIM_STONE_COST, 3);
    }

    #[test]
    fn capital_hex_grants_absolute_ownership_radius() {
        // The capital hex itself is owned without paying; claims hang off it.
        let owned = vec![id(0, 0)];
        assert_eq!(claim_error(id(1, 0), false, &owned, id(0, 0), 1), None);
    }

    #[test]
    fn starter_square_covers_radius_excluding_capital() {
        use idlecore_core::defense_config::CAPITAL_STARTER_RADIUS;
        let tiles = starter_square_tiles((0, 0), CAPITAL_STARTER_RADIUS);
        // (2R+1)² minus the capital itself.
        let w = 2 * CAPITAL_STARTER_RADIUS + 1;
        assert_eq!(tiles.len() as i32, w * w - 1);
        // The capital hex is never in the list.
        assert!(!tiles.contains(&(0, 0)));
        // Every corner of the square is present.
        for corner in [
            (-CAPITAL_STARTER_RADIUS, -CAPITAL_STARTER_RADIUS),
            (CAPITAL_STARTER_RADIUS, -CAPITAL_STARTER_RADIUS),
            (-CAPITAL_STARTER_RADIUS, CAPITAL_STARTER_RADIUS),
            (CAPITAL_STARTER_RADIUS, CAPITAL_STARTER_RADIUS),
        ] {
            assert!(tiles.contains(&corner));
        }
        // Nothing extends past the square.
        assert!(tiles.iter().all(|(q, r)| {
            q.abs() <= CAPITAL_STARTER_RADIUS && r.abs() <= CAPITAL_STARTER_RADIUS
        }));
        // Anchored away from origin too.
        let shifted = starter_square_tiles((7, -3), 2);
        assert!(!shifted.contains(&(7, -3)));
        assert!(shifted.contains(&(9, -5)));
        assert!(!shifted.contains(&(10, -5)));
    }

    // -- Spec 035 REV: the random curved raid path ------------------------

    /// Seeded xorshift — the test twin of `ctx.rng()`.
    fn test_rng(seed: u64) -> impl FnMut() -> u64 {
        let mut s = seed | 1;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        }
    }

    #[test]
    fn path_starts_far_ends_at_the_capital() {
        for seed in 1..40u64 {
            let mut r = test_rng(seed * 7_919);
            let target = crate::capital::PATH_TARGET_MIN + (seed % 4) as i32;
            let path = generate_raid_path(target, 3, 0, 0, |_, _| true, &mut r);
            // Spawn hex FIRST, capital hex LAST — never degenerate.
            assert!(path.len() >= 3, "seed {seed}: short path {path:?}");
            assert_eq!(path[path.len() - 1], (0, 0));
            let spawn_dist = tile_distance(path[0].0, path[0].1, 0, 0);
            assert!(
                spawn_dist >= crate::capital::PATH_TARGET_MIN - 1,
                "seed {seed}: spawn at {spawn_dist}, path {path:?}"
            );
            // Consecutive steps are hex-adjacent, no revisits.
            let mut seen = std::collections::HashSet::new();
            for (idx, &(q, r)) in path.iter().enumerate() {
                if let Some(&(pq, pr)) = idx.checked_sub(1).and_then(|i| path.get(i)) {
                    assert_eq!(
                        tile_distance(q, r, pq, pr),
                        1,
                        "seed {seed}: step {pq},{pr} -> {q},{r} in {path:?}"
                    );
                }
                assert!(seen.insert((q, r)), "seed {seed}: revisit {q},{r}");
            }
        }
    }

    #[test]
    fn different_seeds_bend_differently() {
        // True randomness, not determinism: several seeds must produce
        // several DISTINCT traces.
        let traces: Vec<Vec<(i32, i32)>> = (0..8)
            .map(|seed| {
                let mut r = test_rng((seed as u64 + 41) * 12_997);
                generate_raid_path(8, 4, 0, 0, |_, _| true, &mut r)
            })
            .collect();
        let all_same = traces[1..].iter().all(|t| t == &traces[0]);
        assert!(!all_same, "all seeds produced the same trace: {:?}", traces[0]);
    }

    #[test]
    fn sidesteps_bend_the_trace_off_the_straight_line() {
        // With bend budget, at least one node lies OFF hex_line(spawn, cap).
        let mut r = test_rng(5_555_111);
        let path = generate_raid_path(8, 4, 0, 0, |_, _| true, &mut r);
        let straight: Vec<(i32, i32)> =
            idlecore_core::grid::hex_line(path[0].0, path[0].1, 0, 0);
        assert!(
            path.iter().any(|n| !straight.contains(n)),
            "no bends: {path:?} vs line {straight:?}"
        );
    }

    #[test]
    fn water_blocks_the_walk() {
        // One impassable hex near the capital: the trace routes around it.
        let mut r = test_rng(2_222);
        let path = generate_raid_path(
            6,
            3,
            0,
            0,
            |q, r| !(q == 1 && r == -1),
            &mut r,
        );
        assert!(!path.contains(&(1, -1)));
        assert_eq!(path[path.len() - 1], (0, 0));
        assert!(path.len() >= 3);
    }
}
