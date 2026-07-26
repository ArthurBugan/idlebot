//! IdleBot Client — Bevy Game Engine
//!
//! Single-player local version: 3D hex grid, player movement, idle gains
//! (no SpacetimeDB, no voice, no multiplayer — testing focus only)

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
use idlebot_core::Vehicle;

/// Sistema principal do jogo
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "IdleBot Single Player".to_string(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_startup_system(setup)
        .add_systems(Update, (player_movement, update_input));
    println!("IdleBot client initialized!");
}

/// Setup inicial da cena
fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 30.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("main_camera"),
    ));

    // Spawn light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
        Name::new("sun_light"),
    ));

    // Spawn world (hex grid) using spawn_world system
    commands.spawn((
        Name::new("world_spawn"),
        hex_renderer::SpawnWorld,
    ));

    // Spawn minimap camera
    commands.spawn((
        Camera2d::default(),
        Transform::from_xyz(0.0, 10.0, 1.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("minimap_camera"),
    ));

    // Spawn spawn marker at center
    commands.spawn((
        Name::new("spawn_point"),
        player::SpawnMarker,
        Transform::from_xyz(0.0, 0.5, 0.5),
        Visibility::default(),
    ));

    println!("IdleBot client setup complete — single-player local version");
}

/// Convert world position (x, z) to axial hex coordinates (q, r).
/// World X maps to hex Y plane, World Z maps to hex X plane.
/// Hex center: world_x = hex_radius * sqrt(3) * (q + r/2), world_z = hex_radius * 1.5 * r
/// Inverse: r = world_z / (1.5 * hex_radius), q = world_x / (sqrt(3) * hex_radius) - r / 2
pub fn world_pos_to_hex(world_x: f32, world_z: f32, hex_radius: f32) -> (i32, i32) {
    let sq3 = std::f32::consts::SQRT_3;
    let r_approx = (world_z / (1.5 * hex_radius)) as i32;
    let q_approx = ((world_x / (sq3 * hex_radius)) - (r_approx as f32) / 2.0) as i32;

    let fq = q_approx as f64;
    let fr = r_approx as f64;
    let fs = -(fq + fr);

    // Clamp to valid hex (round to nearest of q, r, or s direction)
    let dq = (fq - fr).abs();
    let ds = (fq - fs).abs();
    let dr = (fr - fs).abs();

    if dq > dr && dq > ds {
        // On q-r-s plane: q dominates, move to q-r direction
        let s_sgn = fs.signum();
        (s_sgn * (fs.saturating_sub(fr)) as i32, r_approx)
    } else if dr > ds {
        (q_approx, r_approx)
    } else {
        // s dominates, move to q-s direction
        let s_sign = fs.signum();
        (q_approx, -q_approx - fs)
    }
}

/// Sistema de movimento WASD — calcula direção, aplica velocidade e clamp ao hex
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.get_single_mut() else {
        return;
    };

    // Initialize hex if none yet
    if player.current_hex.is_none() {
        player.current_hex = Some(CurrentHex {
            q: 0,
            r: 0,
        });
    }

    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    // Normalize if any movement
    if direction.length() > 0.0 {
        direction = direction.normalize();
    }

    // Get base speed (10 m/s) and vehicle multiplier
    let base_speed = 10.0;
    let vehicle = player.owned_vehicle.as_ref();
    let speed_multiplier = vehicle.map_or(1.0, |v| v.speed_multiplier());
    let current_speed = base_speed * speed_multiplier;

    // Apply movement in world space
    let delta = direction * current_speed * time.delta_secs();
    let new_pos = transform.translation + Vec3::new(delta.x, 0.0, delta.y);

    // Update current hex based on new world position
    let hex_radius = 10.0f32;
    let new_hex = world_pos_to_hex(new_pos.x, new_pos.z, hex_radius);
    player.current_hex = Some(CurrentHex { q: new_hex.0, r: new_hex.1 });

    // Calculate hex center in world space
    let (hex_q, hex_r) = (*player.current_hex.unwrap());
    let sq3 = std::f32::consts::SQRT_3;
    let center_x = hex_radius * sq3 * (hex_q as f32 + hex_r as f32 / 2.0);
    let center_z = hex_radius * 1.5 * hex_r as f32;

    // Move toward hex center, clamped to hex boundary
    let target_x = center_x + delta.x;
    let target_z = center_z + delta.y;

    // Hex boundary: half the distance between adjacent hex centers
    let hex_half = hex_radius * 0.5;

    // Project to nearest valid hex center and clamp
    let final_x = project_to_hex_center(target_x, target_z, hex_radius, center_x);
    let final_z = project_to_hex_center(target_z, target_x, hex_radius, center_z);

    transform.translation.x = final_x;
    transform.translation.z = final_z;
    transform.translation.y = 0.0;

    // Update player position
    player.position = transform.translation;

    // Update velocity tracking
    player.velocity = Vec2::new(delta.x, delta.y);
}

/// Sistema que aplica WASD input ao movimento com veículo
fn update_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut ClientPlayer, &mut Transform)>,
) {
    let Ok((mut player, mut transform)) = player_query.get_single_mut() else {
        return;
    };

    let hex_radius = 10.0f32;
    let sq3 = std::f32::consts::SQRT_3;

    // Build movement from WASD
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

    // Vehicle speed multiplier
    let speed_multiplier = player.owned_vehicle
        .as_ref()
        .map_or(1.0, |v| v.speed_multiplier());
    let speed = 10.0 * speed_multiplier;

    // Normalize direction
    let len = (vx * vx + vz * vz).sqrt();
    if len > 0.0 {
        vx /= len;
        vz /= len;
    }

    // Move the player
    let delta = speed * time.delta_secs();
    let old_pos = transform.translation;
    let new_pos = Vec3::new(
        old_pos.x + vx * delta,
        old_pos.y,
        old_pos.z + vz * delta,
    );

    transform.translation = new_pos;

    // Update hex tracking
    player.current_hex = Some(CurrentHex {
        q: world_pos_to_hex(new_pos.x, new_pos.z, hex_radius).0,
        r: world_pos_to_hex(new_pos.x, new_pos.z, hex_radius).1,
    });

    // Handle key presses for instant movement
    if keyboard.just_pressed(KeyCode::KeyW) {
        // Move forward (toward +Z in world)
        let move_dist = 10.0 * speed_multiplier;
        transform.translation.z += move_dist;
    } else if keyboard.just_pressed(KeyCode::KeyS) {
        transform.translation.z -= move_dist;
    } else if keyboard.just_pressed(KeyCode::KeyA) {
        transform.translation.x -= move_dist;
    } else if keyboard.just_pressed(KeyCode::KeyD) {
        transform.translation.x += move_dist;
    }

    player.current_hex = Some(CurrentHex {
        q: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).0,
        r: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).1,
    });

    // Reset to spawn point
    if keyboard.just_pressed(KeyCode::NumPad0) || keyboard.just_pressed(KeyCode::Key0) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }

    // Debug: toggle vehicle
    if keyboard.just_pressed(KeyCode::KeyV) {
        println!("Vehicle: {:?}", player.owned_vehicle);
    }

    // Reset position
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }
}

/// Project a point to the nearest hex center, clamping within hex boundaries.
/// This keeps the player inside or at the edge of their current hex.
fn project_to_hex_center(x: f32, z: f32, hex_radius: f32, center_x: f32) -> f32 {
    // Calculate offset from center
    let dx = x - center_x;
    let dz = z; // z-axis for the hex plane

    // Determine which hex edge we're on
    let q = 0;
    let r = 0;
    let q_pos = center_x;
    let r_pos = center_z;

    // Half-distance between hex centers in x and z
    let sq3 = std::f32::consts::SQRT_3;
    let ddx = hex_radius * sq3 * 0.5; // half-distance x between adjacent hexes
    let ddz = hex_radius * 1.5 * 0.5; // half-distance z between adjacent hexes

    // Clamp to stay within the current hex
    let clamped_x = clamp(dx, -ddx, ddx) + center_x;
    let clamped_z = clamp(dz, -ddz, ddz) + center_z;

    clamped_x
}

/// Clamp a value between min and max
fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min { min } else if val > max { max } else { val }
}
