//! IdleBot Client — Bevy Game Engine
//!
//! Sistema principal do jogo: renderização, input, multiplayer, voice chat

#[path = "world/map_generator.rs"]
pub mod map_generator;

#[path = "world/hex_renderer.rs"]
pub mod hex_renderer;

#[path = "player/player_system.rs"]
pub mod player_system;

#[path = "voice/voice_system.rs"]
pub mod voice_system;

#[path = "assets/procedural.rs"]
pub mod procedural;

use bevy::prelude::*;

/// Componente do Player (cliente)
#[derive(Component)]
pub struct ClientPlayer {
    pub address: String,
    pub position: Vec3,
    pub vehicle: String,
    pub xp: u64,
    pub gold: u64,
    pub level: u32,
}

/// Sistema principal do jogo
pub fn main() {
    App::new()
        .add_systems(Startup, setup)
        .add_systems(Update, player_movement)
        .run();
}

/// Setup inicial da cena
fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("main_camera"),
    ));

    // Spawn light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
        Name::new("sun_light"),
    ));

    tracing::info!("IdleBot client initialized!");
}

/// Sistema de movimento do player (WASD)
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut ClientPlayer)>,
) {
    let Ok((mut transform, mut player)) = player_query.get_single_mut() else {
        return;
    };

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

    if direction.length() > 0.0 {
        direction = direction.normalize();
    }

    let speed = 10.0;
    let delta = direction * speed * time.delta_secs();

    transform.translation.x += delta.x;
    transform.translation.z += delta.y;

    player.position = transform.translation;
}
