//! WASD Movement System
//!
//! Handles keyboard input and applies movement to the player with
//! vehicle speed multipliers and hex-clamped boundaries.

use bevy::prelude::*;
use crate::player::{Player, Vehicle};
use crate::hex::HexWorld;

/// Base movement speed: 10 m/s
const BASE_SPEED: f32 = 10.0;
/// Hex radius in world units
const HEX_RADIUS: f32 = 10.0;
/// Maximum movement per tick to prevent tunneling through hex boundaries
const MAX_MOVE_PER_TICK: f32 = 20.0;

/// System que aplica input WASD ao movimento do player
/// Calcula direção, aplica velocidade com veículo, clamp ao hex.
pub fn movement_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut Player)>,
) {
    let Ok((mut transform, mut player)) = player_query.get_single_mut() else {
        return;
    };

    // Initialize current hex if not set
    if player.current_hex.is_none() {
        player.current_hex = Some((0, 0));
    }

    // Gather WASD input
    let mut vx = 0.0f32;
    let mut vz = 0.0f32;

    if keyboard.pressed(KeyCode::KeyW) {
        vz -= 1.0;
    } else if keyboard.pressed(KeyCode::KeyS) {
        vz += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        vx -= 1.0;
    } else if keyboard.pressed(KeyCode::KeyD) {
        vx += 1.0;
    }

    // Get vehicle speed multiplier
    let vehicle = player.owned_vehicle().copied();
    let speed_multiplier = vehicle.map_or(1.0, |v| v.speed_multiplier());
    let speed = BASE_SPEED * speed_multiplier;

    // Normalize movement direction
    let len = (vx * vx + vz * vz).sqrt();
    if len > 0.0 {
        vx /= len;
        vz /= len;
    }

    // Calculate delta position
    let dt = time.delta_secs();
    if len > 0.0 {
        let delta = speed * dt;
        let move_x = vx * delta;
        let move_z = vz * delta;

        // Clamp movement to prevent tunneling
        let actual_delta_x = move_x.clamp(-MAX_MOVE_PER_TICK, MAX_MOVE_PER_TICK);
        let actual_delta_z = move_z.clamp(-MAX_MOVE_PER_TICK, MAX_MOVE_PER_TICK);

        let old_pos = transform.translation;
        let new_pos = Vec3::new(
            old_pos.x + actual_delta_x,
            old_pos.y,
            old_pos.z + actual_delta_z,
        );
        transform.translation = new_pos;
    }

    // Update hex tracking and velocity
    let new_hex = Player::world_to_hex(
        transform.translation.x,
        transform.translation.z,
        HEX_RADIUS,
    );
    player.current_hex = Some(new_hex);
    player.velocity = Vec2::new(vx * speed * dt, vz * speed * dt);
    player.position = transform.translation;

    // Zero velocity when not moving
    if len == 0.0 {
        player.velocity = Vec2::ZERO;
    }

    // Apply idle gains on login (if offline > 1 minute)
    if let Some((q, r)) = new_hex {
        // Check if player has idle time to claim
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_login = player.economy.last_login_time;
        let seconds_offline = now.saturating_sub(last_login);

        if seconds_offline > 60 {
            let gains = apply_idle_gains_on_login(player);
            if gains.xp > 0 || gains.gold > 0 {
                println!(
                    "[IDLE] Applied idle gains: {} XP, {} Gold (offline ~{}s)",
                    gains.xp, gains.gold, seconds_offline
                );
            }
        }
    }
}

/// System que aplica WASD input ao movimento com veículo
pub fn update_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut ClientPlayer, &mut Transform)>,
) {
    // Placeholder — actual movement is handled by movement_system
    // This will be populated when ClientPlayer system is fully integrated
}

/// Apply idle gains when player logs in after being offline
fn apply_idle_gains_on_login(player: &mut Player) -> crate::idle::IdleGains {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let last_login = player.economy.last_login_time;
    let seconds_offline = now.saturating_sub(last_login);

    let gains = crate::idle::gains_for_time(std::time::Duration::from_secs(seconds_offline));

    player.economy.xp += gains.xp;
    player.economy.gold += gains.gold;

    // Recalculate level
    player.economy.level = crate::progression::calculate_level(player.economy.xp);
    player.economy.next_level_xp_needed = crate::economy::xp_for_next_level(player.economy.level);

    println!(
        "[LOGIN] Idle gains applied: {} XP, {} Gold (level now {})",
        gains.xp, gains.gold, player.economy.level
    );

    // Update last login time
    player.economy.last_login_time = now;

    gains
}

/// Debug helper: toggle vehicle from client
pub fn debug_toggle_vehicle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<&mut Player>,
) {
    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyV) {
        println!("Current vehicle: {:?}", player.owned_vehicle());
    }
}

/// Helper: Reset player to spawn point
pub fn reset_to_spawn(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut Player)>,
) {
    let Ok((mut transform, mut player)) = player_query.get_single_mut() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some((0, 0));
        player.velocity = Vec2::ZERO;
    }
}
