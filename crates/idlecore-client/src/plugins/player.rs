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
        app.add_systems(Update, (
            player_movement.after(PhysicsSet::Writeback),
            sync_visual_to_physics.after(player_movement),
            player_animation,
        ));
        app.add_systems(Startup, (register_player_orientation, register_player_animations));
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
    mut player_query: Query<(&mut Transform, &mut Velocity, &mut crate::player::ClientPlayer), With<PhysicsBody>>,
    mut player_transform: ResMut<PlayerTransform>,
    mut orientation: ResMut<PlayerOrientation>,
) {
    let Ok((mut transform, mut velocity, mut player)) = player_query.single_mut() else {
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
    // body so the step is climbable instead of becoming a wall.
    if dir != Vec2::ZERO {
        let x = transform.translation.x;
        let z = transform.translation.z;
        let ahead_x = x + dir.x * STEP_PROBE_DIST;
        let ahead_z = z + dir.y * STEP_PROBE_DIST;
        let here = terrain_height_at(&streaming_world, x, z);
        let ahead = terrain_height_at(&streaming_world, ahead_x, ahead_z);
        if ahead > here && ahead - here <= MAX_STEP_HEIGHT {
            transform.translation.y = ahead - FEET_OFFSET;
        }
    }

    if dir != Vec2::ZERO {
        orientation.facing_angle = input.y.atan2(input.x);
        transform.rotation = Quat::from_rotation_y(std::f32::consts::PI / 2.0 - orientation.facing_angle);
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
/// so copy the body's world pose onto the visual root every frame.
fn sync_visual_to_physics(
    bodies: Query<&Transform, With<PhysicsBody>>,
    mut roots: Query<&mut Transform, (With<Player>, Without<PhysicsBody>)>,
) {
    let Ok(body) = bodies.single() else { return };
    let Ok(mut root) = roots.single_mut() else { return };
    root.translation = body.translation;
    root.rotation = body.rotation;
}
