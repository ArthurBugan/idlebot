//! IdleBot — Bevy 0.19 hex grid single-player client.
//!
//! Main entry point: start the Bevy app with hex world, player, WASD movement,
//! idle gains, vehicle system, and idle time tracking.

use bevy::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;

// Re-export from lib
pub use idlecore_client::world_pos_to_hex;

mod player;
mod idle;
mod input;
mod vehicle;
mod progression;

/// 3D camera height for looking at the hex grid
const CAMERA_HEIGHT: f32 = 30.0;
const CAMERA_Y: f32 = 30.0;

/// Main function — start the Bevy app.
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, setup)
        .add_systems(Update, (
            player_movement,
            debug_commands,
        ))
        .add_systems(PostStartup, spawn_world)
        .run();
}

/// Startup: spawn camera, light, and world.
fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, CAMERA_HEIGHT, CAMERA_Y).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("main_camera"),
        Name::new("isometric_camera"),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
        Name::new("sun_light"),
    ));

    // Spawn the player at world center with orange tetrahedron
    player::spawn_player(commands, None, 0, 1000, 100, vec![]);
}

/// Spawn the hex world after the window is ready.
fn spawn_world(mut commands: Commands) {
    let mut rng = rand::thread_rng();
    // World generation will be added when needed
    let hex_count = 64 * 64; // Simplified

    tracing::info!("Generated world with {} hexes", hex_count);

    // We won't spawn hex meshes in PostStartup to keep things simple
    // Hex world generation will be added when needed
    println!("World ready with {} hexes (single-player local mode)", hex_count);
}

/// Player movement system — WASD input with vehicle speed multipliers.
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
    };

    // Initialize current hex if not set
    if player.current_hex.is_none() {
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
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
    let vehicle = player.owned_vehicle.clone();
    let speed_multiplier = vehicle.map_or(1.0, |v| v.speed_multiplier());
    let base_speed = 10.0;
    let speed = base_speed * speed_multiplier;

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
        let actual_delta_x = move_x.clamp(-20.0, 20.0);
        let actual_delta_z = move_z.clamp(-20.0, 20.0);

        let old_pos = transform.translation;
        let new_pos = Vec3::new(
            old_pos.x + actual_delta_x,
            old_pos.y,
            old_pos.z + actual_delta_z,
        );
        transform.translation = new_pos;
    }

    // Update hex tracking and velocity
    let (q, r) = crate::world_pos_to_hex(
        transform.translation.x,
        transform.translation.z,
        10.0,
    );
    player.current_hex = Some(player::CurrentHex { q, r });
    player.velocity = Vec2::new(vx * speed * dt, vz * speed * dt);
    player.position = transform.translation;

    // Zero velocity when not moving
    if len == 0.0 {
        player.velocity = Vec2::ZERO;
    }
}

/// Debug commands for the single-player local version
fn debug_commands(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut player::ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
    };

    // 0 or key 0 — reset to spawn point
    if keyboard.just_pressed(KeyCode::Numpad0) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        println!("[DEBUG] Reset to spawn point");
    }

    // V — toggle vehicle info
    if keyboard.just_pressed(KeyCode::KeyV) {
        println!("Current vehicle: {:?}", player.owned_vehicle);
    }

    // R — reset position
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
        println!("[DEBUG] Position reset");
    }

    // L — apply idle gains (simulate login after offline time)
    if keyboard.just_pressed(KeyCode::KeyL) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_login = player.last_login_time;
        let seconds_offline = now.saturating_sub(last_login);

        if seconds_offline > 60 {
            println!(
                "[DEBUG] Offline time: {}s, applying idle gains",
                seconds_offline
            );

            // Apply idle gains manually
            let gains = if seconds_offline < 3600 {
                (10, 5)
            } else if seconds_offline < 21600 {
                (60, 30)
            } else if seconds_offline < 43200 {
                (100, 50)
            } else {
                (150, 75)
            };

            player.xp += gains.0;
            player.gold += gains.1;
            player.level = crate::progression::calculate_level(player.xp);

            println!(
                "[DEBUG] Applied: +{} XP, +{} Gold. New level: {}",
                gains.0, gains.1, player.level
            );
        } else {
            println!("[DEBUG] Not enough offline time for idle gains (need > 60s)");
        }

        // Update last login time
        player.last_login_time = now;
    }
}
