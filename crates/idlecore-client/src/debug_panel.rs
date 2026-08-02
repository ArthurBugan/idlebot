//! Debug panel using Bevy UI (no external dependencies)
//! Toggle with F1 key

use bevy::prelude::*;

#[derive(Component)]
pub struct DebugPanel;

#[derive(Resource, Default)]
pub struct DebugPanelOpen(pub bool);

pub fn debug_panel_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel_open: ResMut<DebugPanelOpen>,
) {
    if keys.just_pressed(KeyCode::F1) {
        panel_open.0 = !panel_open.0;
    }
}

pub fn spawn_debug_panel(mut commands: Commands, panel_open: Res<DebugPanelOpen>) {
    if panel_open.0 {
        commands
            .spawn((
                Node {
                    width: Val::Percent(30.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
            ))
            .insert(DebugPanel)
            .insert(Name::new("Debug Panel"))
            .with_children(|parent| {
                parent.spawn(Text::new("=== Camera Controls ==="));
                parent.spawn(Text::new("Height: 500.0"));
                parent.spawn(Text::new("Distance: 500.0"));
                parent.spawn(Text::new("Angle: 45°"));

                parent.spawn(Text::new("\n=== World Info ==="));
                parent.spawn(Text::new("World: EarthWorld"));
                parent.spawn(Text::new("Hex Size: 100 units"));
                parent.spawn(Text::new("Map Radius: 50 hexes"));
                parent.spawn(Text::new("Tiles: ~7651"));

                parent.spawn(Text::new("\n=== Controls ==="));
                parent.spawn(Text::new("F1: Toggle this panel"));
                parent.spawn(Text::new("WASD: Move player"));
                parent.spawn(Text::new("M: Toggle minimap"));
            });
    }
}
