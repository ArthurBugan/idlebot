//! Minimap Bevy rendering and input systems.

use bevy::prelude::*;
use idlecore_core::hex::HexCoord;

/// Spawn the minimap background quad and attach the minimap component.
pub fn spawn_minimap_overlay(
    mut commands: Commands,
    cameras: Query<&Camera>,
    windows: Query<&Window>,
) {
    let cam = cameras.single();
    if !cam.is_active {
        return;
    }

    let win_w = windows.iter().next().map(|w| w.width() as f32).unwrap_or(1920.0);
    let win_h = windows.iter().next().map(|w| w.height() as f32).unwrap_or(1080.0);
    let minimap_size = 180.0;
    let screen_offset_x = win_w - minimap_size - 10.0;
    let screen_offset_y = 10.0;

    commands.spawn((
        Name::new("minimap_bg"),
        Sprite {
            color: Color::srgba(0.05, 0.05, 0.10, 0.85),
            custom_size: Some(Vec2::splat(minimap_size)),
            ..default()
        },
        Transform::from_xyz(
            screen_offset_x + minimap_size / 2.0,
            screen_offset_y + minimap_size / 2.0,
            1000.0,
        ),
        bevy::prelude::UiRect::new(
            Val::Px(screen_offset_x),
            Val::Px(screen_offset_x + minimap_size),
            Val::Px(screen_offset_y),
            Val::Px(screen_offset_y + minimap_size),
        ),
        crate::minimap::MinimapComponent::new(10.0, win_w, win_h),
    ));
}

/// Sync player position into the minimap component each frame.
pub fn sync_player_to_minimap(
    player: Query<&Transform, With<crate::player::ClientPlayer>>,
    mut minimap: Query<&mut crate::minimap::MinimapComponent>,
) {
    let Ok(t) = player.single() else {
        return;
    };
    let Ok(mut mm) = minimap.single_mut() else {
        return;
    };
    mm.set_player_pos(Vec2::new(t.translation.x, t.translation.z));
}

/// Spawn/update hex sprites for the minimap viewport.
pub fn render_minimap_hexes(
    minimap: Query<&crate::minimap::MinimapComponent>,
    mut hex_sprites: Query<(&crate::minimap::MinimapHexSprite, &mut Transform)>,
) {
    let mm = minimap.single();
    let hex_set: std::collections::HashSet<HexCoord> = mm.viewport_hexes.iter().copied().collect();

    for (sprite, mut transform) in hex_sprites.iter_mut() {
        if !hex_set.contains(&sprite.hex) {
            transform.translation.z = -100.0; // hide off-screen
        } else {
            transform.translation.z = 900.0;
        }
    }
}

/// Minimap keyboard/mouse control system.
pub fn handle_minimap_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<MouseScroll>,
    mut minimap: Query<(Entity, &mut crate::minimap::MinimapComponent)>,
    mut selected: Local<Option<HexCoord>>,
) {
    let (entity, mut mm) = minimap.single_mut();

    // Zoom controls
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadMinus) {
        mm.zoom_in();
    }
    if keyboard.just_pressed(KeyCode::Equals) || keyboard.just_pressed(KeyCode::NumpadEqual) {
        mm.zoom_out();
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        mm.toggle_global_map();
    }

    // Scroll wheel zoom
    for scroll in mouse.iter() {
        if scroll.y < 0.0 {
            mm.zoom_in();
        } else if scroll.y > 0.0 {
            mm.zoom_out();
        }
    }

    *selected = mm.selected_destination();
    let _ = entity;
}

/// Spawn minimap hex sprites when viewport changes.
pub fn refresh_minimap_sprites(
    minimap: Query<(Entity, &crate::minimap::MinimapComponent)>,
    mut commands: Commands,
    hex_sprites: Query<(Entity, &crate::minimap::MinimapHexSprite)>,
) {
    let (entity, mm) = minimap.single();

    // Despawn sprites no longer in viewport
    let visible_set: std::collections::HashSet<HexCoord> = mm.viewport_hexes.iter().copied().collect();
    for (sprite_ent, sprite) in hex_sprites.iter() {
        if !visible_set.contains(&sprite.hex) {
            commands.entity(sprite_ent).despawn();
        }
    }

    // Spawn new sprites for hexes not yet rendered
    let existing: std::collections::HashSet<(i32, i32)> = hex_sprites
        .iter()
        .map(|(_, s)| (s.hex.q, s.hex.r))
        .collect();

    for hex in &mm.viewport_hexes {
        if !existing.contains(&(hex.q, hex.r)) {
            let world_pos = mm.hex_to_world(hex);
            let screen = mm.world_to_screen(world_pos);
            let color = mm.hex_terrain_color(hex);

            commands.spawn((
                Name::new(format!("minimap_hex_{}_{}", hex.q, hex.r)),
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(8.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(screen.x, screen.y, 900.0)),
                crate::minimap::MinimapHexSprite {
                    hex: *hex,
                    screen_pos: screen,
                    color,
                },
            ));
        }
    }
}
