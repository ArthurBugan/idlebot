//! Teleport system — teleport to hexes within view range.

use crate::economy;

// ---------------------------------------------------------------------------
// Teleport Config
// ---------------------------------------------------------------------------

/// Maximum teleport range in hexes from current position
pub const TELEPORT_RANGE_HEXES: i32 = 8;

/// View range for hex visibility (hexes within range)
pub const VIEW_RANGE_HEXES: i32 = 16;

// ---------------------------------------------------------------------------
// Teleport State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TeleportTarget {
    pub hex_id: u64,
    pub distance: i32, // hex distance from player
}

// ---------------------------------------------------------------------------
// Hex Navigation (simplified flat-map for local mode)
// ---------------------------------------------------------------------------

/// Calculate hex distance (simplified Chebyshev / Manhattan on flat grid)
pub fn hex_distance(hex_a: u64, hex_b: u64) -> i32 {
    // Extract approximate row/column from hex IDs
    // (simplified — real hex math would use axial coordinates)
    let row_a = (hex_a >> 32) as i32;
    let col_a = hex_a as i32;
    let row_b = (hex_b >> 32) as i32;
    let col_b = hex_b as i32;

    // Manhattan-like distance on the transformed grid
    let dist = (row_a - row_b).abs() + (col_a - col_b).abs();
    dist.min(TELEPORT_RANGE_HEXES as i32) as i32
}

/// Generate nearby hexes around a position (for minimap/teleport targets)
pub fn generate_nearby_hexes(current_hex_id: u64, range: i32) -> Vec<TeleportTarget> {
    let mut targets = Vec::new();

    let row = (current_hex_id >> 32) as i32;
    let col = current_hex_id as i32;

    for dr in -range..=range {
        for dc in -range..=range {
            let target_row = row + dr;
            let target_col = col + dc;

            // Skip the player's own hex
            if target_row == row && target_col == col {
                continue;
            }

            let target_hex = ((target_row as u64) << 32) | (target_col as u64);

            // Generate hex terrain labels for display
            let terrain_label = generate_terrain_label(target_row, target_col);

            let dist = i32::min((dr.abs() + dc.abs()), range);
            if dist <= range {
                targets.push(TeleportTarget {
                    hex_id: target_hex,
                    distance: dist,
                });
            }
        }
    }

    targets
}

/// Generate a terrain label for a hex based on grid position
fn generate_terrain_label(row: i32, col: i32) -> &'static str {
    if row.abs() > 5 { "Desert" }
    else if col.abs() > 5 { "City" }
    else if (row % 3 == 0 && col % 3 == 0) { "Forest" }
    else if ((row + col) % 5 == 0) { "Polluted" }
    else { "Grass" }
}

// ---------------------------------------------------------------------------
// Teleport Execution
// ---------------------------------------------------------------------------

/// Calculate teleport cost for current level
pub fn calc_teleport_cost(level: u32) -> u64 {
    let base = 100u64;
    (base as f64 * (level as f64).sqrt()).min((level as u64).pow(2) as f64) as u64
}

/// Attempt to teleport. Returns true if successful, false if not enough gold.
pub fn try_teleport(
    gs: &mut economy::LocalGameState,
    target_hex_id: u64,
    range: i32,
) -> bool {
    let cost = calc_teleport_cost(gs.economy.level);

    // Verify target is within range
    let dist = hex_distance(gs.current_hex_id, target_hex_id);
    if dist > range {
        println!("[TELEPORT] Target hex {} is too far (distance: {} > {})",
            target_hex_id, dist, range);
        return false;
    }

    // Check gold
    if gs.economy.gold < cost {
        println!("[TELEPORT] Not enough gold! Need {}G (have {}G)", cost, gs.economy.gold);
        return false;
    }

    // Spend gold
    economy::spend_gold(&mut gs.economy, cost);

    // Execute teleport
    gs.current_hex_id = target_hex_id;
    gs.nearby_hexes.clear();

    println!("[TELEPORT] Teleported to hex {} (cost: {}G, distance: {} hexes)",
        target_hex_id, cost, dist);

    true
}

// ---------------------------------------------------------------------------
// Teleport UI Data
// ---------------------------------------------------------------------------

/// Generate teleport options for the UI
pub fn get_teleport_options(gs: &economy::LocalGameState, player_hex: u64) -> Vec<TeleportTarget> {
    generate_nearby_hexes(player_hex, TELEPORT_RANGE_HEXES)
}

/// Format a teleport cost for display
pub fn format_teleport_cost(cost: u64) -> String {
    format!("{}G", cost)
}

/// Get level-dependent teleport cost display
pub fn get_teleport_cost_display(level: u32) -> String {
    let cost = calc_teleport_cost(level);
    format!("Teleport: {}G (level {}) | Root: 100G x sqrt({})",
        cost, level, level)
}
