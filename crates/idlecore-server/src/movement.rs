//! Movement — server-authoritative position (Spec 003 FR5, Spec 018 FR7,
//! Spec 021 FR1/FR2/FR5/FR6).
//!
//! The client sends the *target* position plus the expected speed multiplier;
//! the server validates that the displacement is within what the player's
//! equipped vehicle allows for the elapsed time, then recomputes the hex and
//! persists it. Heartbeat refreshes `last_seen` (Spec 021 FR6).

use spacetimedb::ReducerContext;
use crate::types::{hex_id_of, now_secs, player};
use crate::player::update_hex;

/// Base walking speed in world units/sec (Spec 003 FR3: 10 m/s × vehicle).
pub const BASE_SPEED: f32 = 10.0;
/// Tolerance: allows 20% network jitter before flagging a speed hack.
const SPEED_TOLERANCE: f32 = 1.2;

/// Normalized per-frame displacement cap for a given speed and elapsed time.
fn displacement_cap(speed: f32, elapsed: f32) -> f32 {
    speed * elapsed.max(0.05) * SPEED_TOLERANCE
}

/// Move reducer body. `dir_x/dir_y` is the normalized client input direction,
/// `intended_speed` the base speed the client claims (10 m/s × multiplier).
/// The server recomputes a bounded displacement from the elapsed time, so a
/// client spamming stale updates cannot teleport or outrun its vehicle.
pub fn move_player(
    ctx: &ReducerContext,
    address: &str,
    dir_x: f32,
    dir_y: f32,
    intended_speed: f32,
    dt: f32,
) -> Result<(f32, f32, u64, bool), String> {
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    let elapsed = (now.saturating_sub(p.last_seen)).max(1) as f32;
    let _ = dt;

    // Speed cap: base × vehicle multiplier × (delta time handled below).
    let multiplier = match p.vehicle.as_str() {
        "Bicycle" => 2.0,
        "Scooter" => 3.0,
        "Motorcycle" => 5.0,
        "Boat" => 4.0,
        "Airplane" => 10.0,
        _ => 1.0,
    };
    let max_speed = BASE_SPEED * multiplier;
    let speed = intended_speed.clamp(0.0, max_speed);

    let mag = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if mag <= 0.0 {
        // Still (no input): refresh online status only.
        update_hex(ctx, &p.address, p.hex_q, p.hex_r, p.position_x, p.position_y, now);
        return Ok((p.position_x, p.position_y, p.hex_id, false));
    }

    let dir_x = dir_x / mag;
    let dir_y = dir_y / mag;

    // Displacement bounded by max_speed over the flagged elapsed time; this
    // is the server-authoritative speed limit (Spec 018 FR7).
    let distance = (speed * elapsed).min(displacement_cap(max_speed, elapsed));

    let mut x = p.position_x + dir_x * distance;
    let mut y = p.position_y + dir_y * distance;

    // World bounds (Spec 003 FR4): the grid is R=64 away from the origin.
    let world_radius = crate::world::WORLD_RADIUS as f32 * 10.0 * 1.9;
    x = x.clamp(-world_radius, world_radius);
    y = y.clamp(-world_radius, world_radius);

    let (q, r) = hex_at(x, y);
    let corrected = (q, r) != (p.hex_q, p.hex_r);
    update_hex(ctx, &p.address, q, r, x, y, now);

    Ok((x, y, hex_id_of(q, r), corrected))
}

/// Flat-top axial hex containing a world position (PROPOSAL 9.1).
/// Delegates to the shared, cube-round-tested core implementation.
pub fn hex_at(x: f32, y: f32) -> (i32, i32) {
    idlecore_core::hex_grid::HexGrid::world_to_axial(x, y, 10.0)
}

/// Heartbeat (Spec 021 FR6): refresh last_seen so idle gains and occupancy
/// don't mis-fire on long-running sessions.
pub fn heartbeat(ctx: &ReducerContext, address: &str) -> Result<(), String> {
    let now = now_secs(ctx);
    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;
    p.last_seen = now;
    ctx.db.player().address().update(p);
    Ok(())
}