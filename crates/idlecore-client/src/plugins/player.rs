//! Player system plugin — 2D world.
//!
//! WASD moves the player sprite directly (no physics engine: the transform is
//! the source of truth and the server clamps reported speeds). Also drives
//! the vehicle indicator (colored diamond + label) and the eco aura glow.

use bevy::prelude::*;
use idlecore_core::grid::world_pos_to_tile;
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
                dash_ghosts,
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
/// stops the desired velocity is zero and the player glides briefly.
const ACCELERATION: f32 = 12.0;

/// Below this speed (u/s) with no input, snap to zero instead of gliding —
/// long exponential tails read as laggy input, especially on web where frame
/// pacing is uneven.
const STOP_EPSILON: f32 = 0.6;

/// Max simulated delta per frame: web tabs can stall for hundreds of ms
/// between frames; without a clamp one catch-up step would teleport the
/// player through walkability checks and feel like a lag spike.
const MAX_FRAME_DT: f32 = 1.0 / 20.0;

/// Marker for the player's sprite entity. Kept as the "player body" marker so
/// shared systems (net sync, VFX) query one concept regardless of rendering.
#[derive(Component)]
pub struct PhysicsBody;

/// Draw-order offset for the player: above all tiles, plants and remote
/// players. Tiles are z = 1000 - y (see `world_floor`), so the player rides
/// 50 units above its own tile.
const PLAYER_DEPTH_OFFSET: f32 = 50.0;
const TILE_DEPTH_BASE: f32 = 130_000.0; // prop band base (world_floor::prop_depth)

fn register_player_orientation(mut commands: Commands) {
    commands.insert_resource(PlayerOrientation::default());
}

/// One controller integration step: ease `velocity` toward the commanded
/// `dir * speed`, snapping to a stop when input is released and the glide
/// falls under `STOP_EPSILON`. Pure so the feel is unit-testable.
fn step_velocity(velocity: Vec2, dir: Vec2, speed: f32, dt: f32) -> Vec2 {
    let blend = 1.0 - (-ACCELERATION * dt.min(MAX_FRAME_DT)).exp();
    let v = velocity.lerp(dir * speed, blend);
    if dir == Vec2::ZERO && v.length() < STOP_EPSILON {
        Vec2::ZERO
    } else {
        v
    }
}

/// Move the player sprite from WASD input; y = north.
#[allow(clippy::too_many_arguments)]
fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut skins: Option<ResMut<crate::skins::PlayerSkins>>,
    mut player_query: Query<(&mut Transform, &mut ClientPlayer), With<PhysicsBody>>,
    player_transform: ResMut<PlayerTransform>,
    orientation: ResMut<PlayerOrientation>,
    streaming_world: Option<Res<StreamingWorldResource>>,
    net: Option<ResMut<crate::net::plugin::Net>>,
    name_edit: Option<Res<crate::net::hud::NameEdit>>,
    touch: Res<crate::touch::TouchControls>,
) {
    // UI owns the keyboard while the login page is up (pre-login) or a text
    // field is being edited — WASD must not move the player then.
    let ui_focus = net.as_ref().map(|n| n.address.is_none()).unwrap_or(false)
        || name_edit.as_ref().map(|e| e.editing).unwrap_or(false);
    if ui_focus {
        return;
    }
    let Ok((mut transform, mut player)) = player_query.single_mut() else { return };

    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.y += 1.0; } // north
    if keyboard.pressed(KeyCode::KeyS) { input.y -= 1.0; } // south
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; } // west
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; } // east
    // Virtual joystick adds analog movement (magnitude <= 1).
    input += touch.move_vec;

    let dir = input.normalize_or_zero();
    let mut speed = BASE_SPEED;
    if let Some(v) = &player.owned_vehicle {
        speed *= v.speed_multiplier();
    }
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) || touch.sprint {
        speed *= SPRINT_MULTIPLIER;
    }
    // Spec 027 §5.2: dash burst — Space fires the reducer, the local window
    // (0.4 s vs the server's 1-tick rounding) drives the ×3 burst speed now.
    {
        let now = crate::time_ext::now_unix_secs() as f64;
        if keyboard.just_pressed(KeyCode::Space) {
            let dir = idlecore_core::player_combat::heading_index(orientation.facing_angle);
            if let Some(mut net) = net {
                if net.address.is_some() && player.death_until_utc <= now as u64 {
                    let tx = net.sender();
                    use crate::net::gen::player_dash;
                    crate::net::hud::send_reducer(&mut net, |r| {
                        r.player_dash_then(
                            dir,
                            crate::net::hud::reducer_report("dash", tx.clone(), 0),
                        )
                    });
                    // Immediate burst: the server row confirms a beat later.
                    player.dash_burst_secs = now
                        + idlecore_core::player_combat::DASH_DURATION_SECS as f64
                        + 0.1;
                }
            }
        }
        if player.death_until_utc > now as u64 {
            speed = 0.0; // dead: stand still until respawn
        } else if player.dashing(now) {
            speed *= idlecore_core::player_combat::DASH_SPEED_MULT as f32;
        }
    }
    // Partial joystick tilt scales speed for precise analog movement.
    speed *= input.length().min(1.0);

    // Spec 034 C3 (HLD lunge): a fresh attack start pushes the body toward
    // the strike heading. The exp-ease recovers the impulse inside the clip.
    if let Some(mut skins) = skins.as_mut() {
        if crate::skins::consume_lunge(&mut skins) {
            let rad = orientation.facing_angle;
            player.velocity.x += rad.cos() * crate::skins::LUNGE_IMPULSE;
            player.velocity.y += rad.sin() * crate::skins::LUNGE_IMPULSE;
        }
    }

    // Exp-lerp the actual velocity toward the commanded one; the player
    // accelerates on key press and decelerates to a stop on release. dt is
    // clamped so a stalled web frame can't teleport the player.
    let dt = time.delta_secs().min(MAX_FRAME_DT);
    player.velocity = step_velocity(player.velocity, dir, speed, time.delta_secs());
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
    let (hq, hr) = world_pos_to_tile(x, y, WorldGenConfig::TILE_SIZE);
    let (cq, cr) = hex_to_chunk_coord(hq, hr, WorldGenConfig::CHUNK_SIZE);
    let Some(chunk) = world.chunks.chunks.get(&(cq, cr)) else { return true };
    for cell in &chunk.cells {
        if cell.x == hq && cell.y == hr {
            return cell.terrain.is_walkable();
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Dash trail (Spec 034 C7 — HLD afterimages)
// ---------------------------------------------------------------------------

/// A fading afterimage stamped behind the dashing player.
#[derive(Component)]
pub struct DashGhost {
    pub t: f32,
}

/// Spawn cadence while the burst lives (seconds between afterimages).
const GHOST_EVERY: f32 = 0.06;

/// Drive the dash trail: stamp a translucent diamond every 60 ms while the
/// burst is live, fade it over 0.25 s. The afterimage reads by silhouette —
/// the skin pose a frame stamped is not knowable here without duplicating
/// the skins state machine.
#[allow(clippy::type_complexity)]
fn dash_ghosts(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
    mut next: Local<f32>,
    player_query: Query<(&Transform, &ClientPlayer), With<PhysicsBody>>,
    mut ghosts: Query<(Entity, &mut DashGhost, &mut Sprite)>,
) {
    let unix_now = crate::time_ext::now_unix_secs() as f64;
    let Ok((t, player)) = player_query.single() else { return };

    // Fading pass first (runs even between spawns).
    for (entity, mut ghost, mut sprite) in ghosts.iter_mut() {
        ghost.t += time.delta_secs();
        if ghost.t >= 0.25 {
            commands.entity(entity).despawn();
            continue;
        }
        // The stamp stores its own alpha; re-read the sRGB tint and drop it.
        let base = sprite.color.to_srgba();
        sprite.color = bevy::color::Color::Srgba(
            base.with_alpha(0.35 * (1.0 - ghost.t / 0.25)),
        );
    }

    // Stamp pass.
    *next -= time.delta_secs();
    if player.dashing(unix_now) && *next <= 0.0 {
        *next = GHOST_EVERY;
        let img = images.add(bevy::image::Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &[255, 255, 255, 255],
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        ));
        commands.spawn((
            DashGhost { t: 0.0 },
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, 0.35),
                custom_size: Some(Vec2::splat(1.4)),
                image: img,
                ..default()
            },
            Transform::from_xyz(t.translation.x, t.translation.y - 0.9, t.translation.z - 2.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
        ));
    }
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

/// Vehicle art from the Tiny* packs: real sprites where the packs have them
/// (Tiny Battle boats/planes/cars), tinted diamonds as a fallback.
#[derive(Resource, Default)]
pub struct VehicleSprites {
    pub white: Option<Handle<Image>>,
    pub boat: Option<Handle<Image>>,
    pub airplane: Option<Handle<Image>>,
    pub car: Option<Handle<Image>>,
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

fn spawn_vehicle_indicator(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut indicator: ResMut<VehicleIndicator>,
) {
    let white = images.add(bevy::image::Image::new_fill(
        bevy::render::render_resource::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        &[255, 255, 255, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    ));
    // Vehicle indicators render as tinted diamonds over the player; the old
    // Tiny Battle art is gone, so no dedicated sprites are loaded.
    commands.insert_resource(VehicleSprites {
        white: Some(white),
        boat: None,
        airplane: None,
        car: None,
    });
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
    sprites: Res<VehicleSprites>,
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
                    // Real art where the packs have it (boats, planes, cars);
                    // tinted diamond otherwise.
                    let art = match v {
                        idlecore_core::Vehicle::Bicycle => sprites.car.clone(),
                        idlecore_core::Vehicle::Boat => sprites.boat.clone(),
                        idlecore_core::Vehicle::Airplane => sprites.airplane.clone(),
                        _ => None,
                    }
                    .or_else(|| sprites.white.clone());
                    match art {
                        Some(art) => {
                            sprite.image = art;
                            let is_art = matches!(
                                v,
                                idlecore_core::Vehicle::Bicycle
                                    | idlecore_core::Vehicle::Boat
                                    | idlecore_core::Vehicle::Airplane
                            );
                            sprite.color = if is_art { Color::WHITE } else { vehicle_color(v) };
                            sprite.custom_size = Some(Vec2::splat(if is_art { 3.6 } else { 3.6 }));
                            t.rotation = if is_art {
                                Quat::IDENTITY
                            } else {
                                Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
                            };
                        }
                        None => {}
                    }
                }
                None => {
                    t.translation.y = -1000.0;
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
    // The vehicle name now lives in the inventory/hotbar UI; the big world-
    // space label is intentionally not shown (Spec feedback).
    if let Some(entity) = indicator.label {
        if let Ok((mut t, mut vis, _text)) = entities.p1().get_mut(entity) {
            t.translation.y = -1000.0;
            *vis = Visibility::Hidden;
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_accelerates_toward_commanded_speed() {
        let mut v = Vec2::ZERO;
        for _ in 0..60 {
            v = step_velocity(v, Vec2::X, BASE_SPEED, 1.0 / 60.0);
        }
        assert!((v.x - BASE_SPEED).abs() < 0.5, "did not reach walk speed: {v}");
    }

    #[test]
    fn velocity_snaps_to_zero_after_release() {
        let mut v = Vec2::new(BASE_SPEED, 0.0);
        for _ in 0..90 {
            v = step_velocity(v, Vec2::ZERO, BASE_SPEED, 1.0 / 60.0);
        }
        assert_eq!(v, Vec2::ZERO, "glide tail should end in a hard stop");
    }

    #[test]
    fn stalled_frame_dt_is_clamped() {
        // A 500 ms web stall must not exceed the clamped step: the velocity
        // after one huge frame equals the velocity after a 50 ms frame.
        let huge = step_velocity(Vec2::ZERO, Vec2::X, BASE_SPEED, 0.5);
        let clamped = step_velocity(Vec2::ZERO, Vec2::X, BASE_SPEED, MAX_FRAME_DT);
        assert_eq!(huge, clamped);
    }
}
