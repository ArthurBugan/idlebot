//! Camera system plugin — 2D camera following the player with zoom.

use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use crate::player::PlayerTransform;
use crate::minimap::MinimapState;
use crate::plugins::player::PhysicsBody;

/// Minimap corner padding (must match `minimap::MINIMAP_PADDING`).
const MINIMAP_PADDING: f32 = 10.0;

/// Pixels per world unit at the default zoom: the 132px-wide isometric tile
/// art covers one hex (√3 × 10 ≈ 17.32 world units).
const BASE_PIXELS_PER_UNIT: f32 = 132.0 / (1.7320508075688772 * 10.0);

/// Camera zoom limits (pixels per world unit).
const MIN_ZOOM: f32 = 3.0;
const MAX_ZOOM: f32 = 2000.0;

/// Camera zoom level (pixels per world unit).
#[derive(Resource)]
pub struct CameraZoom {
    pub scale: f32,
}

impl Default for CameraZoom {
    fn default() -> Self {
        // Default is 3x the 1:1 art scale so the player starts comfortably
        // close (tiles render at 3x their native pixel size).
        Self { scale: BASE_PIXELS_PER_UNIT * 3.0 }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (follow_camera, handle_zoom));
    }
}

/// Camera follow smoothing (1/s): the camera converges on the player with a
/// 1/k time constant, absorbing per-frame jitter (visible on web where frame
/// pacing is uneven). Distances beyond this threshold snap instantly so
/// teleports/session restores don't swoosh across the planet.
const FOLLOW_RATE: f32 = 10.0;
const SNAP_DISTANCE: f32 = 50.0;

/// One follow step: exponential ease toward `target`, snapping when far
/// (teleports). Pure so the feel is unit-testable.
fn follow_step(cam: Vec2, target: Vec2, dt: f32) -> Vec2 {
    let delta = target - cam;
    if delta.length() > SNAP_DISTANCE {
        return target;
    }
    let blend = 1.0 - (-FOLLOW_RATE * dt.min(1.0 / 20.0)).exp();
    cam + delta * blend
}

/// Camera follow system (2D: x = east, y = north). Eases toward the player
/// sprite entity (covers teleports/restores), falling back to the
/// `PlayerTransform` resource before the sprite exists.
fn follow_camera(
    player_transform: Res<PlayerTransform>,
    zoom: Res<CameraZoom>,
    body: Query<&Transform, (With<PhysicsBody>, Without<Camera2d>)>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    time: Res<Time>,
) {
    let Ok((mut camera_transform, mut projection)) = camera.single_mut() else {
        return;
    };
    let target = body
        .single()
        .ok()
        .map(|t| t.translation)
        .unwrap_or(player_transform.translation);

    // Exponential ease with snap for teleports: smooth at walk range,
    // instant across continents.
    let next = follow_step(
        camera_transform.translation.xy(),
        target.xy(),
        time.delta_secs(),
    );
    camera_transform.translation.x = next.x;
    camera_transform.translation.y = next.y;
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        // Bevy 0.19 `scale` is world-units-per-pixel (higher = zoomed OUT),
        // while `CameraZoom` tracks pixels per world unit (higher = zoomed IN).
        ortho.scale = 1.0 / zoom.scale.max(1e-4);
    }
}

/// Is the cursor currently over the minimap (top-right corner)?
fn cursor_over_minimap(
    windows: &Query<&Window>,
    minimap_state: &MinimapState,
) -> bool {
    let Ok(window) = windows.single() else { return false };
    let Some(cursor) = window.cursor_position() else { return false };
    let mm_size = minimap_state.mm_size();
    let mm_left = window.width() - MINIMAP_PADDING - mm_size;
    let mm_top = MINIMAP_PADDING;
    cursor.x >= mm_left
        && cursor.x < mm_left + mm_size
        && cursor.y >= mm_top
        && cursor.y < mm_top + mm_size
}

/// Handle camera zoom: mouse wheel (over the map, not the minimap) and +/- keys.
fn handle_zoom(
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    minimap_state: Res<MinimapState>,
    mut zoom: ResMut<CameraZoom>,
    mut touch: ResMut<crate::touch::TouchControls>,
) {
    // Zoom step per wheel notch / +/- key press (1.3 = +30% per step).
    let factor: f32 = 1.3;
    // Browsers report wheel deltas in *pixels* (one notch ≈ 100px), not
    // integer Line notches. Feeding that raw `event.y` into `factor.powf`
    // makes a single scroll notch jump straight to MAX_ZOOM, so normalize
    // pixel deltas to notch units first.
    const PIXELS_PER_NOTCH: f32 = 100.0;

    for event in scroll.read() {
        if event.y == 0.0 || cursor_over_minimap(&windows, &minimap_state) {
            continue;
        }
        // Trackpads (Pixel deltas, macOS natural scrolling) report the
        // "zoom in" gesture as negative y; mice (Line deltas) report wheel
        // up as positive. Normalize both so up/forward zooms in.
        let dir = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => -event.y / PIXELS_PER_NOTCH,
        };
        // Clamp per-event step so a large/coalesced delta can't lurch.
        let dir = dir.clamp(-2.0, 2.0);
        zoom.scale = (zoom.scale * factor.powf(dir)).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        zoom.scale = (zoom.scale * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        info!("Camera zoom: {:.1} px/unit", zoom.scale);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        zoom.scale = (zoom.scale / factor).clamp(MIN_ZOOM, MAX_ZOOM);
        info!("Camera zoom: {:.1} px/unit", zoom.scale);
    }
    if std::mem::take(&mut touch.zoom_in) {
        zoom.scale = (zoom.scale * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        info!("Camera zoom: {:.1} px/unit", zoom.scale);
    }
    if std::mem::take(&mut touch.zoom_out) {
        zoom.scale = (zoom.scale / factor).clamp(MIN_ZOOM, MAX_ZOOM);
        info!("Camera zoom: {:.1} px/unit", zoom.scale);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_snaps_over_long_distances() {
        let cam = Vec2::ZERO;
        let far = Vec2::new(10_000.0, 0.0);
        assert_eq!(follow_step(cam, far, 1.0 / 60.0), far);
    }

    #[test]
    fn camera_eases_and_converges_when_close() {
        let mut cam = Vec2::ZERO;
        let target = Vec2::new(10.0, 0.0);
        for _ in 0..120 {
            cam = follow_step(cam, target, 1.0 / 60.0);
        }
        assert!((cam - target).length() < 0.05, "did not converge: {cam}");
        // Never overshoots (pure lerp).
        assert!(cam.x <= target.x + 1e-4);
    }
}
