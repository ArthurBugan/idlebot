//! Plugin for the IdleBot game.
//!
//! This plugin manages the game world state and runs the game loop.

use bevy::prelude::*;
use idlecore_core::world::EarthWorld;

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

/// Resource holding the current world state.
#[derive(Resource)]
pub struct WorldResource {
    pub world: EarthWorld,
    pub scale: f32,
}

impl Default for WorldResource {
    fn default() -> Self {
        Self {
            world: EarthWorld::generate(42, 50),
            scale: 0.1,
        }
    }
}

// ---------------------------------------------------------------------------
// Startup Systems
// ---------------------------------------------------------------------------

/// Set up the initial world state.
fn setup_world(mut world_resource: ResMut<WorldResource>) {
    world_resource.world = EarthWorld::generate(42, 100);
}

// ---------------------------------------------------------------------------
// Game Systems
// ---------------------------------------------------------------------------

/// Start playing the game.
fn start_playing(_resource: ResMut<WorldResource>) {
    // Game state transition handled by Bevy states
}

/// Run the next action in the game loop.
fn run_next_action(_resource: Res<WorldResource>) {
    // For now, just sync the minimap
    // TODO: implement next action
}

/// Update the minimap to match the current world state.
fn update_minimap(mut resource: ResMut<WorldResource>) {
    // Get player position from the player transform
    // For now, use the center of the world
    let player_q = 0i32;
    let player_r = 0i32;
    let view_radius = 20i32;

    // Load chunks around player
    resource.world.load_chunks_around(player_q, player_r, view_radius);

    // Unload chunks outside view radius
    resource.world.unload_chunks_around(player_q, player_r, view_radius);
}
