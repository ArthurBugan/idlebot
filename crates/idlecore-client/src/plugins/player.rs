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
    pub playing: Option<String>,
}

impl Default for PlayerAnimationState {
    fn default() -> Self {
        Self {
            gltf: Handle::default(),
            graph: None,
            clips: default(),
            player_entity: None,
            playing: None,
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
/// `AnimationPlayer` once the scene spawns, then play idle/run by input.
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
        let mut graph = AnimationGraph::new();
        for (name, clip) in &gltf.named_animations {
            let node = graph.add_clip(clip.clone(), 1.0, graph.root);
            state.clips.insert(name.to_string(), node);
        }
        if state.clips.is_empty() {
            return;
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
            break;
        }
        if state.player_entity.is_none() {
            return;
        }
    }

    // 3. Switch animation based on input (with a short crossfade).
    let target = if keyboard.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]) {
        "run"
    } else {
        "idle"
    };
    if state.playing.as_deref() == Some(target) {
        return;
    }
    let Some(node_index) = state.clips.get(target) else { return };
    let Ok((_, mut player, Some(mut transitions))) = animation_players.get_mut(state.player_entity.unwrap()) else {
        return
    };
    transitions
        .play(&mut player, *node_index, std::time::Duration::from_millis(150))
        .repeat();
    state.playing = Some(target.to_string());
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
