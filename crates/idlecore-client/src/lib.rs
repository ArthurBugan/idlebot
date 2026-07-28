//! IdleBot Client Library
//!
//! Single-player local version: 3D hex grid, player movement, idle gains

#[path = "world/hex_renderer.rs"]
pub mod hex_renderer;

#[path = "world/map_generator.rs"]
pub mod map_generator;

#[path = "voice/voice_system.rs"]
pub mod voice_system;

#[path = "assets/procedural.rs"]
pub mod procedural;

#[path = "player.rs"]
pub mod player;

#[path = "vehicle.rs"]
pub mod vehicle;

#[path = "idle.rs"]
pub mod idle;

#[path = "input.rs"]
pub mod input;

#[path = "progression.rs"]
pub mod progression;

use bevy::prelude::*;
use crate::player::ClientPlayer;

/// Convert world position (x, z) to axial hex coordinates (q, r).
pub fn world_pos_to_hex(world_x: f32, world_z: f32, hex_radius: f32) -> (i32, i32) {
    let sq3 = 1.732050808f32;
    let r_approx = (world_z / (1.5 * hex_radius)) as i32;
    let q_approx = ((world_x / (sq3 * hex_radius)) - (r_approx as f32) / 2.0) as i32;

    let fq = q_approx as f64;
    let fr = r_approx as f64;
    let fs = -(fq + fr);

    let dq = (fq - fr).abs();
    let dr = (fq - fs).abs();
    let ds = (fr - fs).abs();

    if dq > dr && dq > ds {
        let sgn = fs.signum() as i32;
        (sgn * ((fs - fr) as i32 + fr as i32), r_approx)
    } else if dr > ds {
        (q_approx, r_approx)
    } else {
        let sgn = fs.signum() as i32;
        (q_approx, sgn * (fs - fr) as i32 + fr as i32)
    }
}

/// Sistema de movimento WASD
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut ClientPlayer)>,
) {
    let mut iter = player_query.iter_mut();
    let Some((mut transform, mut player)) = iter.next() else {
        return;
    };
    drop(iter);

    if player.current_hex.is_none() {
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
    }

    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) { direction.y += 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { direction.y -= 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { direction.x += 1.0; }

    if direction.length() > 0.0 {
        direction = direction.normalize();
    }

    let base_speed = 10.0;
    let speed_multiplier = player.owned_vehicle.as_ref().map_or(1.0, |v| v.speed_multiplier());
    let current_speed = base_speed * speed_multiplier;

    let delta = direction * current_speed * time.delta_secs();
    let new_pos = transform.translation + Vec3::new(delta.x, 0.0, delta.y);

    let hex_radius = 10.0f32;
    let new_hex = world_pos_to_hex(new_pos.x, new_pos.z, hex_radius);
    player.current_hex = Some(player::CurrentHex { q: new_hex.0, r: new_hex.1 });

    transform.translation = new_pos;
    player.position = transform.translation;
    player.velocity = Vec2::new(delta.x, delta.y);
}

/// Sistema que aplica WASD input ao movimento
#[allow(dead_code)]
fn update_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut ClientPlayer, &mut Transform)>,
) {
    let mut iter = player_query.iter_mut();
    let Some((mut player, mut transform)) = iter.next() else {
        return;
    };
    drop(iter);

    let hex_radius = 10.0f32;
    let mut vx = 0.0f32;
    let mut vz = 0.0f32;

    if keyboard.pressed(KeyCode::KeyW) { vz -= 1.0; } else if keyboard.pressed(KeyCode::KeyS) { vz += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { vx -= 1.0; } else if keyboard.pressed(KeyCode::KeyD) { vx += 1.0; }

    let speed_multiplier = player.owned_vehicle.as_ref().map_or(1.0, |v| v.speed_multiplier());
    let speed = 10.0 * speed_multiplier;
    let len = (vx * vx + vz * vz).sqrt();
    if len > 0.0 { vx /= len; vz /= len; }

    let delta = speed * time.delta_secs();
    transform.translation = Vec3::new(
        transform.translation.x + vx * delta,
        transform.translation.y,
        transform.translation.z + vz * delta,
    );

    player.current_hex = Some(player::CurrentHex {
        q: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).0,
        r: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).1,
    });

    if keyboard.just_pressed(KeyCode::Numpad0) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(player::CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }
}

/// Project a point to the nearest hex center, clamping within hex boundaries.
#[allow(dead_code)]
fn project_to_hex_center(x: f32, z: f32, hex_radius: f32, center_x: f32) -> f32 {
    let dx = x - center_x;
    let dz = z;
    let sq3 = 1.732050808f32;
    let ddx = hex_radius * sq3 * 0.5;
    let ddz = hex_radius * 1.5 * 0.5;
    let clamped_x = clamp(dx, -ddx, ddx) + center_x;
    let _clamped_z = clamp(dz, -ddz, ddz);
    clamped_x
}

/// Clamp a value between min and max
#[allow(dead_code)]
fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min { min } else if val > max { max } else { val }
}
