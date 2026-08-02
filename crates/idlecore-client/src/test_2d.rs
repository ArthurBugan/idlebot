//! Test 2D UI rendering

use bevy::prelude::*;

pub struct Test2dPlugin;

impl Plugin for Test2dPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui_test);
    }
}

fn setup_ui_test(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let font_source = FontSource::Handle(font);
    
    // UI Panel
    commands.spawn((
        Name::new("ui_panel"),
        Node {
            position_type: PositionType::Absolute,
            width: px(400),
            height: px(100),
            bottom: px(20),
            left: px(20),
            padding: UiRect::all(px(10)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
    ));
    
    // Text
    commands.spawn((
        Name::new("ui_text"),
        Text::new("TEST"),
        TextFont {
            font: font_source,
            font_size: FontSize::Px(48.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new(
            Justify::Center,
            bevy::text::LineBreak::NoWrap,
        ),
        TextShadow::default(),
    ));
    
    // Camera2d on top
    commands.spawn((
        Name::new("ui_camera"),
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            order: 1,
            ..default()
        },
    ));
}
