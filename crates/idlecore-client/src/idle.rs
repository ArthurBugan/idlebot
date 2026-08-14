//! Idle Gains Panel — Bevy UI: black square with XP & Gold text

use bevy::prelude::*;
use idlecore_core::idle_config::{gains_for_time, is_idle_eligible};
use std::time::Duration;

/// Resource tracking pending idle gains
#[derive(Resource, Default)]
pub struct IdleGainsState {
    pub pending_xp: u64,
    pub pending_gold: u64,
    pub time_offline: Option<Duration>,
    /// XP earned per second while playing
    pub xp_rate: u64,
    /// Gold earned per second while playing
    pub gold_rate: u64,
}

/// Marker for the XP TextSpan child
#[derive(Component)]
pub struct XpText;

/// Marker for the Gold TextSpan child
#[derive(Component)]
pub struct GoldText;

/// Spawn the idle gains panel as Bevy UI (black rounded square + XP/Gold text)
pub fn spawn_idle_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: FontSource = asset_server.load("fonts/FiraSans-Bold.ttf").into();

    // --- Black square (Node with background color) ---
    commands
        .spawn((
            Name::new("idle_gains_panel"),
            Node {
                position_type: PositionType::Absolute,
                width: px(220),
                height: px(70),
                bottom: px(20),
                left: px(20),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(4),
                ..default()
            },
        ))
        .with_children(|parent| {
            // ---- XP line ----
            parent
                .spawn((
                    Text::default(),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
                    TextShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(1.0, 1.0),
                    },
                ))
                .with_child((
                    TextSpan::new("XP: 0"),
                    TextColor(Color::srgb(0.45, 0.82, 1.0)), // sky blue
                    XpText,
                ));

            // ---- Gold line ----
            parent
                .spawn((
                    Text::default(),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
                    TextShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(1.0, 1.0),
                    },
                ))
                .with_child((
                    TextSpan::new("Gold: 0"),
                    TextColor(Color::srgb(1.0, 0.84, 0.0)), // gold
                    GoldText,
                ));
        });
}

/// Update the idle gains text labels every frame
pub fn update_idle_gains_panel(
    mut idle_state: ResMut<IdleGainsState>,
    time: Res<Time>,
    mut ps: ParamSet<(
        Query<&mut TextSpan, With<XpText>>,
        Query<&mut TextSpan, With<GoldText>>,
    )>,
) {
    // Apply offline gains once at start
    if let Some(duration) = idle_state.time_offline.take() {
        if is_idle_eligible(duration) {
            let gains = gains_for_time(duration);
            idle_state.pending_xp = gains.xp;
            idle_state.pending_gold = gains.gold;
        }
    }

    // Increment gains in real-time based on elapsed frame time
    let dt = time.delta_secs();
    idle_state.pending_xp += (idle_state.xp_rate as f32 * dt) as u64;
    idle_state.pending_gold += (idle_state.gold_rate as f32 * dt) as u64;

    // Update XP span value
    if let Some(mut span) = ps.p0().iter_mut().next() {
        **span = format!("XP: {}", idle_state.pending_xp);
    }

    // Update Gold span value
    if let Some(mut span) = ps.p1().iter_mut().next() {
        **span = format!("Gold: {}", idle_state.pending_gold);
    }
}
