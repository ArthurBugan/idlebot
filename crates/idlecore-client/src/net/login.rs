//! Pre-game login screen — pick a name to claim your account while wallet
//! auth isn't wired yet (Spec 013 chain SDK pending).
//!
//! The name doubles as the account key (the server lowercases it). The page
//! hides once `Net.address` is set (login confirmed) and shows the live
//! connection status meanwhile — including the stale-token self-heal
//! (`401` → fresh identity → automatic retry).

use bevy::prelude::*;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use super::plugin::{Net, NetStatus};

/// Marks the fullscreen login overlay; hidden once logged in.
#[derive(Component)]
pub struct LoginPageRoot;

/// The editable name display.
#[derive(Component)]
pub struct LoginNameText;

/// Live connection/login status line.
#[derive(Component)]
pub struct LoginStatusText;

/// The "Enter World" button.
#[derive(Component)]
pub struct LoginButton;

/// The name field; tappable on touch to focus the HTML input.
#[derive(Component)]
pub struct LoginNameField;

/// Login screen state: the in-progress name buffer.
#[derive(Resource, Default)]
pub struct LoginPage {
    pub buffer: String,
}

const MAX_NAME: usize = 20;

pub struct LoginPlugin;

impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginPage>()
            .add_systems(Startup, spawn_login_page)
            .add_systems(Update, (
                login_page_input,
                login_page_button,
                login_page_sync,
            ));
        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<wasm_login::LoginInputBridge>()
                .add_systems(Startup, wasm_login::spawn_html_input)
                .add_systems(Update, wasm_login::sync_html_input);
        }
    }
}

/// Build the fullscreen overlay: title, name field, Enter World button,
/// status line.
fn spawn_login_page(mut commands: Commands) {
    commands
        .spawn((
            Name::new("login-page"),
            LoginPageRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.92)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::axes(Val::Px(56.0), Val::Px(40.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.97)),
                    BorderColor::all(Color::srgba(0.22, 0.5, 0.32, 0.95)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("IdleBot"),
                        TextFont { font_size: 36.0.into(), ..default() },
                        TextColor(Color::srgb(0.45, 0.9, 0.55)),
                    ));
                    panel.spawn((
                        Text::new("claim your plot — enter a name"),
                        TextFont { font_size: 14.0.into(), ..default() },
                        TextColor(Color::srgba(0.7, 0.78, 0.7, 0.9)),
                    ));
                    // Name field. Tappable on touch (Button + LoginNameField)
                    // to raise the on-screen keyboard via the hidden HTML input.
                    panel
                        .spawn((
                            Button,
                            Interaction::default(),
                            LoginNameField,
                            Node {
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                                min_width: Val::Px(280.0),
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 1.0)),
                            BorderColor::all(Color::srgba(0.35, 0.4, 0.35, 0.9)),
                        ))
                        .with_child((
                            Text::new("your name_"),
                            TextFont { font_size: 18.0.into(), ..default() },
                            TextColor(Color::srgb(0.92, 0.95, 0.9)),
                            LoginNameText,
                        ));
                    // Enter World button.
                    panel
                        .spawn((
                            Button,
                            Interaction::default(),
                            LoginButton,
                            Node {
                                width: Val::Px(240.0),
                                height: Val::Px(46.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.16, 0.45, 0.24, 1.0)),
                        ))
                        .with_child((
                            Text::new("Enter World"),
                            TextFont { font_size: 18.0.into(), ..default() },
                            TextColor(Color::srgb(0.95, 1.0, 0.95)),
                        ));
                    panel.spawn((
                        Text::new("connecting…"),
                        TextFont { font_size: 13.0.into(), ..default() },
                        TextColor(Color::srgba(0.65, 0.7, 0.65, 0.9)),
                        LoginStatusText,
                    ));
                });
        });
}

/// Name typing: alphanumeric + `_`, ENTER submits, Backspace deletes,
/// ESC clears. Only owns the keyboard while not logged in.
fn login_page_input(
    mut keys: MessageReader<KeyboardInput>,
    mut net: ResMut<Net>,
    mut page: ResMut<LoginPage>,
) {
    if net.address.is_some() {
        return;
    }
    for event in keys.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let name = std::mem::take(&mut page.buffer);
                net.request_login(name);
            }
            KeyCode::Backspace => {
                page.buffer.pop();
            }
            KeyCode::Escape => page.buffer.clear(),
            _ => {
                if let Some(text) = &event.text {
                    for ch in text.chars().filter(|c| c.is_alphanumeric() || *c == '_') {
                        if page.buffer.chars().count() < MAX_NAME {
                            page.buffer.push(ch);
                        }
                    }
                }
            }
        }
    }
}

/// Clicking "Enter World" submits the name.
fn login_page_button(
    interactions: Query<(&Interaction, &LoginButton), Changed<Interaction>>,
    mut net: ResMut<Net>,
    mut page: ResMut<LoginPage>,
) {
    for (interaction, _) in &interactions {
        if *interaction == Interaction::Pressed {
            let name = std::mem::take(&mut page.buffer);
            net.request_login(name);
        }
    }
}

/// Hide the page once logged in; keep the name + status lines live.
fn login_page_sync(
    net: Res<Net>,
    page: Res<LoginPage>,
    mut root_q: Query<&mut Visibility, With<LoginPageRoot>>,
    mut name_q: Query<&mut Text, (With<LoginNameText>, Without<LoginStatusText>)>,
    mut status_q: Query<&mut Text, (With<LoginStatusText>, Without<LoginNameText>)>,
) {
    let Ok(mut vis) = root_q.single_mut() else { return };
    let logged_in = net.address.is_some();
    *vis = if logged_in { Visibility::Hidden } else { Visibility::Visible };
    if logged_in {
        return;
    }
    if let Ok(mut t) = name_q.single_mut() {
        t.0 = if page.buffer.is_empty() {
            "your name_".to_string()
        } else {
            format!("{}_", page.buffer)
        };
    }
    let status = match (&net.status, &net.pending_name) {
        (NetStatus::Connected, Some(name)) => format!("logging in as {name}…"),
        (NetStatus::Connected, None) => "connected — enter a name and press Enter".to_string(),
        (NetStatus::Connecting, _) | (NetStatus::Disconnected, _) => "connecting…".to_string(),
        (NetStatus::Error(e), _) => {
            // Stale-token self-heal surfaces here: the client already cleared
            // the file and is reconnecting anonymously.
            format!("connection issue — retrying ({e})")
        }
    };
    if let Ok(mut t) = status_q.single_mut() {
        t.0 = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_page_defaults() {
        let page = LoginPage::default();
        assert!(page.buffer.is_empty());
    }
}

/// Wasm-only: an HTML `<input>` overlay so the mobile soft keyboard can type
/// the username. Bevy's canvas swallows on-screen keyboard events, so we layer
/// a real input element (hidden until the name field is tapped) and sync its
/// value into `LoginPage.buffer`. Keydown is stopped from propagating so Bevy's
/// `login_page_input` doesn't also echo the keys.
#[cfg(target_arch = "wasm32")]
mod wasm_login {
    use super::*;
    use std::sync::{Arc, Mutex};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{Event, HtmlInputElement, KeyboardEvent};

    /// Shared bridge between the DOM input and Bevy's `LoginPage`.
    #[derive(Resource)]
    pub struct LoginInputBridge {
        pub inner: Arc<Mutex<Bridge>>,
    }

    impl Default for LoginInputBridge {
        fn default() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Bridge::default())),
            }
        }
    }

    pub struct Bridge {
        pub value: String,
        pub submit: bool,
    }

    impl Default for Bridge {
        fn default() -> Self {
            Self {
                value: String::new(),
                submit: false,
            }
        }
    }

    const INPUT_ID: &str = "idlebot-login-input";

    pub fn spawn_html_input(bridge: Res<LoginInputBridge>) {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let Ok(el) = document.create_element("input") else { return };
        let Ok(input) = el.dyn_into::<HtmlInputElement>() else { return };
        input.set_id(INPUT_ID);
        input.set_attribute("type", "text").unwrap();
        input.set_attribute("inputmode", "text").unwrap();
        input.set_attribute("autocapitalize", "none").unwrap();
        input.set_attribute("autocomplete", "off").unwrap();
        input.set_attribute("enterkeyhint", "go").unwrap();
        let style = input.style();
        let _ = style.set_property("position", "absolute");
        let _ = style.set_property("left", "50%");
        let _ = style.set_property("top", "50%");
        let _ = style.set_property("transform", "translate(-50%, -120px)");
        let _ = style.set_property("width", "280px");
        let _ = style.set_property("font-size", "18px");
        let _ = style.set_property("padding", "10px 16px");
        let _ = style.set_property("border-radius", "4px");
        // Hidden and non-interactive until the name field is tapped.
        let _ = style.set_property("opacity", "0");
        let _ = style.set_property("pointer-events", "none");
        let _ = style.set_property("z-index", "10");
        if let Some(body) = document.body() {
            let _ = body.append_child(&input);
        }

        let b = bridge.inner.clone();
        let b_submit = bridge.inner.clone();
        let input_val = input.clone();
        let on_input = Closure::wrap(Box::new(move |_e: Event| {
            b.lock().unwrap().value = input_val.value();
        }) as Box<dyn FnMut(Event)>);
        let _ = input.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref());
        on_input.forget();

        let input_for_key = input.clone();
        let on_key = Closure::wrap(Box::new(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                b_submit.lock().unwrap().submit = true;
                let _ = input_for_key.blur();
            }
            e.stop_propagation();
        }) as Box<dyn FnMut(KeyboardEvent)>);
        let _ = input.add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref());
        on_key.forget();
    }

    pub fn sync_html_input(
        bridge: Res<LoginInputBridge>,
        mut page: ResMut<LoginPage>,
        mut net: ResMut<Net>,
        name_field_q: Query<&Interaction, With<LoginNameField>>,
    ) {
        // Once logged in, the overlay input is no longer needed — hide it so
        // it doesn't linger in the middle of the screen during gameplay.
        if net.address.is_some() {
            let _ = hide_html_input();
            return;
        }
        // Tapping the name field focuses the hidden input (raises keyboard).
        for i in &name_field_q {
            if *i == Interaction::Pressed {
                let Some(window) = web_sys::window() else { break };
                let Some(document) = window.document() else { break };
                let Some(el) = document.get_element_by_id(INPUT_ID) else { break };
                let Ok(input) = el.dyn_into::<HtmlInputElement>() else { break };
                let style = input.style();
                let _ = style.set_property("pointer-events", "auto");
                let _ = style.set_property("opacity", "1");
                input.set_value(&page.buffer);
                let _ = input.focus();
            }
        }

        let mut b = bridge.inner.lock().unwrap();
        if b.value != page.buffer {
            page.buffer = b.value.clone();
        }
        if b.submit {
            b.submit = false;
            let name = std::mem::take(&mut page.buffer);
            b.value = String::new();
            let _ = hide_html_input();
            net.request_login(name);
        }
    }

    /// Hide the DOM input overlay (opacity 0, non-interactive) so it can't sit
    /// visibly in the middle of the screen during gameplay.
    fn hide_html_input() -> Result<(), ()> {
        let window = web_sys::window().ok_or(())?;
        let document = window.document().ok_or(())?;
        let el = document.get_element_by_id(INPUT_ID).ok_or(())?;
        let input = el.dyn_into::<HtmlInputElement>().map_err(|_| ())?;
        let style = input.style();
        let _ = style.set_property("pointer-events", "none");
        let _ = style.set_property("opacity", "0");
        let _ = input.blur();
        Ok(())
    }
}
