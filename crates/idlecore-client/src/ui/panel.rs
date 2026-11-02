//! The shared panel shell (Spec 029): one top-centred frame with a tab rail
//! and one open-state resource. The three overlay panels — Inventory
//! (Tab / I), Weapon Forge (G) and Buildings (B) — keep all of their content
//! logic; they re-home as [`TabContent`] roots inside this frame's body.
//!
//! Key mapping (all historic keys keep working):
//!   • Tab / I  → open onto Inventory, switch to it, or close it
//!   • G        → same cycle for the Forge
//!   • B        → same cycle for Buildings
//!   • ESC      → close the panel (world ESC semantics unaffected)
//!
//! Panels are exclusive: opening one never overlaps another (the old bug:
//! inventory and forge anchored the identical screen slot and stacked).

use bevy::prelude::*;
// 0.20: `bevy::ui::Button` is a deprecated alias — use `ui_widgets::Button`.
use bevy::ui_widgets::Button;

use crate::touch::TouchControls;

// ---------------------------------------------------------------------------
// Chrome tokens (Spec 029 §3.2) — the single source of truth for shared
// panel colours. Content modules re-point at these; no module may paste its
// own copy of the panel background again.
// ---------------------------------------------------------------------------

/// The frame/card background (was `PANEL_BG` in inventory + forge).
pub const FRAME_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);
/// Frame / row hairline border.
pub const FRAME_BORDER: Color = Color::srgb(0.35, 0.35, 0.42);
/// Tab-rail strip behind the chips.
pub const RAIL_BG: Color = Color::srgba(0.10, 0.10, 0.15, 0.90);
/// Inactive tab chip background.
pub const CHIP_BG: Color = Color::srgba(0.13, 0.13, 0.18, 0.92);
/// Active tab chip background.
pub const TAB_ACTIVE_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);
/// Inactive tab label colour.
pub const TAB_TEXT: Color = Color::srgb(0.62, 0.70, 0.80);
/// Active / hot tab label colour (the idler gold).
pub const TAB_TEXT_HOT: Color = Color::srgb(1.0, 0.95, 0.6);
/// Panel headings ("Inventory", "Weapon Forge").
pub const HEAD_TEXT: Color = Color::srgb(0.85, 0.90, 1.0);
/// Muted caption / hint text.
pub const BODY_TEXT: Color = Color::srgb(0.6, 0.65, 0.75);
/// Cost, hover and buy accents (was the forge `BUY_BORDER` gold).
pub const ACCENT: Color = Color::srgb(1.0, 0.8, 0.4);
/// Positive / affordable confirmation (was the forge `PROOF_BORDER`).
pub const GOOD: Color = Color::srgb(0.4, 0.95, 0.6);
/// Button / list-row background (was `ROW_BG` in forge + `SLOT_BG` family).
pub const ROW_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.88);
pub const FRAME_RADIUS: f32 = 9.0;
pub const ROW_RADIUS: f32 = 6.0;

/// The one text shadow every panel uses (black, offset (1,1)).
pub fn text_shadow() -> TextShadow {
    TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) }
}

// ---------------------------------------------------------------------------
// State + pure toggle logic (unit-tested)
// ---------------------------------------------------------------------------

/// Which tab the shell currently hosts (default: Inventory).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Inventory,
    Forge,
    Build,
}

/// Single source of truth for the unified panel's open/tab state. The three
/// content modules read (or mirror) this; nothing else owns an overlay flag.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPanel {
    pub open: bool,
    pub tab: Tab,
}

impl CommandPanel {
    /// Is the shell open *on* this tab?
    pub fn is_open(&self, tab: Tab) -> bool {
        self.open && self.tab == tab
    }

    /// Key/press for `tab`: closed → open there; open elsewhere → switch;
    /// open here → close. Exclusive by construction. Returns `true` when the
    /// press moved state (consumed).
    pub fn toggle(&mut self, tab: Tab) -> bool {
        let was = *self;
        if !self.open {
            // Closed: open onto the pressed tab.
            self.open = true;
            self.tab = tab;
        } else if self.tab == tab {
            // Open on this tab: the historic toggle-key close.
            self.open = false;
        } else {
            // Open on another tab: switch, stay open.
            self.tab = tab;
        }
        was.open != self.open || was.tab != self.tab
    }

    /// ESC / ✕ — only closes; never re-opens. `false` when already closed.
    pub fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        true
    }
}

/// Pure key→state resolver used by the input system (spec §3.3 `on_key`).
pub fn on_key(panel: &mut CommandPanel, tab: Tab) -> bool {
    panel.toggle(tab)
}

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

/// The bordered shell frame; hidden whenever [`CommandPanel::open`] is false.
#[derive(Component)]
struct PanelFrame;

/// Content host inside the frame; every tab content re-parents here.
#[derive(Component)]
struct PanelBody;

/// A tab chip in the rail (the Button).
#[derive(Component)]
struct TabChip(pub Tab);

/// Label text of a tab chip.
#[derive(Component)]
struct TabChipLabel(pub Tab);

/// 2 px accent underline shown under the ACTIVE chip only.
#[derive(Component)]
struct TabChipUnderline(pub Tab);

/// The ✕ close chip.
#[derive(Component)]
struct PanelCloseBtn;

/// A content panel adopting the shell: content modules spawn their former
/// panel root with this marker; the shell re-parents it into the body and
/// drives its `Display` per [`CommandPanel`]. (Content is never removed.)
#[derive(Component)]
pub struct TabContent(pub Tab);

/// Font used across the shell (same face as every other panel).
const FONT: &str = "fonts/FiraSans-Bold.ttf";

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandPanel>()
            .add_systems(Startup, spawn_panel_frame)
            .add_systems(
                Update,
                (
                    command_panel_keys,
                    adopt_tab_contents,
                    sync_panel_tabs,
                    update_panel_chrome,
                    panel_button_clicks,
                )
                    .chain(),
            );
    }
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

fn spawn_panel_frame(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load(FONT);

    // Full-screen wrapper does the centering (same trick inventory/forge used).
    let wrapper = commands
        .spawn((Name::new("command-panel-wrapper"), Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Start,
            ..default()
        }))
        .id();

    // The frame: auto width (wraps the widest tab body), rail + body column.
    let frame = commands
        .spawn((
            Name::new("command-panel-frame"),
            PanelFrame,
            Node {
                margin: UiRect::top(Val::Px(90.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(FRAME_RADIUS)),
                display: Display::None,
                overflow: Overflow::clip_y(),
                max_height: Val::Vh(70.0),
                ..default()
            },
            BackgroundColor(FRAME_BG),
            BorderColor::all(FRAME_BORDER),
        ))
        .id();

    // ---- Tab rail --------------------------------------------------------
    let rail = commands
        .spawn((
            Name::new("command-panel-rail"),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(RAIL_BG),
            BorderColor::default(),
        ))
        .id();

    let per_tab: [(Tab, &str, &str); 3] = [
        (Tab::Inventory, "Bag", "Tab"),
        (Tab::Forge, "Forge", "G"),
        (Tab::Build, "Build", "B"),
    ];
    let mut chips = Vec::new();
    for (tab, label, key) in per_tab {
        let chip = commands
            .spawn((
                Name::new(format!("panel-tab-{:?}", tab).to_lowercase()),
                TabChip(tab),
                Button,
                Interaction::default(),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(3.0),
                    height: Val::Px(22.0),
                    padding: UiRect::horizontal(Val::Px(9.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(ROW_RADIUS)),
                    position_type: PositionType::Relative,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(CHIP_BG),
                BorderColor::all(FRAME_BORDER),
            ))
            .id();
        let label_e = commands
            .spawn((
                TabChipLabel(tab),
                Text::new(label),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(TAB_TEXT),
                text_shadow(),
            ))
            .id();
        let key_e = commands
            .spawn((
                Text::new(key),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 9.0.into(), ..default() },
                TextColor(TAB_TEXT),
                text_shadow(),
            ))
            .id();
        // Active-tab accent underline (bottom strip of the chip).
        let underline = commands
            .spawn((
                TabChipUnderline(tab),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Px(6.0),
                    right: Val::Px(6.0),
                    height: Val::Px(2.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(ACCENT),
                Visibility::Hidden,
            ))
            .id();
        commands.entity(chip).add_children(&[label_e, key_e, underline]);
        chips.push(chip);
    }
    let close_btn = commands
        .spawn((
            Name::new("panel-close-chip"),
            PanelCloseBtn,
            Button,
            Interaction::default(),
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::left(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(ROW_RADIUS)),
                ..default()
            },
            BackgroundColor(ROW_BG),
            BorderColor::all(FRAME_BORDER),
            children![(
                Text::new("✕"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 10.0.into(), ..default() },
                TextColor(TAB_TEXT),
                text_shadow(),
            )],
        ))
        .id();

    // ---- Body home for the tab contents ---------------------------------
    let body = commands
        .spawn((
            Name::new("command-panel-body"),
            PanelBody,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
        ))
        .id();

    commands.entity(rail).add_children(&chips);
    commands.entity(rail).add_child(close_btn);
    commands.entity(frame).add_children(&[rail, body]);
    commands.entity(wrapper).add_child(frame);
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// The three historic toggle keys (plus the touch bag button) now feed one
/// shell. ESC closes the shell without touching world semantics.
fn command_panel_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut touch: ResMut<TouchControls>,
    mut panel: ResMut<CommandPanel>,
) {
    if keys.just_pressed(KeyCode::Tab)
        || keys.just_pressed(KeyCode::KeyI)
        || touch.inventory
    {
        // Consumed here — the touch bag button IS a tab press now.
        touch.inventory = false;
        on_key(&mut panel, Tab::Inventory);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        on_key(&mut panel, Tab::Forge);
    }
    if keys.just_pressed(KeyCode::KeyB) {
        on_key(&mut panel, Tab::Build);
    }
    if keys.just_pressed(KeyCode::Escape) {
        panel.close();
    }
}

// ---------------------------------------------------------------------------
// Adoption + visibility
// ---------------------------------------------------------------------------

/// Adopt content panels spawned by their modules: any root [`TabContent`]
/// entity moves under the shell body. Idempotent; keeps scanning until none
/// are left outside the shell.
fn adopt_tab_contents(
    mut commands: Commands,
    frame_body: Query<Entity, With<PanelBody>>,
    strays: Query<(Entity, &TabContent), Without<ChildOf>>,
    mut retry: Local<bool>,
) {
    if *retry {
        return;
    }
    let Ok(body) = frame_body.single() else { return };
    let strays: Vec<Entity> = strays.iter().map(|(e, _)| e).collect();
    if strays.is_empty() {
        // Content modules spawn on Startup — first clean scan wins.
        *retry = true;
        return;
    }
    for entity in strays {
        commands.entity(body).add_child(entity);
    }
}

/// One display authority: the frame shows when open; the tab content shows
/// only for the active tab. Replaces the three per-module toggle hacks.
fn sync_panel_tabs(
    panel: Res<CommandPanel>,
    mut frame: Query<&mut Node, With<PanelFrame>>,
    contents: Query<(&TabContent, &mut Node), Without<PanelFrame>>,
) {
    if let Ok(mut node) = frame.single_mut() {
        let want = if panel.open { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (tab, mut node) in contents {
        let want = if panel.is_open(tab.0) { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

// ---------------------------------------------------------------------------
// Rail buttons + chrome refresh
// ---------------------------------------------------------------------------

/// Chip click: switch/toggle/close like the keyboard; ✕ closes.
fn panel_button_clicks(
    mut panel: ResMut<CommandPanel>,
    chips: Query<(&Interaction, &TabChip), Changed<Interaction>>,
    closes: Query<&Interaction, (With<PanelCloseBtn>, Changed<Interaction>)>,
) {
    for (interaction, chip) in &chips {
        if *interaction == Interaction::Pressed {
            panel.toggle(chip.0);
        }
    }
    if let Ok(i) = closes.single() {
        if *i == Interaction::Pressed {
            panel.close();
        }
    }
}

/// Chip visuals: active chip = hot bg + gold label + accent underline;
/// hovered (inactive) chips get the accent border.
fn update_panel_chrome(
    panel: Res<CommandPanel>,
    chips: Query<(&TabChip, &Interaction, &mut BackgroundColor, &mut BorderColor)>,
    labels: Query<(&TabChipLabel, &mut TextColor)>,
    underlines: Query<(&TabChipUnderline, &mut Visibility)>,
) {
    for (chip, interaction, mut bg, mut border) in chips {
        let active = panel.is_open(chip.0);
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let want_bg = if active { TAB_ACTIVE_BG } else { CHIP_BG };
        if bg.0 != want_bg {
            bg.0 = want_bg;
        }
        let want_border = if active {
            GOOD
        } else if hovered {
            ACCENT
        } else {
            FRAME_BORDER
        };
        if border.top != want_border {
            *border = BorderColor::all(want_border);
        }
    }
    for (label, mut color) in labels {
        let want = if panel.is_open(label.0) { TAB_TEXT_HOT } else { TAB_TEXT };
        if color.0 != want {
            color.0 = want;
        }
    }
    for (underline, mut vis) in underlines {
        let want = if panel.is_open(underline.0) { Visibility::Visible } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (pure toggle logic — spec §5 A1)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn p(open: bool, tab: Tab) -> CommandPanel {
        CommandPanel { open, tab }
    }

    #[test]
    fn default_shell_is_closed_on_inventory() {
        assert_eq!(CommandPanel::default(), p(false, Tab::Inventory));
    }

    #[test]
    fn closed_panel_opens_onto_the_pressed_tab() {
        let mut panel = CommandPanel::default();
        assert!(on_key(&mut panel, Tab::Forge));
        assert_eq!(panel, p(true, Tab::Forge));
    }

    #[test]
    fn pressing_the_active_tab_key_closes() {
        let mut panel = p(true, Tab::Forge);
        on_key(&mut panel, Tab::Forge);
        assert_eq!(panel, p(false, Tab::Forge));
    }

    #[test]
    fn pressing_another_tab_key_switches_without_closing() {
        let mut panel = p(true, Tab::Inventory);
        on_key(&mut panel, Tab::Build);
        assert_eq!(panel, p(true, Tab::Build));
    }

    #[test]
    fn reopening_after_close_restores_the_pressed_tab() {
        let mut panel = p(true, Tab::Build);
        panel.close();
        on_key(&mut panel, Tab::Inventory);
        assert_eq!(panel, p(true, Tab::Inventory));
    }

    #[test]
    fn esc_only_closes_and_never_reopens() {
        let mut panel = p(true, Tab::Forge);
        assert!(panel.close());
        assert_eq!(panel, p(false, Tab::Forge));
        assert!(!panel.close());
        // Closing twice in a row never flips the shell back open.
        assert_eq!(panel, p(false, Tab::Forge));
    }

    #[test]
    fn is_open_requires_both_open_and_tab_match() {
        let panel = p(true, Tab::Inventory);
        assert!(panel.is_open(Tab::Inventory));
        assert!(!panel.is_open(Tab::Forge));
        assert!(!p(false, Tab::Inventory).is_open(Tab::Inventory));
    }
}
