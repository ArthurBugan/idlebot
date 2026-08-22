//! Player system plugin — 2D world.
//!
//! WASD moves the player sprite directly (no physics engine: the transform is
//! the source of truth and the server clamps reported speeds). Also drives
//! the vehicle indicator (colored diamond + label) and the eco aura glow.

use bevy::prelude::*;
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::world_gen::{hex_to_chunk_coord, WorldGenConfig};
use crate::player::{ClientPlayer, PlayerOrientation, PlayerTransform};
use crate::plugins::world::StreamingWorldResource;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIndicator>()
            .init_resource::<AuraLight>()
            .add_systems(Startup, (register_player_orientation, spawn_vehicle_indicator))
            .add_systems(Update, (
                player_movement,
                update_vehicle_indicator,
                update_aura_light,
            ));
    }
}

/// Base walk speed (world units/s) — matches the server's BASE_SPEED so
/// persisted positions stay close to local movement.
pub const BASE_SPEED: f32 = 10.0;

/// Sprint multiplier (local only; the server caps reported speed).
pub const SPRINT_MULTIPLIER: f32 = 2.0;

/// Acceleration/deceleration rate (1/s): the actual velocity converges on the
/// commanded speed with a 1/k time constant. Smooth start/stop; when input
/// stops the desired velocity is zero and the player glides to a halt.
const ACCELERATION: f32 = 8.0;

/// Marker for the player's sprite entity. Kept as the "player body" marker so
/// shared systems (net sync, VFX) query one concept regardless of rendering.
#[derive(Component)]
pub struct PhysicsBody;

/// Draw-order offset for the player: above all tiles, plants and remote
/// players. Tiles are z = 1000 - y (see `world_floor`), so the player rides
/// 50 units above its own tile.
const PLAYER_DEPTH_OFFSET: f32 = 50.0;
const TILE_DEPTH_BASE: f32 = 1000.0;

fn register_player_orientation(mut commands: Commands) {
    commands.insert_resource(PlayerOrientation::default());
}

/// Move the player sprite from WASD input; y = north.
fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_query: Query<(&mut Transform, &mut ClientPlayer), With<PhysicsBody>>,
    mut player_transform: ResMut<PlayerTransform>,
    mut orientation: ResMut<PlayerOrientation>,
    streaming_world: Option<Res<StreamingWorldResource>>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else { return };

    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.y += 1.0; } // north
    if keyboard.pressed(KeyCode::KeyS) { input.y -= 1.0; } // south
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; } // west
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; } // east

    let dir = input.normalize_or_zero();
    let mut speed = BASE_SPEED;
    if let Some(v) = &player.owned_vehicle {
        speed *= v.speed_multiplier();
    }
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        speed *= SPRINT_MULTIPLIER;
    }

    // Exp-lerp the actual velocity toward the commanded one; the player
    // accelerates on key press and decelerates to a stop on release.
    let dt = time.delta_secs();
    let blend = 1.0 - (-ACCELERATION * dt).exp();
    player.velocity = player.velocity.lerp(dir * speed, blend);
    let next_x = transform.translation.x + player.velocity.x * dt;
    let next_y = transform.translation.y + player.velocity.y * dt;

    // Client-side walkability clamp: never step into a non-walkable hex
    // (water). Slide along the free axis instead. Standing inside blocked
    // terrain stays allowed so a bad spawn can always walk out, and
    // unloaded hexes count as walkable so streaming gaps never trap us.
    let Some(world) = streaming_world.as_ref() else {
        transform.translation.x = next_x;
        transform.translation.y = next_y;
        return finish_movement(transform, player, player_transform, orientation, dir);
    };
    if walkable_at(world, next_x, next_y)
        || !walkable_at(world, transform.translation.x, transform.translation.y)
    {
        transform.translation.x = next_x;
        transform.translation.y = next_y;
    } else {
        if walkable_at(world, next_x, transform.translation.y) {
            transform.translation.x = next_x;
        }
        if walkable_at(world, transform.translation.x, next_y) {
            transform.translation.y = next_y;
        }
    }

    finish_movement(transform, player, player_transform, orientation, dir)
}

/// Shared tail of `player_movement`: facing, depth ordering, mirror sync.
fn finish_movement(
    mut transform: Mut<Transform>,
    mut player: Mut<ClientPlayer>,
    mut player_transform: ResMut<PlayerTransform>,
    mut orientation: ResMut<PlayerOrientation>,
    dir: Vec2,
) {
    if dir != Vec2::ZERO {
        // Facing tracks the commanded direction (instant), not the lagging
        // velocity, so the character turns on key press.
        orientation.facing_angle = dir.y.atan2(dir.x);
    }
    // Isometric draw order: south (smaller y) rows draw over north rows.
    transform.translation.z = TILE_DEPTH_BASE - transform.translation.y + PLAYER_DEPTH_OFFSET;

    player.position = transform.translation;
    player_transform.translation = transform.translation;
}

/// True if the hex at world `(x, y)` is known and walkable.
fn walkable_at(world: &StreamingWorldResource, x: f32, y: f32) -> bool {
    let (hq, hr) = world_pos_to_hex(x, y, WorldGenConfig::HEX_SIZE);
    let (cq, cr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);
    let Some(chunk) = world.chunks.chunks.get(&(cq, cr)) else { return true };
    for cell in &chunk.cells {
        if cell.q == hq && cell.r == hr {
            return cell.terrain.is_walkable();
        }
    }
    true
}

// ============================================================================
// Vehicle Indicator (Spec 006 T5.1/T5.2)
// ============================================================================

/// Colored ground diamond + floating label rendered while a vehicle is
/// equipped.
#[derive(Resource, Default)]
pub struct VehicleIndicator {
    pub plate: Option<Entity>,
    pub label: Option<Entity>,
}

fn vehicle_color(vehicle: &idlecore_core::Vehicle) -> Color {
    match vehicle {
        idlecore_core::Vehicle::None => Color::srgba(0.2, 0.2, 0.2, 0.0),
        idlecore_core::Vehicle::Bicycle => Color::srgb(0.2, 0.9, 1.0),
        idlecore_core::Vehicle::Scooter => Color::srgb(0.6, 1.0, 0.3),
        idlecore_core::Vehicle::Motorcycle => Color::srgb(1.0, 0.45, 0.2),
        idlecore_core::Vehicle::Boat => Color::srgb(0.3, 0.6, 1.0),
        idlecore_core::Vehicle::Airplane => Color::srgb(0.8, 0.5, 1.0),
    }
}

fn spawn_vehicle_indicator(mut commands: Commands, mut indicator: ResMut<VehicleIndicator>) {
    let plate = commands
        .spawn((
            Name::new("vehicle-plate"),
            Sprite {
                color: Color::srgb(0.3, 0.3, 0.3),
                custom_size: Some(Vec2::splat(3.6)),
                ..default()
            },
            Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            Visibility::Hidden,
        ))
        .id();
    let label = commands
        .spawn((
            Name::new("vehicle-label"),
            Text2d::new(""),
            TextFont { font_size: 14.0.into(), ..default() },
            TextColor(Color::BLACK),
            TextShadow { color: Color::WHITE, offset: Vec2::new(0.5, 0.5) },
            Visibility::Hidden,
        ))
        .id();
    indicator.plate = Some(plate);
    indicator.label = Some(label);
}

fn update_vehicle_indicator(
    body: Query<(&Transform, &ClientPlayer), With<PhysicsBody>>,
    indicator: Option<Res<VehicleIndicator>>,
    mut entities: ParamSet<(
        Query<(&mut Transform, &mut Visibility, &mut Sprite), (Without<Text2d>, Without<PhysicsBody>)>,
        Query<(&mut Transform, &mut Visibility, &mut Text2d), (Without<Sprite>, Without<PhysicsBody>)>,
    )>,
) {
    let Some(indicator) = indicator else { return };
    let Ok((body_t, player)) = body.single() else { return };

    let vehicle = player
        .owned_vehicle
        .as_ref()
        .filter(|v| **v != idlecore_core::Vehicle::None);

    if let Some(entity) = indicator.plate {
        if let Ok((mut t, mut vis, mut sprite)) = entities.p0().get_mut(entity) {
            match vehicle {
                Some(v) => {
                    t.translation = Vec3::new(body_t.translation.x, body_t.translation.y, TILE_DEPTH_BASE - body_t.translation.y + PLAYER_DEPTH_OFFSET - 1.0);
                    *vis = Visibility::Visible;
                    sprite.color = vehicle_color(v);
                }
                None => {
                    t.translation.y = -1000.0;
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
    if let Some(entity) = indicator.label {
        if let Ok((mut t, mut vis, mut text)) = entities.p1().get_mut(entity) {
            match vehicle {
                Some(v) => {
                    t.translation = Vec3::new(
                        body_t.translation.x,
                        body_t.translation.y + 6.0,
                        TILE_DEPTH_BASE - body_t.translation.y + PLAYER_DEPTH_OFFSET - 0.5,
                    );
                    *vis = Visibility::Visible;
                    text.0 = v.display_name().to_string();
                }
                None => {
                    t.translation.y = -1000.0;
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

// ============================================================================
// Aura Glow (Spec 016 T5.4) — 2D glow diamond gated by eco rank
// ============================================================================

/// The spawned aura entity (child of the player).
#[derive(Resource, Default)]
pub struct AuraLight {
    pub entity: Option<Entity>,
}

/// Pure eco-rank → aura mapping: (color, alpha), None below Enthusiast.
pub fn aura_config(ep: u64) -> Option<(Color, f32)> {
    if ep >= 1000 {
        Some((Color::srgb(1.0, 0.85, 0.3), 0.35))
    } else if ep >= 500 {
        Some((Color::srgb(0.25, 1.0, 0.45), 0.3))
    } else if ep >= 100 {
        Some((Color::srgb(0.3, 0.9, 1.0), 0.25))
    } else {
        None
    }
}

/// Spawn (once) and drive the aura diamond from the eco rank.
fn update_aura_light(
    mut commands: Commands,
    mut light: ResMut<AuraLight>,
    player_query: Query<(Entity, &ClientPlayer), With<PhysicsBody>>,
    mut existing: Query<(&mut Sprite, &mut Visibility)>,
) {
    let Ok((body, player)) = player_query.single() else { return };
    if light.entity.is_none() {
        light.entity = Some(
            commands
                .spawn((
                    Name::new("eco-aura"),
                    Sprite {
                        custom_size: Some(Vec2::splat(8.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -1.0)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                    Visibility::Hidden,
                ))
                .insert(ChildOf(body))
                .id(),
        );
    }
    let Some(entity) = light.entity else { return };
    let Ok((mut sprite, mut vis)) = existing.get_mut(entity) else { return };
    match aura_config(player.eco_points) {
        Some((color, alpha)) => {
            sprite.color = color.with_alpha(alpha);
            *vis = Visibility::Visible;
        }
        None => {
            *vis = Visibility::Hidden;
        }
    }
}