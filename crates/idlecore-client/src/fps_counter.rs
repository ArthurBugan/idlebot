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
    /// Time accumulated in the current measurement window.
    pub accumulator: f32,
    /// Frames rendered inside the current measurement window.
    pub frames: u32,
}

/// FPS counter UI is spawned as a child of the minimap UI in minimap.rs

/// Update the FPS display every ~0.5 seconds.
pub fn update_fps_counter(
    time: Res<Time>,
    mut counter: ResMut<FpsCounter>,
    mut text_query: Query<&mut Text, With<FpsText>>,
) {
    counter.accumulator += time.delta_secs();
    counter.frames += 1;

    // Average frames over the whole window: reporting 1/last-delta made a
    // single slow frame read as "20 FPS" while Bevy ran at a steady 60.
    if counter.accumulator >= 0.5 {
        let fps = ((counter.frames as f32) / counter.accumulator).floor() as u32;
        counter.fps = fps;
        counter.accumulator = 0.0;
        counter.frames = 0;

        if let Some(mut text) = text_query.iter_mut().next() {
            **text = format!("FPS: {}", fps);
        }
        if fps > 0 && fps < 45 {
            info!("FPS: {}", fps);
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
