//! Mobile touch controls — a virtual joystick (movement "controller") plus a
//! few on-screen action buttons, so the game is playable without a keyboard.
//!
//! The joystick uses raw `TouchInput` events for reliable dragging; the
//! buttons are ordinary Bevy `Button`s (which already respond to touch). The
//! resulting state lives in [`TouchControls`] and is read by the movement,
//! interact, inventory and zoom systems. Controls are hidden unless the
//! device has a coarse (touch) pointer, and while the inventory/ogin is up.

use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::ui::BorderRadius;

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

/// Which touch (if any) is currently dragging the joystick.
#[derive(Resource, Default)]
struct JoyState {
    active_id: Option<u64>,
}

#[derive(Component)]
struct TouchRoot;

#[derive(Component)]
struct JoyBase;

#[derive(Component)]
struct JoyKnob;

#[derive(Component)]
struct TouchBtn(ButtonKind);

#[derive(Clone, Copy)]
enum ButtonKind {
    Interact,
    Inventory,
    ZoomIn,
    ZoomOut,
}

const JOY_RADIUS: f32 = 64.0;
const KNOB_RADIUS: f32 = 28.0;

/// True on devices with a coarse (touch) pointer.
fn is_touch_device() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::window;
        window()
            .and_then(|w| w.match_media("(pointer: coarse)").ok())
            .flatten()
            .map(|m| m.matches())
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        // The touch state resource is always present so gameplay systems can
        // read it without panicking; the on-screen controller (UI + touch
        // event systems) is only registered on the web (wasm) build.
        app.init_resource::<TouchControls>();
        #[cfg(target_arch = "wasm32")]
        app.init_resource::<JoyState>()
            .add_systems(Startup, spawn_touch_controls)
            .add_systems(
                Update,
                (joystick_touch, joystick_visual, touch_buttons, touch_visibility),
            );
    }
}

fn spawn_touch_controls(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    let show = is_touch_device();
    let disp = if show { Display::Flex } else { Display::None };

    let root = commands
        .spawn((
            TouchRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: disp,
                ..default()
            },
        ))
        .id();

    // --- Virtual joystick (bottom-left) ---
    commands
        .entity(root)
        .with_children(|parent| {
            parent
                .spawn((
                    JoyBase,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(28.0),
                        bottom: Val::Px(28.0),
                        width: Val::Px(JOY_RADIUS * 2.0),
                        height: Val::Px(JOY_RADIUS * 2.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(JOY_RADIUS)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.10, 0.16, 0.45)),
                    BorderColor::all(Color::srgba(0.45, 0.45, 0.55, 0.8)),
                    Interaction::default(),
                ))
                .with_children(|base| {
                    base.spawn((
                        JoyKnob,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(JOY_RADIUS - KNOB_RADIUS),
                            top: Val::Px(JOY_RADIUS - KNOB_RADIUS),
                            width: Val::Px(KNOB_RADIUS * 2.0),
                            height: Val::Px(KNOB_RADIUS * 2.0),
                            border_radius: BorderRadius::all(Val::Px(KNOB_RADIUS)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.55, 0.6, 0.7, 0.9)),
                    ));
                });

            // --- Action buttons (bottom-right) ---
            let btn_specs: [(ButtonKind, &str); 4] = [
                (ButtonKind::Interact, "Act"),
                (ButtonKind::Inventory, "Bag"),
                (ButtonKind::ZoomIn, "+"),
                (ButtonKind::ZoomOut, "−"),
            ];
            for (i, (kind, label)) in btn_specs.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                parent
                    .spawn((
                        TouchBtn(*kind),
                        Button,
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(28.0 + col as f32 * 76.0),
                            bottom: Val::Px(28.0 + row as f32 * 76.0),
                            width: Val::Px(64.0),
                            height: Val::Px(64.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(12.0)),
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

/// Drive the joystick from raw touch events (reliable drag, even off-base).
fn joystick_touch(
    mut touches: MessageReader<TouchInput>,
    base_q: Query<&GlobalTransform, With<JoyBase>>,
    mut touch: ResMut<TouchControls>,
    mut joy: ResMut<JoyState>,
) {
    let Ok(base_gt) = base_q.single() else {
        return;
    };
    let center = base_gt.translation().truncate();
    let max = JOY_RADIUS - KNOB_RADIUS;
    for t in touches.read() {
        let pos = t.position;
        match t.phase {
            TouchPhase::Started => {
                if pos.distance(center) <= JOY_RADIUS * 1.6 {
                    joy.active_id = Some(t.id);
                    set_vec(&mut touch, pos, center, max);
                }
            }
            TouchPhase::Moved => {
                if joy.active_id == Some(t.id) {
                    set_vec(&mut touch, pos, center, max);
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled => {
                if joy.active_id == Some(t.id) {
                    joy.active_id = None;
                    touch.move_vec = Vec2::ZERO;
                }
            }
        }
    }
}

fn set_vec(touch: &mut TouchControls, pos: Vec2, center: Vec2, max: f32) {
    let mut d = pos - center;
    let len = d.length();
    if len > max {
        d = d * (max / len);
    }
    // Screen y is down; world north is +y, so invert.
    touch.move_vec = Vec2::new(d.x / max, -d.y / max);
}

/// Move the knob to follow `move_vec`.
fn joystick_visual(mut knob_q: Query<&mut Node, With<JoyKnob>>, touch: Res<TouchControls>) {
    if let Ok(mut style) = knob_q.single_mut() {
        let off = touch.move_vec * (JOY_RADIUS - KNOB_RADIUS);
        style.left = Val::Px(JOY_RADIUS - KNOB_RADIUS + off.x);
        style.top = Val::Px(JOY_RADIUS - KNOB_RADIUS - off.y);
    }
}

/// Action buttons raise edge-triggered flags consumed by gameplay systems.
fn touch_buttons(
    btn_q: Query<(&Interaction, &TouchBtn), Changed<Interaction>>,
    mut touch: ResMut<TouchControls>,
) {
    for (interaction, kind) in &btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match kind.0 {
            ButtonKind::Interact => touch.interact = true,
            ButtonKind::Inventory => touch.inventory = true,
            ButtonKind::ZoomIn => touch.zoom_in = true,
            ButtonKind::ZoomOut => touch.zoom_out = true,
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
