//! Plugin for the IdleBot game.
//!
//! This plugin manages the game world state and runs the game loop.

use bevy::prelude::*;
use idlecore_core::world_gen::{ChunkManager, WorldGenConfig};

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

/// Plugin that manages the game world.
#[derive(Debug, Clone)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.add_systems(Startup, setup_world);
        app.add_systems(
            Update,
            (update_minimap, run_next_action).run_if(in_state(GameState::Playing)),
        );
        app.add_systems(OnEnter(GameState::Playing), start_playing);
    }
}

// ---------------------------------------------------------------------------
// World State
// ---------------------------------------------------------------------------

/// The state of the game.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, States)]
pub enum GameState {
    /// Game is not currently being played.
    #[default]
    NotPlaying,
    /// Game is currently being played.
    Playing,
}

/// Resource holding the streaming (whole-world) hex data model.
///
/// This is the world-scale representation described by the hex-world spec:
/// chunks are generated deterministically from a seed and cached *only*
/// around the active area, so the full world never exists in memory.
#[derive(Resource)]
pub struct StreamingWorldResource {
    pub config: WorldGenConfig,
    pub chunks: ChunkManager,
}

impl Default for StreamingWorldResource {
    fn default() -> Self {
        let config = WorldGenConfig {
            seed: 42,
            world_radius: 100,
            flat: true,
        };
        let chunks = ChunkManager::new(
            WorldGenConfig::CHUNK_SIZE,
            4, // active radius in chunks
            6, // prefetch radius in chunks
        );
        Self {
            config,
            chunks,
        }
    }
}

// ---------------------------------------------------------------------------
// Startup Systems
// ---------------------------------------------------------------------------

/// Set up the initial world state (idempotent; generation is lazy/streamed).
fn setup_world(_streaming_world: ResMut<StreamingWorldResource>) {
    // Nothing to pre-generate — chunks stream around the player on demand.
}

// ---------------------------------------------------------------------------
// Game Systems
// ---------------------------------------------------------------------------

/// Start playing the game.
fn start_playing(_resource: ResMut<StreamingWorldResource>) {
    // Game state transition handled by Bevy states.
}

/// Run the next action in the game loop.
fn run_next_action(_resource: Res<StreamingWorldResource>) {
    // TODO: implement next action.
}

/// Update the streaming world from the current player position.
fn update_minimap(mut resource: ResMut<StreamingWorldResource>) {
    // The minimap systems (`load_nearby_chunks`) handle player-centered
    // streaming; this simply keeps the world anchored at the origin for
    // scenarios before the player position is known.
    let player_q = 0i32;
    let player_r = 0i32;
    let config = resource.config;
    resource.chunks.stream_around(&config, player_q, player_r);
}