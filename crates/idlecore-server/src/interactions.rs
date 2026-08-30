//! Hex interactions — plant / harvest / clean (Spec 004).
//!
//! Server-authoritative with full validation and the PROPOSAL 3.3 conflict
//! rules: hex locking with a 2 s timeout, one plant per hex, planter-only
//! harvest rewards, 5 s action cooldown, max 8 players per hex.

use spacetimedb::{ReducerContext, Table};
use crate::economy::{add_eco_points, add_gold, add_xp, record_eco_tx, spend_gold};
use crate::types::{player, hex_tile,
    now_secs, Plant, PlotState, HexTile, ACTION_COOLDOWN_SECS, CLEAN_COST, CLEAN_GOLD_REWARD,
    CLEAN_XP_REWARD, ECO_FOR_CLEAN, ECO_FOR_HARVEST_TREE, ECO_FOR_PLANT_TREE,
    HARVEST_GOLD_REWARD, HARVEST_XP_REWARD, HEX_LOCK_TIMEOUT_SECS, MAX_PLAYERS_PER_HEX,
    RATING_FOR_CLEAN, RATING_FOR_HARVEST, RATING_FOR_PLANT, ITEM_SEED,
};
use crate::player::players_in_hex;

/// Outcome of an interaction, returned to the caller for logging.
#[derive(Clone)]
pub enum Outcome {
    Ok(String),
    Err(String),
}

impl Outcome {
    pub fn ok(msg: impl Into<String>) -> Self {
        Outcome::Ok(msg.into())
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Outcome::Err(msg.into())
    }
    pub fn into_result(self) -> Result<(), String> {
        match self {
            Outcome::Ok(_) => Ok(()),
            Outcome::Err(e) => Err(e),
        }
    }
}

/// Pure rule set used by preflight (Spec 004 FR1/NFR2/PROPOSAL 3.3) —
/// extracted so the guards are unit-testable with mock values.
fn interaction_checks(
    dist_to_hex: i32,
    now: u64,
    last_action_at: u64,
    hex_last_interaction: u64,
    occupants: usize,
) -> Result<(), String> {
    if dist_to_hex > 1 {
        return Err("Hex is out of interaction range (1 hex)".to_string());
    }
    if now.saturating_sub(last_action_at) < ACTION_COOLDOWN_SECS {
        return Err(format!("Action cooldown ({}s)", ACTION_COOLDOWN_SECS));
    }
    if now.saturating_sub(hex_last_interaction) < HEX_LOCK_TIMEOUT_SECS && hex_last_interaction != 0 {
        return Err("Hex busy — another player is acting on it".to_string());
    }
    if occupants >= MAX_PLAYERS_PER_HEX {
        return Err("Hex full (max 8 players)".to_string());
    }
    Ok(())
}

/// Shared pre-checks: player exists, hex exists, within 1-hex interaction
/// range, action cooldown respected, hex lock not held by someone else.
fn preflight(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
) -> Result<(crate::types::Player, HexTile), String> {
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    // Spec 004 FR1: interaction range is 1 hex.
    let (hq, hr) = crate::types::hex_coords_of(hex_id);
    crate::world::ensure_hex(ctx, hq, hr);
    let dist = crate::types::hex_distance(p.hex_q, p.hex_r, hq, hr);
    let hex = ctx
        .db
        .hex_tile()
        .hex_id()
        .find(hex_id)
        .ok_or_else(|| "Hex not found".to_string())?;
    interaction_checks(
        dist,
        now,
        p.last_action_at,
        hex.last_interaction,
        players_in_hex(ctx, hex_id),
    )?;

    p.last_action_at = now;
    Ok((p, hex))
}

fn commit_hex(ctx: &ReducerContext, mut hex: HexTile, now: u64) {
    hex.last_interaction = now;
    ctx.db.hex_tile().hex_id().update(hex);
}

/// Plant a seed at the given hex. Costs gold, gives +5 XP, grows over time
/// (Spec 004 FR2/FR6).
pub fn plant_at(ctx: &ReducerContext, address: &str, hex_id: u64, plant_type: &str) -> Outcome {
    if !Plant::valid_type(plant_type) {
        return Outcome::err("Unknown plant type");
    }
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };

    // PROPOSAL 3.3: one plant per hex, no planting on polluted/water hexes.
    if hex.plant.is_some() {
        return Outcome::err("Hex already has a plant");
    }
    if hex.is_polluted {
        return Outcome::err("Cannot plant on a polluted hex — clean it first");
    }
    if hex.terrain != "Grass" && hex.terrain != "Forest" {
        return Outcome::err(format!("Cannot plant on {} terrain", hex.terrain));
    }

    let cost = Plant::planting_cost(plant_type);
    if let Err(e) = spend_gold(ctx, &mut p, cost, "plant") {
        return Outcome::err(e);
    }

    let now = now_secs(ctx);
    let plant = Plant {
        plant_type: plant_type.to_string(),
        planted_at: now,
        growth_time: Plant::growth_seconds(plant_type),
    };
    let mut hex = hex;
    let rating_before = hex.eco_rating;
    hex.plant = Some(plant.to_json());
    hex.planted_by = Some(p.address.clone());
    hex.eco_rating = (rating_before + RATING_FOR_PLANT).min(100);
    let after = hex.eco_rating;
    commit_hex(ctx, hex, now);

    p.plants_planted = p.plants_planted.saturating_add(1);
    add_xp(ctx, &mut p, 5, "plant");
    // Spec 020: planting a tree earns eco points.
    if plant_type == "Tree" {
        add_eco_points(ctx, &mut p, ECO_FOR_PLANT_TREE, "plant_tree");
        record_eco_tx(ctx, &p.address, hex_id, "plant_tree", ECO_FOR_PLANT_TREE, rating_before, after);
    }
    ctx.db.player().address().update(p);

    Outcome::ok(format!("Planted {plant_type} (-{cost}G, +5 XP)"))
}

/// Terrain a Hoe can till into loose dirt for planting.
fn tillable_terrain() -> [&'static str; 7] {
    [
        "Grass", "Forest", "Grassland", "TropicalRainforest",
        "Tundra", "Taiga", "Desert",
    ]
}

/// Load the per-slot plot map from the hex's `plots` JSON (Stardew-style).
fn load_plot_map(hex: &HexTile) -> std::collections::HashMap<String, PlotState> {
    hex.plots
        .as_deref()
        .map(crate::types::plot_map_from_json)
        .unwrap_or_default()
}

/// Persist a mutated plot map back onto the hex row.
fn commit_plot_map(ctx: &ReducerContext, mut hex: HexTile, plots: &std::collections::HashMap<String, PlotState>, now: u64) {
    hex.plots = crate::types::plot_map_to_json(plots);
    commit_hex(ctx, hex, now);
}

/// Till the selected 16px slot with the Hoe (Spec 022 §5): no seed is
/// consumed here — it only turns the soil into loose dirt so a seed can be
/// planted, which then needs watering to grow.
pub fn till(ctx: &ReducerContext, address: &str, hex_id: u64, slot_x: i32, slot_y: i32) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };

    if hex.is_polluted {
        return Outcome::err("Cannot till a polluted hex — clean it first");
    }
    if !tillable_terrain().contains(&hex.terrain.as_str()) {
        return Outcome::err(format!("Cannot till {} terrain", hex.terrain));
    }
    let mut plots = load_plot_map(&hex);
    let key = PlotState::key(slot_x, slot_y);
    if plots.get(&key).map(|p| p.tilled).unwrap_or(false) {
        return Outcome::err("This soil is already tilled");
    }
    plots.insert(
        key.clone(),
        PlotState {
            tilled: true,
            kind: None,
            planted_at: 0,
            watered_at: None,
            growth_time: 0,
            planted_by: None,
        },
    );
    let now = now_secs(ctx);
    commit_plot_map(ctx, hex, &plots, now);

    add_xp(ctx, &mut p, 2, "till");
    ctx.db.player().address().update(p);

    Outcome::ok("Tilled the soil — plant a seed with a Seed item")
}

/// Plant a seed onto a plot that has been tilled by a Hoe. Consumes one Seed.
/// The planted crop does not grow until it is watered.
pub fn plant_plot(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
    kind: Option<String>,
) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };
    if hex.is_polluted {
        return Outcome::err("Cannot plant on a polluted hex — clean it first");
    }
    let mut plots = load_plot_map(&hex);
    let key = PlotState::key(slot_x, slot_y);
    let Some(mut plot) = plots.get(&key).cloned() else {
        return Outcome::err("Till the soil first with a Hoe");
    };
    if !plot.tilled {
        return Outcome::err("Soil not tilled — use a Hoe first");
    }
    if plot.kind.is_some() {
        return Outcome::err("This plot is already planted");
    }
    if !crate::objects::remove_item(ctx, &p.address, ITEM_SEED, 1) {
        return Outcome::err("No seeds — destroy tall grass first");
    }

    let now = now_secs(ctx);
    let pt = kind.unwrap_or_else(|| "Wheat".to_string());
    if !Plant::valid_type(&pt) {
        return Outcome::err("Unknown plant type");
    }
    plot.kind = Some(pt.clone());
    plot.planted_at = now;
    plot.watered_at = None;
    plot.growth_time = Plant::growth_seconds(&pt);
    plot.planted_by = Some(p.address.clone());
    plots.insert(key, plot.clone());
    commit_plot_map(ctx, hex, &plots, now);

    add_xp(ctx, &mut p, 5, "plant");
    ctx.db.player().address().update(p);

    Outcome::ok(format!("Planted {pt} — water it to grow (-1 Seed)"))
}

/// Water a planted plot with the WateringCan (Spec 022 §5). A plot does not
/// grow until watered; watering sets the clock from which it matures.
pub fn water_plot(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };
    if hex.is_polluted {
        return Outcome::err("Cannot water a polluted hex — clean it first");
    }
    let mut plots = load_plot_map(&hex);
    let key = PlotState::key(slot_x, slot_y);
    let Some(mut plot) = plots.get(&key).cloned() else {
        return Outcome::err("No plot here — till the soil first");
    };
    let Some(kind) = plot.kind.clone() else {
        return Outcome::err("Nothing planted here to water");
    };
    let now = now_secs(ctx);
    if let Some(wa) = plot.watered_at {
        if now < plot.mature_at(wa) {
            return Outcome::err("This crop is already watered and growing");
        }
    }
    plot.watered_at = Some(now);
    plot.planted_at = plot.planted_at.max(1);
    plots.insert(key, plot.clone());
    commit_plot_map(ctx, hex, &plots, now);

    add_xp(ctx, &mut p, 1, "water");
    ctx.db.player().address().update(p);

    Outcome::ok(format!("Watered the {kind} — it is now growing"))
}

/// Harvest a mature planted plot. Only the planter earns the reward; the
/// soil stays tilled so it can be planted again.
pub fn harvest_plot(
    ctx: &ReducerContext,
    address: &str,
    hex_id: u64,
    slot_x: i32,
    slot_y: i32,
) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };
    let now = now_secs(ctx);
    let mut plots = load_plot_map(&hex);
    let key = PlotState::key(slot_x, slot_y);
    let Some(plot) = plots.get(&key).cloned() else {
        return Outcome::err("No plot here");
    };
    let Some(kind) = plot.kind.clone() else {
        return Outcome::err("Nothing planted here to harvest");
    };
    let Some(watered_at) = plot.watered_at else {
        return Outcome::err("This crop needs water before it can grow");
    };
    if now < plot.mature_at(watered_at) {
        return Outcome::err(format!(
            "Still growing ({}s left)",
            plot.mature_at(watered_at).saturating_sub(now)
        ));
    }
    let planter = plot.planted_by.clone().unwrap_or_default();
    if planter != p.address {
        return Outcome::err("Not your plant — only the planter earns the harvest");
    }

    let mut cleared = plot.clone();
    cleared.kind = None;
    cleared.planted_at = 0;
    cleared.watered_at = None;
    cleared.growth_time = 0;
    cleared.planted_by = None;
    plots.insert(key, cleared);
    let rating_before = hex.eco_rating;
    commit_plot_map(ctx, hex, &plots, now);

    add_gold(ctx, &mut p, HARVEST_GOLD_REWARD, "harvest");
    add_xp(ctx, &mut p, HARVEST_XP_REWARD, "harvest");
    if kind == "Tree" {
        add_eco_points(ctx, &mut p, ECO_FOR_HARVEST_TREE, "harvest_tree");
        record_eco_tx(ctx, &p.address, hex_id, "harvest_tree", ECO_FOR_HARVEST_TREE, rating_before, (rating_before + RATING_FOR_HARVEST).min(100));
    }
    ctx.db.player().address().update(p);

    Outcome::ok(format!(
        "Harvested {kind} (+{HARVEST_GOLD_REWARD}G, +{HARVEST_XP_REWARD} XP)"
    ))
}

/// Harvest a mature plant. +15G / +10XP for the planter only (PROPOSAL 3.3).
pub fn harvest(ctx: &ReducerContext, address: &str, hex_id: u64) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };
    let now = now_secs(ctx);

    let plant_str = match &hex.plant {
        Some(s) => s.clone(),
        None => return Outcome::err("No plant here".to_string()),
    };
    let plant = match Plant::from_json(&plant_str) {
        Some(p) => p,
        None => return Outcome::err("Corrupt plant data".to_string()),
    };

    if !plant.is_mature(now) {
        return Outcome::err(format!(
            "Plant not mature yet ({}s remaining)",
            plant.time_remaining(now)
        ));
    }

    let planter = hex.planted_by.clone().unwrap_or_default();
    if planter != p.address {
        // Non-planter harvests get no reward (PROPOSAL 3.3).
        return Outcome::err("Not your plant — only the planter earns the harvest");
    }

    let mut hex = hex;
    let rating_before = hex.eco_rating;
    hex.plant = None;
    hex.planted_by = None;
    hex.eco_rating = (rating_before + RATING_FOR_HARVEST).min(100);
    let after = hex.eco_rating;
    commit_hex(ctx, hex, now);

    p.plants_harvested = p.plants_harvested.saturating_add(1);
    add_gold(ctx, &mut p, HARVEST_GOLD_REWARD, "harvest");
    add_xp(ctx, &mut p, HARVEST_XP_REWARD, "harvest");
    if plant.plant_type == "Tree" {
        add_eco_points(ctx, &mut p, ECO_FOR_HARVEST_TREE, "harvest_tree");
        record_eco_tx(ctx, &p.address, hex_id, "harvest_tree", ECO_FOR_HARVEST_TREE, rating_before, after);
    }
    ctx.db.player().address().update(p);

    Outcome::ok(format!(
        "Harvested {} (+{}G, +{} XP)",
        plant.plant_type, HARVEST_GOLD_REWARD, HARVEST_XP_REWARD
    ))
}

/// Clean pollution: pay 20G, earn 20G back, +15 XP, +10 eco points,
/// +10 hex rating (PROPOSAL 2.3, Spec 020).
pub fn clean(ctx: &ReducerContext, address: &str, hex_id: u64) -> Outcome {
    let (mut p, hex) = match preflight(ctx, address, hex_id) {
        Ok(v) => v,
        Err(e) => return Outcome::err(e),
    };
    let now = now_secs(ctx);

    if !hex.is_polluted {
        return Outcome::err("Hex is not polluted".to_string());
    }

    if let Err(e) = spend_gold(ctx, &mut p, CLEAN_COST, "clean") {
        return Outcome::err(e);
    }
    add_gold(ctx, &mut p, CLEAN_GOLD_REWARD, "clean");

    let mut hex = hex;
    let rating_before = hex.eco_rating;
    hex.is_polluted = false;
    hex.eco_rating = (rating_before + RATING_FOR_CLEAN).min(100);
    hex.cleaned_at = Some(now);
    let after = hex.eco_rating;
    commit_hex(ctx, hex, now);

    p.pollution_cleaned = p.pollution_cleaned.saturating_add(1);
    add_xp(ctx, &mut p, CLEAN_XP_REWARD, "clean");
    add_eco_points(ctx, &mut p, ECO_FOR_CLEAN, "clean");
    record_eco_tx(ctx, &p.address, hex_id, "clean", ECO_FOR_CLEAN, rating_before, after);
    ctx.db.player().address().update(p);

    Outcome::ok(format!(
        "Cleaned pollution (net 0G, +{} XP, +{} eco)",
        CLEAN_XP_REWARD, ECO_FOR_CLEAN
    ))
}

/// Re-publish the state to make a currently-locked hex available again after
/// the lock timeout; called by the movement system when a player leaves.
pub fn release_lock(_ctx: &ReducerContext, _hex_id: u64) {
    // Locks are timestamp-based; they expire naturally via last_interaction.
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_adjacent_hex_passes() {
        assert!(interaction_checks(1, 1000, 990, 900, 1).is_ok());
    }

    #[test]
    fn mock_non_adjacent_hex_rejected() {
        let err = interaction_checks(2, 1000, 990, 900, 0).unwrap_err();
        assert!(err.contains("out of interaction range"));
    }

    #[test]
    fn mock_action_cooldown_rejected() {
        let err = interaction_checks(1, 1000, 998, 900, 0).unwrap_err();
        assert!(err.contains("cooldown"));
    }

    #[test]
    fn mock_hex_lock_rejected() {
        let err = interaction_checks(1, 1000, 990, 999, 0).unwrap_err();
        assert!(err.contains("busy"));
    }

    #[test]
    fn mock_full_hex_rejected() {
        let err = interaction_checks(1, 1000, 990, 900, 8).unwrap_err();
        assert!(err.contains("full"));
    }

    #[test]
    fn mock_plant_wait_harvest_flow() {
        // Plant → wait → harvest: maturity logic drives the wait (Spec 004 T5.5).
        let planted = Plant { plant_type: "Wheat".into(), planted_at: 1000, growth_time: 3600 };
        assert!(!planted.is_mature(2000));
        assert!(planted.is_mature(4600));
        assert_eq!(planted.time_remaining(2000), 2600);
    }
}

#[cfg(test)]
mod perf_tests {
    use super::*;

    #[test]
    fn many_interactions_fit_frame_budget() {
        // Spec 004 T6.2: many players interacting must not block a frame.
        // 10k full mock preflights (all checks) under 250 ms.
        let start = std::time::Instant::now();
        let mut ok = 0usize;
        for i in 0..10_000u32 {
            let now = 100_000 + i as u64;
            let dist = (i % 3) as i32;
            let occupants = (i % 10) as usize;
            if interaction_checks(dist, now, now - 6, now - 3, occupants).is_ok() {
                ok += 1;
            }
        }
        let elapsed = start.elapsed();
        assert!(ok > 0);
        assert!(
            elapsed.as_millis() < 250,
            "10k interaction checks took {elapsed:?}"
        );
    }
}
