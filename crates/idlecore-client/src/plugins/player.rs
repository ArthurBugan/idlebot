//! Player system plugin
//! Handles player movement and position tracking

use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::gltf::Gltf;
use bevy_rapier3d::prelude::*;
use crate::player::{Player, PlayerOrientation, PlayerTransform};
use crate::plugins::world::StreamingWorldResource;
use idlecore_core::hex::world_pos_to_hex;
use idlecore_core::world_gen::WorldGenConfig;

/// Feet sit +0.1365 world units above the player anchor (model offset × 13 scale).
const FEET_OFFSET: f32 = 0.0105 * 13.0;

/// Small height differences between neighboring hexes (elevation × 25) are
/// treated as climbable steps instead of walls.
const MAX_STEP_HEIGHT: f32 = 12.0;

/// How far ahead of the player the next hex's height is probed.
const STEP_PROBE_DIST: f32 = 10.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Init at build time so Startup systems that consume it don't race
        // other plugins' Startup command flushes (missing resource → panic).
        app.init_resource::<VehicleIndicator>()
            .init_resource::<AuraLight>()
            .add_systems(Update, (
                player_movement.after(PhysicsSet::Writeback),
                sync_visual_to_physics.after(player_movement),
                player_animation,
                update_vehicle_indicator,
                update_aura_light,
            ));
        app.add_systems(Startup, (
            register_player_orientation,
            register_player_animations,
            spawn_vehicle_indicator,
        ));
    }
}

fn register_player_orientation(mut commands: Commands) {
    commands.insert_resource(PlayerOrientation::default());
}

/// Named animations from the character GLB, played via an `AnimationGraph`.
#[derive(Resource)]
pub struct PlayerAnimationState {
    pub gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    pub clips: HashMap<String, AnimationNodeIndex>,
    pub player_entity: Option<Entity>,
    /// Currently-playing locomotion clip (idle/walk/run/crouch…).
    pub locomotion: Option<String>,
    /// A one-shot clip (jump/attack/death) currently playing; `None` when idle.
    pub one_shot: Option<AnimationNodeIndex>,
}

impl Default for PlayerAnimationState {
    fn default() -> Self {
        Self {
            gltf: Handle::default(),
            graph: None,
            clips: default(),
            player_entity: None,
            locomotion: None,
            one_shot: None,
        }
    }
}

fn register_player_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(PlayerAnimationState {
        gltf: asset_server.load::<Gltf>("models/characterLargeMale.glb"),
        ..default()
    });
}

/// Load named clips into an `AnimationGraph` asset, attach it to the player's
/// `AnimationPlayer` once the scene spawns, then drive locomotion and
/// one-shot animations (jump/attack/death) from input.
fn player_animation(
    mut commands: Commands,
    gltf_assets: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut state: ResMut<PlayerAnimationState>,
    player_root: Query<Entity, With<Player>>,
    children: Query<&Children>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // 1. Build the animation graph once the glTF asset is loaded.
    if state.graph.is_none() {
        let Some(gltf) = gltf_assets.get(&state.gltf) else { return };
        info!("GLB loaded: {} named animations", gltf.named_animations.len());
        for (name, _) in &gltf.named_animations {
            info!("  Animation: {}", name);
        }
        if gltf.named_animations.is_empty() {
            warn!("No animations found in GLB!");
            return;
        }
        let mut graph = AnimationGraph::new();
        for (name, clip) in &gltf.named_animations {
            let node = graph.add_clip(clip.clone(), 1.0, graph.root);
            state.clips.insert(name.to_string(), node);
        }
        state.graph = Some(graphs.add(graph));
    }

    // 2. Find the scene-spawned AnimationPlayer (a descendant of the player)
    //    and attach the graph + transitions to it once.
    if state.player_entity.is_none() {
        let Ok(root) = player_root.single() else { return };
        for child in children.iter_descendants(root) {
            let Ok((entity, _, _)) = animation_players.get(child) else { continue };
            commands
                .entity(entity)
                .insert(AnimationGraphHandle(state.graph.clone().unwrap()))
                .insert(AnimationTransitions::new());
            state.player_entity = Some(entity);
            info!("Attached animation graph to entity");
            return;
        }
        warn!("No AnimationPlayer found in player descendants");
        return;
    }

    let Ok((_, mut player, Some(mut transitions))) =
        animation_players.get_mut(state.player_entity.unwrap())
    else {
        return;
    };

    // 3a. A one-shot animation (jump/attack/death) is playing: wait for it to
    //     finish, then fall back to locomotion.
    if let Some(shot_node) = state.one_shot {
        let finished = player
            .animation(shot_node)
            .map(bevy::animation::ActiveAnimation::is_finished)
            .unwrap_or(true);
        if !finished {
            return;
        }
        state.one_shot = None;
        // Force the locomotion clip to restart once the shot is over.
        state.locomotion = None;
    }

    // 3b. Trigger a one-shot animation.
    let shot = if keyboard.just_pressed(KeyCode::Space) {
        Some("jump")
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        Some("attack")
    } else if keyboard.just_pressed(KeyCode::KeyK) {
        Some("death")
    } else {
        None
    };
    if let Some(name) = shot {
        let Some(node) = state.clips.get(name) else {
            warn!("Animation '{name}' not found!");
            return;
        };
        player.stop(*node);
        transitions.play(&mut player, *node, std::time::Duration::from_millis(120));
        state.one_shot = Some(*node);
        info!("Playing one-shot animation: {name}");
        return;
    }

    // 3c. Locomotion: crouch (Ctrl) / walk / run (Shift), idle otherwise.
    let moving = keyboard.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]);
    let crouching = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let sprinting =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let target = match (crouching, moving, sprinting) {
        (true, true, _) => "crouchWalk",
        (true, false, _) => "crouchIdle",
        (false, true, true) => "run",
        (false, true, false) => "walk",
        _ => "idle",
    };
    if state.locomotion.as_deref() == Some(target) {
        return;
    }
    let Some(node) = state.clips.get(target) else {
        warn!("Animation '{target}' not found! Available: {:?}", state.clips.keys().collect::<Vec<_>>());
        return;
    };
    info!("Playing animation: {target}");
    // `stop` removes the previous entry so the clip always restarts from the
    // beginning of its cycle (bevy 0.19 `replay()` keeps the old seek position,
    // which made re-triggered runs resume mid-stride).
    player.stop(*node);
    transitions
        .play(&mut player, *node, std::time::Duration::from_millis(150))
        .repeat();
    state.locomotion = Some(target.to_string());
}

/// Marker for the physics body entity that drives the visible player model.
#[derive(Component)]
pub struct PhysicsBody;

/// Move the player: WASD maps directly to horizontal rigid-body velocity.
/// Vertical motion is left to gravity against the terrain colliders, and the
/// physics writeback keeps the body `Transform` in sync.
fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    streaming_world: Res<StreamingWorldResource>,
    mut rapier_ctx: WriteRapierContext,
    mut player_query: Query<(
        &Transform,
        &RapierRigidBodyHandle,
        &mut Velocity,
        &mut crate::player::ClientPlayer,
    ), With<PhysicsBody>>,
    mut player_transform: ResMut<PlayerTransform>,
    mut orientation: ResMut<PlayerOrientation>,
) {
    let Ok((transform, handle, mut velocity, mut player)) = player_query.single_mut() else {
        return;
    };

    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.y -= 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { input.y += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; }

    let dir = input.normalize_or_zero();
    let mut speed = 150.0;
    if let Some(v) = &player.owned_vehicle {
        speed *= v.speed_multiplier();
    }
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        speed *= 100.0;
    }
    velocity.linear.x = dir.x * speed;
    velocity.linear.z = dir.y * speed;

    // Step up: when walking toward a slightly higher neighbor hex, lift the
    // body so the step is climbable instead of becoming a wall. Done through
    // rapier (`set_translation` on the physics body) — direct writes to the
    // body's bevy `Transform` would fight the physics writeback every frame.
    if dir != Vec2::ZERO {
        let x = transform.translation.x;
        let z = transform.translation.z;
        let ahead_x = x + dir.x * STEP_PROBE_DIST;
        let ahead_z = z + dir.y * STEP_PROBE_DIST;
        let here = terrain_height_at(&streaming_world, x, z);
        let ahead = terrain_height_at(&streaming_world, ahead_x, ahead_z);
        if ahead > here && ahead - here <= MAX_STEP_HEIGHT {
            if let Ok(mut ctx) = rapier_ctx.single_mut() {
                let body = &mut ctx.rigidbody_set.bodies[handle.0];
                body.set_translation(Vec3::new(x, ahead - FEET_OFFSET, z), true);
            }
        }
    }

    if dir != Vec2::ZERO {
        orientation.facing_angle = input.y.atan2(input.x);
        // The facing is applied to the visual root in `sync_visual_to_physics`;
        // writing the physics body's rotation directly would be overwritten by
        // the writeback and flicker between the two poses every frame.
    }

    player.position = transform.translation;
    player_transform.translation = transform.translation;
}

/// Terrain height (world units) directly under a world position, matching the
/// world_floor mesh (hex elevation × elevation_scale 25).
fn terrain_height_at(
    streaming_world: &StreamingWorldResource,
    x: f32,
    z: f32,
) -> f32 {
    let (q, r) = world_pos_to_hex(x, z, WorldGenConfig::HEX_SIZE);
    streaming_world.config.generate_hex(q, r).elevation * 25.0
}

/// The visual root carries the 13× model scale; the physics body is unscaled,
/// so copy the body's world pose onto the visual root every frame. Facing is
/// applied here (from `PlayerOrientation`) because the physics body's rotation
/// is rapier-owned and must not be written directly.
fn sync_visual_to_physics(
    bodies: Query<&Transform, With<PhysicsBody>>,
    orientation: Res<PlayerOrientation>,
    mut roots: Query<&mut Transform, (With<Player>, Without<PhysicsBody>)>,
) {
    let Ok(body) = bodies.single() else { return };
    let Ok(mut root) = roots.single_mut() else { return };
    root.translation = body.translation;
    root.rotation =
        Quat::from_rotation_y(std::f32::consts::PI / 2.0 - orientation.facing_angle);
}

// ============================================================================
// Vehicle Indicator (Spec 006 T5.1/T5.2)
// ============================================================================

/// Colored ground plate + floating label rendered while a vehicle is equipped.
#[derive(Resource, Default)]
pub struct VehicleIndicator {
    pub plate: Option<Entity>,
    pub label: Option<Entity>,
    pub plate_material: Option<Handle<StandardMaterial>>,
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut indicator: ResMut<VehicleIndicator>,
) {
    let plate_material = materials.add(StandardMaterial::from_color(Color::srgb(0.3, 0.3, 0.3)));
    let plate = commands
        .spawn((
            Name::new("vehicle-plate"),
            Mesh3d(meshes.add(Cuboid::new(3.0, 0.06, 3.0))),
            MeshMaterial3d(plate_material.clone()),
            Transform::from_xyz(0.0, -100.0, 0.0),
            Visibility::Hidden,
        ))
        .id();
    let label = commands
        .spawn((
            Name::new("vehicle-label"),
            Text2d::new(""),
            TextFont { font_size: 18.0.into(), ..default() },
            TextColor(Color::BLACK),
            TextShadow { color: Color::WHITE, offset: Vec2::new(0.5, 0.5) },
            Transform::from_xyz(0.0, -100.0, 0.0),
            Visibility::Hidden,
        ))
        .id();
    indicator.plate = Some(plate);
    indicator.label = Some(label);
    indicator.plate_material = Some(plate_material);
}

fn update_vehicle_indicator(
    body: Query<(&Transform, &crate::player::ClientPlayer), With<PhysicsBody>>,
    indicator: Option<Res<VehicleIndicator>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut entities: ParamSet<(
        Query<(&mut Transform, &mut Visibility), (Without<Text2d>, Without<PhysicsBody>)>,
        Query<(&mut Transform, &mut Visibility, &mut Text2d), (Without<Mesh3d>, Without<PhysicsBody>)>,
    )>,
) {
    let Some(indicator) = indicator else { return };
    let Ok((body_t, player)) = body.single() else { return };

    let vehicle = player
        .owned_vehicle
        .as_ref()
        .filter(|v| **v != idlecore_core::Vehicle::None);

    if let Some(entity) = indicator.plate {
        if let Ok((mut t, mut vis)) = entities.p0().get_mut(entity) {
            match vehicle {
                Some(v) => {
                    *t = Transform::from_xyz(body_t.translation.x, 0.15, body_t.translation.z);
                    *vis = Visibility::Visible;
                    if let Some(h) = &indicator.plate_material {
                        if let Some(mut m) = materials.get_mut(h) {
                            m.base_color = vehicle_color(v);
                        }
                    }
                }
                None => {
                    *t = Transform::from_xyz(0.0, -100.0, 0.0);
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
    if let Some(entity) = indicator.label {
        if let Ok((mut t, mut vis, mut text)) = entities.p1().get_mut(entity) {
            match vehicle {
                Some(v) => {
                    *t = Transform::from_xyz(
                        body_t.translation.x,
                        body_t.translation.y + 12.0,
                        body_t.translation.z,
                    );
                    *vis = Visibility::Visible;
                    text.0 = v.display_name().to_string();
                }
                None => {
                    *t = Transform::from_xyz(0.0, -100.0, 0.0);
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}
// ============================================================================
// Aura Light (Spec 016 T5.4) — point-light glow gated by eco rank
// ============================================================================

/// The spawned aura light entity (child of the physics body).
#[derive(Resource, Default)]
pub struct AuraLight {
    pub entity: Option<Entity>,
}

/// Pure eco-rank → aura mapping: (color, intensity), None below Enthusiast.
pub fn aura_config(ep: u64) -> Option<(Color, f32)> {
    if ep >= 1000 {
        Some((Color::srgb(1.0, 0.85, 0.3), 6.0))
    } else if ep >= 500 {
        Some((Color::srgb(0.25, 1.0, 0.45), 4.0))
    } else if ep >= 100 {
        Some((Color::srgb(0.3, 0.9, 1.0), 2.5))
    } else {
        None
    }
}

/// Spawn (once) and drive the aura point light from the eco rank.
fn update_aura_light(
    mut commands: Commands,
    mut light: ResMut<AuraLight>,
    player_query: Query<(&crate::player::ClientPlayer, Entity), With<PhysicsBody>>,
    mut existing: Query<(&mut PointLight, &mut Visibility)>,
) {
    let Ok((player, body)) = player_query.single() else { return };
    if light.entity.is_none() {
        light.entity = Some(
            commands
                .spawn((
                    Name::new("eco-aura"),
                    PointLight {
                        color: Color::WHITE,
                        intensity: 0.0,
                        range: 42.0,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 8.0, 0.0),
                ))
                .insert(ChildOf(body))
                .id(),
        );
    }
    let Some(entity) = light.entity else { return };
    let Ok((mut pl, mut vis)) = existing.get_mut(entity) else { return };
    match aura_config(player.eco_points) {
        Some((color, intensity)) => {
            pl.color = color;
            pl.intensity = intensity;
            *vis = Visibility::Visible;
        }
        None => {
            pl.intensity = 0.0;
            *vis = Visibility::Hidden;
        }
    }
}
