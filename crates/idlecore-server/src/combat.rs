//! Spec 027 — player combat: `player_attack` / `player_dash` /
//! `equip_weapon` reducers, the roaming-Slime spawner + wander AI, and the
//! player-threat side of the 1s tick (regen, death, respawn, aggroed mob
//! dps). Server-authoritative on the existing 1s scheduler; integer math;
//! the client predicts (`dash_until` mirror) and the server validates
//! (`move_player` dash speed allowance + i-frame skips here).

use spacetimedb::{ReducerContext, Table};
use std::collections::HashMap;

use idlecore_core::grid::TileCoord;
use idlecore_core::player_combat as combat;
use idlecore_core::defense_config as cfg;

use crate::objects::add_item;
use crate::types::{
    combat_event,
    capital as cap_table, capital, defense_structure, hex_coords_of, hex_tile, mob, now_secs,
    player, tile_center, tile_distance, tile_id_of, Capital, Mob, Player, CE_PLAYER_HURT,
    CE_PLAYER_HIT, CE_TRAP,
};

// ---------------------------------------------------------------------------
// HP helpers
// ---------------------------------------------------------------------------

/// Effective HP: legacy/seeding rows read `player_hp == 0` as FULL — a real
/// death stamps `death_until` in the same write, so 0-without-a-stamp
/// cannot survive one tick of this module.
pub fn hp_of(p: &Player) -> u32 {
    if p.player_hp == 0 {
        combat::player_max_hp(p.level)
    } else {
        p.player_hp
    }
}

/// Alive = the corpse window has passed (a fresh, never-hit row is alive).
pub fn is_alive(p: &Player, now: u64) -> bool {
    p.death_until <= now
}

/// Walkable land? Unmaterialized hexes block (roamers may not wander into
/// streaming gaps).
fn land_at(ctx: &ReducerContext, hex_id: u64) -> bool {
    ctx.db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .is_some_and(|t| t.terrain != "Water")
}

// ---------------------------------------------------------------------------
// Player kill credit (roamer drop split is §7, raider credit unchanged §8)
// ---------------------------------------------------------------------------

/// Pure pick (§5.1, unit-tested): the nearest arc-allowed candidate —
/// `candidates` carries (mob_id, hex dist, vec x/y from the player).
pub fn pick_arc_target(
    candidates: &[(u64, i32, f32, f32)],
    dir: i32,
    range: i32,
) -> Option<u64> {
    candidates
        .iter()
        .copied()
        .filter(|(_, d, vx, vy)| {
            *d <= range && combat::heading_allows(*vx, *vy, dir)
        })
        .min_by_key(|id_general| (id_general.1, id_general.0))
        .map(|(id, _, _, _)| id)
}

/// Kill application: raid mobs keep the exact tower-kill credit (drop to the
/// capital's defender, `CAPITAL_XP_PER_MOB` to the capital); roamers pay
/// their drops straight to the credited player plus `ROAMER_XP` — no
/// capital XP from roamers (§7).
pub(crate) fn kill_or_wound(
    ctx: &ReducerContext,
    mob_row: &Mob,
    damage: u32,
    capitals: &HashMap<u64, Capital>,
    credit_fallback: Option<&str>,
) {    if mob_row.hp > damage {
        let mut m = mob_row.clone();
        m.hp -= damage;
        ctx.db.mob().mob_id().update(m);
        return;
    }
    // Spec 034 S4: the death itself rides out as its own event so the
    // client can pop a bigger burst — kills must read as kills.
    emit_player_event(
        ctx,
        crate::types::CE_MOB_FELL,
        mob_row.hex_id,
        0,
        mob_row.mob_id,
        &mob_row.kind,
        now_secs(ctx),
    );
    ctx.db.mob().mob_id().delete(mob_row.mob_id);
    let Some(kind) = cfg::mob_kind(&mob_row.kind) else { return };
    let now = now_secs(ctx);

    if mob_row.capital_id == 0 {
        // Roamer: direct drop to the credited player + flat XP (deterministic
        // roll per mob id, same pattern as raider drops — Spec 023 §3.3).
        if let Some(credit) = credit_fallback {
            if kind.name == "Slime" && (u64::from(mob_row.mob_id) % 100) < combat::ROAMER_BONE_PCT {
                add_item(ctx, credit, cfg::ITEM_BONE, 1);
            }
            if (u64::from(mob_row.mob_id) % 100) < u64::from(kind.drop_chance_pct) {
                add_item(ctx, credit, kind.drop_item, u64::from(kind.drop_qty));
            }
            if let Some(mut p) = crate::economy::find_player(ctx, credit) {
                crate::economy::add_xp(ctx, &mut p, combat::ROAMER_XP, "roamer_kill");
            }
        }
        tracing::info!("ROAMER-KILL kind={} drops={}", kind.name, kind.drop_item);
        return;
    }

    let Some(capital) = capitals.get(&mob_row.capital_id) else { return };
    if (u64::from(mob_row.mob_id) % 100) < u64::from(kind.drop_chance_pct) {
        add_item(ctx, &capital.owner, kind.drop_item, u64::from(kind.drop_qty));
    }
    crate::capital::capital_add_xp(ctx, capital, cfg::CAPITAL_XP_PER_MOB, now);
}

// ---------------------------------------------------------------------------
// player_attack
// ---------------------------------------------------------------------------

/// The supplier of a strike for a player attack (Spec 027 §5.1): gate chain →
/// (weapon-dmg × arc pick) → damage/kill → aggro transition → fx event.
/// Reduced-cost: the 5 s action cooldown GATES but is never extended by
/// combat (gather-and-fight mix stays possible).
pub fn player_attack(ctx: &ReducerContext, address: &str, direction: i32) -> Result<(), String> {
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    if !is_alive(&p, now) {
        return Err("Dead — respawn timer is still running".to_string());
    }
    let weapon = combat::weapon(p.weapon).ok_or("Weapon tier vanished — unequipped".to_string())?;
    if p.attack_cd_until > now {
        return Err(format!("Attack cooldown ({}s)", p.attack_cd_until - now));
    }
    // Spec 034 S1: strikes no longer share the build-action ACTION_COOLDOWN —
    // the weapon's own cd (≥ 1 server round) is the only attack gate, so a
    // 1 s weapon actually swings every 1 s. Build/repair keep their 5 s gate
    // in defense.rs; melee and building stop fighting over one meter.
    let range = weapon.range_hexes.max(1);
    // §14 addendum: the bow draws one Arrow per real strike; a dry stack
    // refuses the shot before the arc pick (empty arcs still cost the cd
    // but never an arrow — you didn't release).
    if let Some(ammo) = weapon.ammo {
        if crate::objects::count_item(ctx, &p.address, ammo) < 1 {
            return Err(format!("No {} — craft more in the forge", ammo));
        }
    }

    // Arc pick: nearest hostile mob facing the strike's 8-way heading.
    // REV (hitbox playtest): the arc vector reads the REAL positions
    // (fine player/mob transforms with slot offsets + roam interpolation),
    // not tile centers — the swing must land where the sprites stand. A
    // mob inside the player's OWN hex is a contact hit — its zero-length
    // delta rejects every heading otherwise.
    let (pq, pr) = (p.hex_q, p.hex_r);
    let candidates: Vec<(u64, i32, f32, f32)> = ctx
        .db
        .mob()
        .iter()
        .filter(|m| crate::types::mob_is_active(m, now))
        .filter(|m| tile_distance(m.hex_q, m.hex_r, pq, pr) <= range)
        .map(|m| {
            let d = tile_distance(m.hex_q, m.hex_r, pq, pr);
            if d == 0 {
                // Same-hex contact: a synthetic straight-ahead delta so the
                // arc gate passes regardless of heading.
                let (dx, dy) = idlecore_core::player_combat::DIR_VECS
                    [(((direction % 8) + 8) % 8) as usize];
                (m.mob_id, 0, dx as f32, dy as f32)
            } else {
                (
                    m.mob_id,
                    d,
                    m.position_x - p.position_x,
                    m.position_y - p.position_y,
                )
            }
        })
        .collect();
    let Some(target) = pick_arc_target(&candidates, direction, range) else {
        // Facing an empty arc costs nothing — but the strike still cadences.
        p.attack_cd_until = now + weapon.cd_secs;
        ctx.db.player().address().update(p);
        return Ok(());
    };
    let Some(mob_row) = ctx.db.mob().mob_id().find(target) else {
        return Ok(());
    };

    // Released on target: spend the arrow now (empty arcs cost cd only).
    if let Some(ammo) = weapon.ammo {
        crate::objects::remove_item(ctx, &p.address, ammo, 1);
    }

    // First attack sheds any fresh-spawn grace (Spec 027 §6).
    if p.grace_until > now {
        p.grace_until = 0;
    }

    // Swing fx leaves before the row math (fire-and-forget, §8 exhaustively
    // pinned kinds). Spec 034 S3: the detail carries `"<weapon>|<dmg>"` so
    // the client floats the number without a schema change.
    emit_player_event(
        ctx,
        CE_PLAYER_HIT,
        mob_row.hex_id,
        0,
        mob_row.mob_id,
        &format!("{}|{}", weapon.name, weapon.dmg),
        now,
    );

    // Damage → kill/wound → aggro.
    let capitals: HashMap<u64, Capital> = ctx.db.capital().iter().map(|c| (c.capital_id, c)).collect();
    kill_or_wound(ctx, &mob_row, weapon.dmg, &capitals, Some(&address));
    if let Some(mut fresh) = ctx.db.mob().mob_id().find(target) {
        fresh.aggro_target = p.address.clone();
        fresh.aggro_until_secs = now + combat::MOB_AGGRO_SECS;
        // Zelda feel REV: a hit SHOVES the wounded raider one hex along the
        // swing direction. Rooted kinds and water absolve it in place.
        let kind = cfg::mob_kind(&mob_row.kind);
        let (dx, dy) = idlecore_core::player_combat::DIR_VECS
            [(((direction % 8) + 8) % 8) as usize];
        // The vectors are ×20 fixed-point; the shove is ONE king-step.
        let (kq, kr) = (dx.signum(), dy.signum());
        if fresh.hp > 0
            && kind.is_some_and(|k| k.step_interval_secs > 0)
            && !ctx
                .db
                .hex_tile()
                .hex_id()
                .find(tile_id_of(fresh.hex_q + kq, fresh.hex_r + kr))
                .map(|t| t.terrain == "Water")
                .unwrap_or(false)
        {
            fresh.hex_q += kq;
            fresh.hex_r += kr;
            fresh.hex_id = tile_id_of(fresh.hex_q, fresh.hex_r);
            let (kx, ky) = tile_center(fresh.hex_q, fresh.hex_r);
            fresh.position_x = kx;
            fresh.position_y = ky;
        }
        ctx.db.mob().mob_id().update(fresh);
    }

    p.attack_cd_until = now + weapon.cd_secs;
    ctx.db.player().address().update(p);
    Ok(())
}

// ---------------------------------------------------------------------------
// player_dash
// ---------------------------------------------------------------------------

/// Dash (§5.2): 6 s cooldown furnace; the 0.4 s burst is one server tick.
/// Direction rides along for symmetry — the SPEED budget lives in
/// `move_player` (validated there per step, exactly like normal travel).
pub fn player_dash(ctx: &ReducerContext, address: &str, direction: i32) -> Result<(), String> {
    let _ = direction;
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    if !is_alive(&p, now) {
        return Err("Dead — respawn timer is still running".to_string());
    }
    if p.dash_cd_until > now {
        return Err(format!("Dash cooldown ({}s)", p.dash_cd_until - now));
    }
    // A dash sheds fresh-spawn grace too (you lunged — you fight).
    if p.grace_until > now {
        p.grace_until = 0;
    }
    p.dash_until = now + combat::DASH_TICKS;
    p.dash_cd_until = now + combat::DASH_CD_SECS;
    ctx.db.player().address().update(p);
    Ok(())
}

// ---------------------------------------------------------------------------
// equip_weapon
// ---------------------------------------------------------------------------

/// Equip a weapon tier (§4 + Spec 033): tier 0 (Fists) is always free; a
/// tier whose item row is missing is a permanent purchase — its materials
/// are consumed once (legacy bitmask owners keep a free repair path).
pub fn equip_weapon(ctx: &ReducerContext, address: &str, tier: u8) -> Result<(), String> {
    let weapon = combat::weapon(tier).ok_or_else(|| format!("Unknown weapon tier {tier}"))?;
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    if !is_alive(&p, now) {
        return Err("Dead — respawn timer is still running".to_string());
    }
    if tier != 0 && crate::objects::count_item(ctx, &p.address, &weapon.name) == 0 {
        // Legacy owners (pre-matrix bitmask) repair free; new tiers pay.
        let legacy_bit = (tier < 16
            && p.weapons_owned != 0
            && p.weapons_owned & (1u16 << tier) != 0) as bool;
        if !legacy_bit {
            // Purchase: every ingredient must already be in the stacks
            // (checked before anything is spent — Spec 023 pattern).
            for (item, qty) in &weapon.cost {
                if crate::objects::count_item(ctx, &p.address, item) < u64::from(*qty) {
                    return Err(format!("Not enough materials ({item}×{qty})"));
                }
            }
            for (item, qty) in &weapon.cost {
                crate::objects::remove_item(ctx, &p.address, item, u64::from(*qty));
            }
        }
        // §14 (player request): the crafted weapon is a real inventory item —
        // it shows in the hotbar/panel and stays permanently (never spent).
        crate::objects::add_item(ctx, &p.address, &weapon.name, 1);
    }
    p.weapon = tier;
    ctx.db.player().address().update(p);
    Ok(())
}

// ---------------------------------------------------------------------------
// craft_armor (Spec 033)
// ---------------------------------------------------------------------------

/// Buy an armor piece: permanent item; owning it is wearing it — `def`
/// applies to every enemy hit in `apply_player_damage`.
pub fn craft_armor(ctx: &ReducerContext, address: &str, id: u8) -> Result<(), String> {
    let piece = combat::armor(id).ok_or_else(|| format!("Unknown armor piece {id}"))?;
    let now = now_secs(ctx);
    let p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    if !is_alive(&p, now) {
        return Err("Dead — respawn timer is still running".to_string());
    }
    if crate::objects::count_item(ctx, &p.address, &piece.name) > 0 {
        return Err(format!("Already crafted ({})", piece.name));
    }
    for (item, qty) in &piece.cost {
        if crate::objects::count_item(ctx, &p.address, item) < u64::from(*qty) {
            return Err(format!("Not enough materials ({item}×{qty})"));
        }
    }
    for (item, qty) in &piece.cost {
        crate::objects::remove_item(ctx, &p.address, item, u64::from(*qty));
    }
    crate::objects::add_item(ctx, &p.address, &piece.name, 1);
    Ok(())
}

/// Total flat defense from owned armor pieces (Spec 033).
pub fn player_defense(ctx: &ReducerContext, address: &str) -> u32 {
    combat::armors()
        .iter()
        .filter(|a| crate::objects::count_item(ctx, address, &a.name) > 0)
        .map(|a| a.def)
        .sum()
}

// ---------------------------------------------------------------------------
// craft_arrows (§14.2)
// ---------------------------------------------------------------------------

/// `crate_arrows` material math, pure: needed (Wood, Stone) for `count`.
pub fn arrow_materials(count: u32) -> (u64, u64) {
    let mut wood = 0u64;
    let mut stone = 0u64;
    for (item, qty) in combat::ARROW_COST {
        match *item {
            crate::types::ITEM_WOOD => wood += u64::from(*qty),
            crate::types::ITEM_STONE => stone += u64::from(*qty),
            _ => {}
        }
    }
    let n = u64::from(count);
    (wood * n, stone * n)
}

/// Craft `count` Arrows at once (§14.2): 2 Wood + 1 Stone each, straight
/// from the stacks — the forge panel is the discovery surface; the standalone
/// check gate matches the bench-free `equip_weapon` model.
pub fn craft_arrows(ctx: &ReducerContext, address: &str, count: u32) -> Result<(), String> {
    if count == 0 || count > combat::ARROW_BATCH_CAP {
        return Err(format!("Batch must be 1..={}", combat::ARROW_BATCH_CAP));
    }
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    if !is_alive(&p, now) {
        return Err("Dead — respawn timer is still running".to_string());
    }
    let (wood, stone) = arrow_materials(count);
    if crate::objects::count_item(ctx, &p.address, crate::types::ITEM_WOOD) < wood {
        return Err(format!("Not enough materials (Wood×{wood})"));
    }
    if crate::objects::count_item(ctx, &p.address, crate::types::ITEM_STONE) < stone {
        return Err(format!("Not enough materials (Stone×{stone})"));
    }
    let mut spent_wood: u64 = 0;
    let mut spent_stone: u64 = 0;
    for (item, qty) in combat::ARROW_COST {
        let qty = u64::from(*qty) * u64::from(count);
        if crate::objects::remove_item(ctx, &p.address, item, qty) {
            match *item {
                crate::types::ITEM_WOOD => spent_wood = wood,
                crate::types::ITEM_STONE => spent_stone = stone,
                _ => {}
            }
        } else {
            // Race loss: bidder checked, stack gone — refund exactly what
            // the earlier removes spent so the batch stays all-or-nothing.
            crate::objects::add_item(ctx, &p.address, crate::types::ITEM_WOOD, spent_wood);
            crate::objects::add_item(ctx, &p.address, crate::types::ITEM_STONE, spent_stone);
            return Err(format!("Not enough {item}"));
        }
    }
    crate::objects::add_item(
        ctx,
        &p.address,
        idlecore_core::defense_config::ITEM_ARROW,
        u64::from(count),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 1s-tick passes — roamers + player lifecycle
// ---------------------------------------------------------------------------

/// Every 1 second (called from `mobs::tick` first): roamers (wander, contact
/// aggro, lifetime) and player lifecycle (regen, death, respawn, roamer
/// spawner). Raid-mob handling stays in the raid pass of `mobs::tick`.
pub fn tick(ctx: &ReducerContext) {
    let now = now_secs(ctx);
    mob_spawner_pass(ctx, now);
    roamer_pass(ctx, now);
    player_pass(ctx, now);
}

/// Spawn one Slime roaming pack at a legal hex (§7): ≥3 hexes from every
/// capital, ≤20 from some online player, on materialized grass. Global cap
/// `ROAMER_PACK_CAP` packs; an attempt every `ROAMER_SPAWN_PERIOD_SECS`.
fn mob_spawner_pass(ctx: &ReducerContext, now: u64) {
    if now % combat::ROAMER_SPAWN_PERIOD_SECS != 0 {
        return;
    }
    let packs: u32 = ctx
        .db
        .mob()
        .iter()
        .filter(|m| m.capital_id == 0 && m.pack_id > 0)
        .map(|m| m.pack_id)
        .collect::<std::collections::HashSet<u64>>()
        .len() as u32;
    if packs >= combat::ROAMER_PACK_CAP {
        return;
    }
    let capitals: Vec<Capital> = ctx.db.capital().iter().collect();
    let players: Vec<Player> = ctx
        .db
        .player()
        .iter()
        .filter(|p| p.status == "online" && is_alive(p, now))
        .collect();
    if players.is_empty() {
        return;
    }
    // Legal spawn nodes — deterministic scan, hash-picked per attempt.
    let candidates: Vec<(i32, i32)> = ctx
        .db
        .hex_tile()
        .iter()
        .filter(|t| t.terrain == "Grass")
        .map(|t| {
            let (q, r) = hex_coords_of(t.hex_id);
            (q, r)
        })
        .filter(|&(q, r)| {
            capitals
                .iter()
                .all(|c| tile_distance(q, r, hex_coords_of(c.hex_id).0, hex_coords_of(c.hex_id).1) >= combat::ROAMER_MIN_CAPITAL_DIST)
                && players.iter().any(|p| {
                    tile_distance(q, r, p.hex_q, p.hex_r) <= combat::ROAMER_MAX_PLAYER_DIST
                        && tile_distance(q, r, p.hex_q, p.hex_r)
                            >= combat::ROAMER_CONTACT_DIST
                })
        })
        .collect();
    if candidates.is_empty() {
        return;
    }
    let pick = (now.wrapping_mul(0x9E3779B97F4A7C15)) as usize % candidates.len();
    let (sq, sr) = candidates[pick];
    // Pack size 2–4 deterministic; packs own their spawn second as pack id.
    let span = (combat::ROAMER_PACK_MAX - combat::ROAMER_PACK_MIN + 1) as u64;
    let size = combat::ROAMER_PACK_MIN as u64
        + ((now / combat::ROAMER_SPAWN_PERIOD_SECS + pack_tag(sq, sr)) % span);
    let count = size as u32;
    for _ in 0..count {
        let (x, y) = tile_center(sq, sr);
        ctx.db.mob().insert(Mob {
            mob_id: 0,
            capital_id: 0,
            wave_id: 0,
            kind: "Slime".to_string(),
            hex_id: tile_id_of(sq, sr),
            hex_q: sq,
            hex_r: sr,
            position_x: x,
            position_y: y,
            hp: cfg::mob_kind("Slime").map(|k| k.hp).unwrap_or(22),
            step_accum_secs: 0,
            pinned_until_secs: 0,
            aggro_target: String::new(),
            aggro_until_secs: 0,
            pack_id: now,
            born_at_secs: now,
            spawn_at_secs: 0,
            // Roamers march free (no capital trace) — lane index unused.
            path_idx: 0,
        });
    }
    tracing::info!("ROAMER-PACK #{now} slimes={count} at ({sq},{sr})");
}

/// Pure pack-size jitter bump (unit-testable): a deterministic per-hex mix
/// so two packs born the same second differ when possible.
fn pack_tag(q: i32, r: i32) -> u64 {
    ((q as i64).wrapping_mul(3) ^ (r as i64).wrapping_mul(11)) as u64
}

/// Roamers each second: lifetime sweep, wander march (traps included),
/// contact aggro, aggroed pursuit + strike at the hunted player.
fn roamer_pass(ctx: &ReducerContext, now: u64) {
    // Standing walls block roamer wander/pursuit steps like any march.
    let blocked: std::collections::HashSet<(i32, i32)> = ctx
        .db
        .defense_structure()
        .iter()
        .filter(|s| cfg::defense_kind(&s.kind).is_some_and(|k| k.is_wall))
        .map(|s| hex_coords_of(s.hex_id))
        .collect();

    // Online, living players for contact checks (hex + address).
    let targets: Vec<(String, u64, i32, i32)> = ctx
        .db
        .player()
        .iter()
        .filter(|p| p.status == "online" && is_alive(p, now) && p.grace_until <= now)
        .map(|p| (p.address.clone(), p.hex_id, p.hex_q, p.hex_r))
        .collect();

    for m in ctx.db.mob().iter().filter(|m| m.capital_id == 0).collect::<Vec<Mob>>() {
        let Some(kind) = cfg::mob_kind(&m.kind) else {
            ctx.db.mob().mob_id().delete(m.mob_id);
            continue;
        };

        // Lifetime: 300 s regardless of mood (worlds don't hoover litter).
        if m.born_at_secs > 0 && now.saturating_sub(m.born_at_secs) >= combat::ROAMER_LIFETIME_SECS {
            ctx.db.mob().mob_id().delete(m.mob_id);
            continue;
        }

        // Contact aggro: a player walking into the roamer's reach turns it
        // (dist ≤ ROAMER_CONTACT_DIST, §7 "walks into their hex").
        if m.aggro_until_secs <= now {
            if let Some((addr, _, pq, pr)) = targets
                .iter()
                .find(|(_, _, pq, pr)| tile_distance(m.hex_q, m.hex_r, *pq, *pr) <= combat::ROAMER_CONTACT_DIST)
            {
                let mut row = m.clone();
                row.aggro_target = addr.clone();
                row.aggro_until_secs = now + combat::MOB_AGGRO_SECS;
                ctx.db.mob().mob_id().update(row);
            }
        }

        // Aggroed: strike at reach and glide toward the hunted player at the
        // march cadence (roamers never siege structures — swing through the
        // capital's defenses stays the player's trick).
        if m.aggro_until_secs > now && m.pinned_until_secs <= now {
            let mut row = m.clone();
            row.step_accum_secs += 1;
            if row.step_accum_secs >= kind.step_interval_secs.max(1) {
                row.step_accum_secs = 0;
                // Pursuit step: closest strictly-closing neighbor toward the
                // player's hex; no target = stationary (player offline/dead).
                if let Some((_, _, pq, pr)) =
                    hunted_player_pos(ctx, &row.aggro_target, now)
                {
                    let cur = tile_distance(m.hex_q, m.hex_r, pq, pr);
                    let closing = TileCoord::new(m.hex_q, m.hex_r)
                        .neighbors()
                        .into_iter()
                        .filter(|&(nq, nr)| {
                            land_at(ctx, tile_id_of(nq, nr))
                                && !blocked.contains(&(nq, nr))
                                && tile_distance(nq, nr, pq, pr) < cur
                        })
                        .min_by_key(|&(nq, nr)| tile_distance(nq, nr, pq, pr));
                    if let Some((nq, nr)) = closing {
                        let (x, y) = tile_center(nq, nr);
                        row.hex_q = nq;
                        row.hex_r = nr;
                        row.hex_id = tile_id_of(nq, nr);
                        row.position_x = x;
                        row.position_y = y;
                    }
                }
            }
            roamer_trap_spring(ctx, &row, now);
            ctx.db.mob().mob_id().update(row.clone());
            let _ = roamer_strike(ctx, &row, kind, now);
            continue;
        }

        // Not aggroed: the wander cadence is one step per interval ×4 (§7).
        let interval = kind.step_interval_secs * combat::ROAMER_WANDER_CADENCE;
        let mut row = m.clone();
        row.step_accum_secs += 1;
        if row.step_accum_secs < interval {
            ctx.db.mob().mob_id().update(row);
            continue;
        }
        row.step_accum_secs = 0;
        let dir = combat::roamer_wander_dir(m.mob_id, now, m.hex_q, m.hex_r);
        let (nq, nr) = TileCoord::new(m.hex_q, m.hex_r).neighbors()[dir];
        let nid = tile_id_of(nq, nr);
        if land_at(ctx, nid) && !blocked.contains(&(nq, nr)) {
            let (x, y) = tile_center(nq, nr);
            row.hex_q = nq;
            row.hex_r = nr;
            row.hex_id = nid;
            row.position_x = x;
            row.position_y = y;
            ctx.db.mob().mob_id().update(row.clone());
            roamer_trap_spring(ctx, &row, now);
        } else {
            ctx.db.mob().mob_id().update(row);
        }
    }

    // Aggro markers with no reachable target decay naturally by stamp.
}

/// Spring a live spike/web trap sitting on `mob`'s CURRENT hex (Roam 027):
/// same consume-on-trigger rule raiders follow — trap deleted, burst dmg or
/// pin applied, trap-owner-credited kill.
fn roamer_trap_spring(ctx: &ReducerContext, mob_row: &Mob, now: u64) {
    let (mq, mr) = (mob_row.hex_q, mob_row.hex_r);
    let trap = ctx
        .db
        .defense_structure()
        .iter()
        .map(|s| {
            let (q, r) = hex_coords_of(s.hex_id);
            (s, q, r)
        })
        .find(|(s, q, r)| {
            (*q, *r).eq(&(mq, mr)) && cfg::defense_kind(&s.kind).is_some_and(|k| k.trap_dmg > 0 || k.pin_secs > 0)
        });
    let Some((t, _, _)) = trap else {
        return;
    };
    let kind = cfg::defense_kind(&t.kind);
    let owner = t.owner.clone();
    let hex_id = t.hex_id;
    ctx.db.defense_structure().structure_id().delete(t.structure_id);
    emit_player_event(
        ctx,
        CE_TRAP,
        hex_id,
        0,
        mob_row.mob_id,
        kind.map(|k| k.name).unwrap_or("Trap"),
        now,
    );
    if let Some(k) = kind {
        if k.trap_dmg > 0 {
            let capitals: HashMap<u64, Capital> =
                ctx.db.capital().iter().map(|c| (c.capital_id, c)).collect();
            let mut fallen = mob_row.clone();
            kill_or_wound(ctx, &fallen, k.trap_dmg, &capitals, Some(&owner));
        }
        if k.pin_secs > 0 {
            if let Some(mut fresh) = ctx.db.mob().mob_id().find(mob_row.mob_id) {
                fresh.pinned_until_secs = now.saturating_add(u64::from(k.pin_secs));
                ctx.db.mob().mob_id().update(fresh);
            }
        }
    }
}

/// Aggroed roamer strike: dps into the hunted player when in reach. Returns
/// true when damage was applied (i-frame/grace rejections included).
fn roamer_strike(ctx: &ReducerContext, mob_row: &Mob, kind: &cfg::MobKind, now: u64) -> bool {
    let Some(player_row) = hunted_player(ctx, &mob_row.aggro_target, now) else {
        return false;
    };
    let dist = tile_distance(mob_row.hex_q, mob_row.hex_r, player_row.hex_q, player_row.hex_r);
    if dist > kind.range_hexes {
        return false;
    }
    let dmg = kind.dps;
    apply_player_damage(ctx, &player_row, dmg, mob_row.mob_id, kind.name, now)
}

/// Player range checks + real damage application shared by roamers and
/// aggroed raiders (§8): applies dps, refreshes `hostile_until`, and runs
/// the death flow at 0 (CE_PLAYER_HURT carries the fx).
/// Shared player-damage application (§8): dash i-frames and grace block
/// first; then dps lands, `hostile_until` refreshes, and 0 HP stamps the
/// death. `CE_PLAYER_HURT` fx rides out.
fn apply_player_damage(
    ctx: &ReducerContext,
    target: &Player,
    dmg: u32,
    attacker_mob: u64,
    detail: &str,
    now: u64,
) -> bool {
    if target.dash_until > now {
        return false; // i-frames vs. attacks only (§5.2)
    }
    // Spec 033: armor grants flat damage reduction floor 1 (never fully
    // immune — armor only softens hits).
    let dmg = dmg.saturating_sub(player_defense(ctx, &target.address)).max(1);
    let cur = hp_of(target);
    // REV (damage numbers): the detail carries the dmg (`"<kind>|<dmg>"`)
    // so the client pops a red number where the player got hit.
    emit_player_event(
        ctx,
        CE_PLAYER_HURT,
        target.hex_id,
        0,
        attacker_mob,
        &format!("{detail}|{dmg}"),
        now,
    );
    let mut s = target.clone();
    if cur > dmg {
        s.player_hp = cur - dmg;
        s.hostile_until = now + combat::MOB_AGGRO_SECS;
    } else {
        // Death (§6): 10 s corpse window, then capital respawn.
        s.player_hp = 0;
        s.death_until = now + combat::DEATH_RESPAWN_SECS;
        s.dash_until = 0;
        s.hostile_until = 0;
        tracing::info!("PLAYER-DIED {} at ({},{})", target.address, target.hex_q, target.hex_r);
    }
    ctx.db.player().address().update(s);
    true
}

/// Hunted player's hex for pursuit marching.
fn hunted_player_pos(ctx: &ReducerContext, address: &str, now: u64) -> Option<(String, u64, i32, i32)> {
    hunted_player(ctx, address, now).map(|p| (p.address.clone(), p.hex_id, p.hex_q, p.hex_r))
}

/// The hunted player if they still exist, are online, alive and out of
/// grace — otherwise None (aggro falls through to normal behavior).
fn hunted_player(ctx: &ReducerContext, address: &str, now: u64) -> Option<Player> {
    if address.is_empty() {
        return None;
    }
    ctx.db
        .player()
        .address()
        .find(&address.to_string())
        .filter(|p| p.status == "online" && is_alive(p, now) && p.grace_until <= now)
}

/// Player lifecycle each second: out-of-combat regen (+2/tick past
/// `hostile_until`), death → respawn at the capital with 60 s grace.
fn player_pass(ctx: &ReducerContext, now: u64) {
    for p in ctx.db.player().iter().collect::<Vec<Player>>() {
        // Deaths: 0 HP stamps death_until; the respawn transition is a
        // single write per player per second.
        if !is_alive(&p, now) {
            if let Some(capital) = ctx
                .db
                .capital()
                .capital_by_owner()
                .filter(&p.address)
                .find(|c| !c.destroyed)
            {
                if p.death_until <= now {
                    // Respawn (§6): full HP at the capital hex + 60 s grace.
                    let mut s = p.clone();
                    let (cq, cr) = hex_coords_of(capital.hex_id);
                    let (cx, cy) = tile_center(cq, cr);
                    s.hex_id = capital.hex_id;
                    s.hex_q = cq;
                    s.hex_r = cr;
                    s.position_x = cx;
                    s.position_y = cy;
                    s.player_hp = combat::player_max_hp(p.level);
                    s.grace_until = now + combat::RESPAWN_GRACE_SECS;
                    s.death_until = 0;
                    s.dash_until = 0;
                    s.hostile_until = 0;
                    ctx.db.player().address().update(s);
                    tracing::info!("PLAYER-RESPAWN {}", p.address);
                }
            }
            continue;
        }
        // Regen (§6): out of combat, +2 HP per tick, capped, nothing at max.
        let max = combat::player_max_hp(p.level);
        let cur = hp_of(&p);
        if cur < max && cur > 0 && p.hostile_until < now {
            let mut s = p.clone();
            s.player_hp = (cur + combat::PLAYER_REGEN_PER_TICK).min(max);
            ctx.db.player().address().update(s);
        }
    }
}

// ---------------------------------------------------------------------------
// mob→player strike branch (raid pass hook, §8)
// ---------------------------------------------------------------------------

/// Given an aggroed RAIDER mob, attack its hunted player when in reach.
/// Called from the raid pass of `mobs::tick` (§8). i-frames/grace/death all
/// block; returns true when the mob's attention was consumed this tick.
pub fn raider_strike(ctx: &ReducerContext, mob_row: &Mob, now: u64) -> bool {
    let Some(kind) = cfg::mob_kind(&mob_row.kind) else { return false };
    roamer_strike(ctx, mob_row, kind, now)
}

/// Does this mob currently hunt an attackable player? (march target = the
/// player's hex instead of the capital).
pub fn hunting_player_hex(ctx: &ReducerContext, mob_row: &Mob, now: u64) -> Option<(i32, i32)> {
    if mob_row.aggro_until_secs <= now {
        return None;
    }
    hunted_player(ctx, &mob_row.aggro_target, now).map(|p| (p.hex_q, p.hex_r))
}

/// fx writer for player events: reuses the combat_event table's exact shape
/// (mobs.rs keeps its own emitter private); target hex-id driven.
pub(crate) fn emit_player_event(
    ctx: &ReducerContext,
    kind: &str,
    target_hex_id: u64,
    wave_id: u64,
    attacker_mob: u64,
    detail: &str,
    now: u64,
) {
    ctx.db.combat_event().insert(crate::types::CombatEvent {
        event_id: 0,
        capital_id: 0,
        wave_id,
        tick_at: now,
        kind: kind.to_string(),
        attacker_mob,
        target_hex_id,
        target_slot_x: -1,
        target_slot_y: -1,
        detail: detail.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_pick_nearest_in_cone() {
        // Two mobs, both in reach — but only the NE one inside the arc.
        let cand = vec![
            (7u64, 1, 0.0, 10.0),  // due north, dir = east → outside arc
            (9u64, 2, 1.0, 1.0),   // NE, dir = east, distance 2 → in head
        ];
        assert_eq!(pick_arc_target(&cand, 0, 2), Some(9));
        // No arc match → None.
        assert_eq!(
            pick_arc_target(&[(7u64, 1, -10.0, 0.0)], 0, 2),
            None,
            "behind you never takes the hit"
        );
        // Nearest wins ties in cone.
        let two = vec![
            (12u64, 1, 10.0, 0.0),
            (3u64, 2, 7.0, 7.0),
        ];
        assert_eq!(pick_arc_target(&two, 0, 2), Some(12));
    }

    #[test]
    fn legacy_hp_rows_read_full_until_first_write() {
        let mut p = test_player(0, 3);
        assert_eq!(hp_of(&p), combat::player_max_hp(3), "0 = unseeded");
        p.player_hp = 42;
        assert_eq!(hp_of(&p), 42);
    }

    #[test]
    fn death_window_gates_life() {
        let p = test_player(0, 2);
        assert!(is_alive(&p, 100));
        let mut dead = p.clone();
        dead.death_until = 90;
        assert!(!is_alive(&dead, 80));
        assert!(is_alive(&dead, 91), "corpse window passes");
    }

    #[test]
    fn arrow_batches_cost_two_wood_one_stone_each() {
        assert_eq!(arrow_materials(1), (2, 1));
        assert_eq!(arrow_materials(10), (20, 10));
        assert_eq!(arrow_materials(32), (64, 32), "batch cap bounds");
    }

    fn test_player(hp: u32, level: u32) -> Player {
        Player {
            address: "arthur".to_string(),
            identity: "id".to_string(),
            status: "online".to_string(),
            display_name: None,
            avatar: "Cube".to_string(),
            bio: None,
            level,
            total_xp: 0,
            gold: 0,
            usdt: 0,
            eco_points: 0,
            lifetime_gold_earned: 0,
            lifetime_gold_spent: 0,
            position_x: 0.0,
            position_y: 0.0,
            hex_q: 0,
            hex_r: 0,
            hex_id: 0,
            vehicle: "None".to_string(),
            cosmetics: "[]".to_string(),
            templates: "[]".to_string(),
            last_login: 0,
            last_seen: 0,
            last_action_at: 0,
            last_spend: 0,
            created_at: 0,
            rapid_login_count: 0,
            idle_gains_blocked_until: 0,
            total_play_time: 0,
            plants_planted: 0,
            plants_harvested: 0,
            pollution_cleaned: 0,
            templates_published: 0,
            templates_purchased: 0,
            player_hp: hp,
            weapon: 0,
            weapons_owned: 0,
            attack_cd_until: 0,
            dash_cd_until: 0,
            dash_until: 0,
            hostile_until: 0,
            death_until: 0,
            grace_until: 0,
        }
    }
}
