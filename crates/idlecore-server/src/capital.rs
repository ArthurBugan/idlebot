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
            capital_id = Some(row.capital_id);
            ctx.db.capital().capital_id().update(row);
        }
        None => {
            let row = ctx.db.capital().insert(Capital {
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
            });
            capital_id = Some(row.capital_id);
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
        // REV8 spacing: 200 tile steps Chebyshev between capitals (two
        // full capped radius-91 squares + a walking gap).
        assert!(capital_spacing_ok(id(200, 0), &[id(0, 0)]));
        assert!(capital_spacing_ok(id(0, 200), &[id(0, 0)]), "axis-aligned");
        assert!(!capital_spacing_ok(id(199, 0), &[id(0, 0)]));
        // Empty world accepts anything.
        assert!(capital_spacing_ok(id(0, 0), &[]));
        // Several capitals each enforce the gap.
        let live = vec![id(0, 0), id(400, 0)];
        assert!(!capital_spacing_ok(id(100, 0), &live));
        assert!(
            !capital_spacing_ok(id(390, 0), &live),
            "10 tiles from (400,0)"
        );
        // (200,0): 200 from (0,0) and 200 from (400,0) — legal.
        assert!(capital_spacing_ok(id(200, 0), &live));
        // (210,0): 210 from origin, 190 from (400,0) — denied.
        assert!(!capital_spacing_ok(id(210, 0), &live));
        // (600,0): 600 from origin, 200 from (400,0) — legal.
        assert!(capital_spacing_ok(id(600, 0), &live));
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
}
