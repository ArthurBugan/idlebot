//! Mobile touch controls using virtual_joystick crate — a virtual joystick
//! plus on-screen action buttons, so the game is playable without a keyboard.
//!
//! Uses the `virtual_joystick` Bevy plugin which provides proper touch handling,
//! floating/dynamic joysticks, axis locking, and works with mouse on desktop.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::asset::RenderAssetUsages;
use virtual_joystick::{
    create_joystick, JoystickFloating, NoAction, VirtualJoystickMessage,
    VirtualJoystickMessageType, VirtualJoystickPlugin,
};

/// Live touch input, written by the on-screen controls and read by gameplay.
#[derive(Resource, Default)]
pub struct TouchControls {
    /// Normalized movement vector (-1..1 each axis) from the joystick.
    pub move_vec: Vec2,
    /// Whether the joystick is at max radius (triggers sprint).
    pub sprint: bool,
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

fn spawn_touch_controls(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
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

    // Create solid-color images for knob and background
    let knob_img = images.add(Image::new_fill(
        Extent3d { width: 80, height: 80, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[140, 153, 178, 230], // srgba(0.55, 0.6, 0.7, 0.9)
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let background_img = images.add(Image::new_fill(
        Extent3d { width: 160, height: 160, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[25, 25, 40, 115], // srgba(0.10, 0.10, 0.16, 0.45)
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));

    // Use the crate's helper to create a properly structured floating joystick
    create_joystick(
        &mut commands,
        JoystickId::Main,
        knob_img,
        background_img,
        None, // knob_color override
        None, // background_color override
        Some(Color::srgba(0.10, 0.10, 0.16, 0.45)), // interaction area color
        Vec2::new(80.0, 80.0), // knob size
        Vec2::new(160.0, 160.0), // background size
        Node {
            width: Val::Px(180.0),
            height: Val::Px(180.0),
            position_type: PositionType::Absolute,
            left: Val::Px(24.0),
            bottom: Val::Px(24.0),
            ..default()
        },
        JoystickFloating,
        NoAction,
    );

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
            // Library returns screen coords (down=+Y), world uses north=+Y, so invert Y.
            // Also check if at max radius (length ~1.0) to trigger sprint.
            let len = axis.length();
            touch.move_vec = Vec2::new(axis.x, -axis.y);
            touch.sprint = len >= 0.95;
        }
        // On release, reset
        if matches!(msg.get_type(), VirtualJoystickMessageType::Up) {
            touch.move_vec = Vec2::ZERO;
            touch.sprint = false;
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