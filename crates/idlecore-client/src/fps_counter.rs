//! FPS counter — overlay text in the top-right corner of the screen.
//! Shows real-time frames per second as computed from Bevy's `Time`.

use bevy::prelude::*;

/// Marker for the FPS counter text entity.
#[derive(Component)]
pub struct FpsText;

/// Resource tracking the current FPS value.
#[derive(Resource, Default)]
pub struct FpsCounter {
    /// Current frame rate (FPS).
    pub fps: u32,
    /// Time remaining until the next display update (1-second interval).
    pub accumulator: f32,
}

/// FPS counter UI is spawned as a child of the minimap UI in minimap.rs

/// Update the FPS display every ~0.5 seconds.
pub fn update_fps_counter(
    time: Res<Time>,
    mut counter: ResMut<FpsCounter>,
    mut text_query: Query<&mut Text, With<FpsText>>,
) {
    let delta = time.delta_secs();
    counter.accumulator += delta;

    if counter.accumulator >= 0.5 {
        counter.fps = (1.0 / delta).floor() as u32;
        counter.accumulator = 0.0;

        if let Some(mut text) = text_query.iter_mut().next() {
            **text = format!("FPS: {}", counter.fps);
        }
        if counter.fps < 58 {
            info!("FPS: {}", counter.fps);
        }
    }
}

/// Register the FPS counter plugin.
pub struct FpsCounterPlugin;

impl Plugin for FpsCounterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FpsCounter>();
        app.add_systems(Update, update_fps_counter);
    }
}
