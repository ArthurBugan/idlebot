//! Mobile touch controls using virtual_joystick crate — a virtual joystick
//! plus on-screen action buttons, so the game is playable without a keyboard.
//!
//! Uses the `virtual_joystick` Bevy plugin which provides proper touch handling,
//! floating/dynamic joysticks, axis locking, and works with mouse on desktop.

use bevy::prelude::*;
use virtual_joystick::{
    JoystickFloating, NoAction, VirtualJoystickBundle, VirtualJoystickInteractionArea, VirtualJoystickMessage,
    VirtualJoystickMessageType, VirtualJoystickNode, VirtualJoystickPlugin,
    VirtualJoystickUIBackground, VirtualJoystickUIKnob,
};

/// Live touch input, written by the on-screen controls and read by gameplay.
#[derive(Resource, Default)]
pub struct TouchControls {
    /// Normalized movement vector (-1..1 each axis) from the joystick.
    pub move_vec: Vec2,
    /// Edge-triggered action flags, each consumed by its owning system.
    pub interact: bool,
    pub inventory: bool,
    pub zoom_in: bool,
    pub zoom_out: bool,
}

#[derive(Component)]
struct TouchRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Reflect)]
#[reflect(Default, Hash, PartialEq)]
enum JoystickId {
    #[default]
    Main,
}

#[derive(Component, Clone, Copy)]
struct ButtonKind(ButtonAction);

#[derive(Clone, Copy)]
enum ButtonAction {
    Interact,
    Inventory,
    ZoomIn,
    ZoomOut,
}

pub struct TouchPlugin;

#[cfg(target_arch = "wasm32")]
impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchControls>()
            .add_plugins(VirtualJoystickPlugin::<JoystickId>::default())
            .add_systems(Startup, spawn_touch_controls)
            .add_systems(
                Update,
                (
                    joystick_input,
                    touch_buttons,
                    touch_visibility,
                ),
            );
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchControls>();
    }
}

fn spawn_touch_controls(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");

    let root = commands
        .spawn((
            TouchRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: Display::Flex,
                ..default()
            },
        ))
        .id();

// --- Virtual joystick (bottom-left, floating) ---
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            VirtualJoystickBundle::new(
                VirtualJoystickNode::<JoystickId>::default()
                    .with_id(JoystickId::Main)
                    .with_behavior(JoystickFloating)
                    .with_action(NoAction),
            )
            .set_style(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                bottom: Val::Percent(0.0),
                ..default()
            }),
            BackgroundColor(Color::srgba(0.10, 0.10, 0.16, 0.45)),
        ))
        .with_children(|parent| {
            // Interaction Area
            parent.spawn((
                VirtualJoystickInteractionArea,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            // Knob
            parent.spawn((
                VirtualJoystickUIKnob,
                ImageNode {
                    color: Color::srgba(0.55, 0.6, 0.7, 0.9),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(80.0),
                    height: Val::Px(80.0),
                    ..default()
                },
                ZIndex(1),
            ));
            // Background/Outline
            parent.spawn((
                VirtualJoystickUIBackground,
                ImageNode {
                    color: Color::srgba(0.10, 0.10, 0.16, 0.45),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(160.0),
                    height: Val::Px(160.0),
                    ..default()
                },
                ZIndex(0),
            ));
        });
    });

    // --- Action buttons (bottom-right) ---
    commands
        .entity(root)
        .with_children(|parent| {

            // --- Action buttons (bottom-right) ---
            let btn_specs: [(ButtonAction, &str); 4] = [
                (ButtonAction::Interact, "Act"),
                (ButtonAction::Inventory, "Bag"),
                (ButtonAction::ZoomIn, "+"),
                (ButtonAction::ZoomOut, "−"),
            ];
            for (i, (kind, label)) in btn_specs.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                parent
                    .spawn((
                        Button,
                        ButtonKind(*kind),
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(28.0 + col as f32 * 76.0),
                            bottom: Val::Px(28.0 + row as f32 * 76.0),
                            width: Val::Px(64.0),
                            height: Val::Px(64.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: bevy::ui::BorderRadius::all(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.16, 0.18, 0.26, 0.85)),
                        BorderColor::all(Color::srgba(0.45, 0.5, 0.6, 0.8)),
                        Interaction::default(),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(*label),
                            TextFont {
                                font: FontSource::Handle(font.clone()),
                                font_size: 18.0.into(),
                                ..default()
                            },
                            TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        ));
                    });
            }
        });
}

/// Read joystick axis and update TouchControls.move_vec
fn joystick_input(
    mut reader: MessageReader<VirtualJoystickMessage<JoystickId>>,
    mut touch: ResMut<TouchControls>,
) {
    for msg in reader.read() {
        // Only care about drag (movement) events
        if matches!(msg.get_type(), VirtualJoystickMessageType::Drag) {
            let axis = msg.axis();
            touch.move_vec = Vec2::new(axis.x, -axis.y); // invert Y: screen down = world north
        }
        // On release, reset
        if matches!(msg.get_type(), VirtualJoystickMessageType::Up) {
            touch.move_vec = Vec2::ZERO;
        }
    }
}

/// Action buttons raise edge-triggered flags consumed by gameplay systems.
fn touch_buttons(
    btn_q: Query<(&Interaction, &ButtonKind), Changed<Interaction>>,
    mut touch: ResMut<TouchControls>,
) {
    for (interaction, kind) in &btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match kind.0 {
            ButtonAction::Interact => touch.interact = true,
            ButtonAction::Inventory => touch.inventory = true,
            ButtonAction::ZoomIn => touch.zoom_in = true,
            ButtonAction::ZoomOut => touch.zoom_out = true,
        }
    }
}

/// Hide the controls during login and while the inventory is open.
fn touch_visibility(
    net: Res<crate::net::plugin::Net>,
    inv: Res<crate::inventory::Inventory>,
    mut root_q: Query<&mut Node, With<TouchRoot>>,
) {
    let visible = net.address.is_some() && !inv.open;
    if let Ok(mut node) = root_q.single_mut() {
        let want = if visible { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}