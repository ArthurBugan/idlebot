//! Minecraft-style inventory + quick-access bar (hotbar).
//!
//! Items themselves are server-authoritative (`player_item` replication);
//! this module owns their *arrangement*: 36 client-side slots — 27 storage +
//! a 9-slot hotbar mirrored at the bottom of the screen. Controls:
//!   • 1–9 select the active hotbar slot
//!   • Tab opens/closes the inventory panel
//!   • click a cell, then another, to move a stack between cells

use bevy::prelude::*;
use spacetimedb_sdk::Table;
use crate::net::gen::player_item_table::PlayerItemTableAccess;

/// Icon texture per item name, loaded once at startup.
#[derive(Resource, Default)]
pub struct ItemIcons {
    pub by_item: std::collections::HashMap<String, Handle<Image>>,
}

pub fn load_item_icons(
    mut commands: Commands,
    props: Option<Res<crate::world_floor::PropTextures>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(props) = props.as_ref() else { return };
    let mut by_item = std::collections::HashMap::new();
    by_item.insert("Seed".to_string(), props.icon_seed.clone());
    by_item.insert("Wood".to_string(), props.icon_wood.clone());
    by_item.insert("Stone".to_string(), props.icon_stone.clone());
    by_item.insert("Grass".to_string(), props.icon_grass.clone());
    commands.insert_resource(ItemIcons { by_item });
    *done = true;
}

const SLOT_SIZE: f32 = 48.0;
const CELL_SIZE: f32 = 44.0;
const HOTBAR_BASE: usize = 27;
const TOTAL_SLOTS: usize = 36;

const SLOT_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.88);
const SLOT_BORDER: Color = Color::srgb(0.35, 0.35, 0.40);
const ACTIVE_BORDER: Color = Color::srgb(1.0, 0.95, 0.6);
const PICKED_BORDER: Color = Color::srgb(1.0, 0.7, 0.1);
const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);

/// Inventory state: arrangement, active hotbar slot, panel/pick state and
/// live counts mirrored from the `player_item` table.
#[derive(Resource)]
pub struct Inventory {
    /// Slot → item name. Indices 0..27 storage, 27..36 hotbar.
    pub slots: [Option<String>; TOTAL_SLOTS],
    /// Active hotbar index 0..9.
    pub active: usize,
    pub open: bool,
    /// First-clicked cell awaiting the destination (stack moving).
    pub picked: Option<usize>,
    /// Live totals per item from replication.
    pub counts: std::collections::HashMap<String, u64>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            active: 0,
            open: false,
            picked: None,
            counts: std::collections::HashMap::new(),
        }
    }
}

impl Inventory {
    pub fn active_item(&self) -> Option<&String> {
        self.slots[HOTBAR_BASE + self.active].as_ref()
    }
}

/// Visual badge for an item kind: (background tint, glyph).
fn item_badge(item: &str) -> (Color, &'static str) {
    match item {
        "Seed" => (Color::srgb(0.52, 0.78, 0.34), "Se"),
        "Wood" => (Color::srgb(0.72, 0.50, 0.28), "Wo"),
        "Stone" => (Color::srgb(0.68, 0.68, 0.74), "St"),
        "Grass" => (Color::srgb(0.45, 0.72, 0.32), "Gr"),
        _ => (Color::srgb(0.6, 0.6, 0.65), "?"),
    }
}

// --- UI markers ---

#[derive(Component)]
struct HotbarSlot(usize);

#[derive(Component)]
struct InvSlot(usize);

#[derive(Component)]
struct SlotIcon;

#[derive(Component)]
struct SlotCount;

#[derive(Component)]
struct InventoryPanel;

/// Entity of the inventory panel root, filled by [`spawn_inventory_panel`].
#[derive(Resource, Default)]
struct PanelRoot(Option<Entity>);

/// Text above the hotbar flashing the active item's name.
#[derive(Component)]
struct ActiveItemLabel;

/// One-line feedback (action errors/results) above the hotbar — stays
/// visible even when the debug panel is hidden.
#[derive(Component)]
struct ToastText;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .init_resource::<PanelRoot>()
            .add_systems(Startup, (spawn_hotbar, spawn_inventory_panel))
            .add_systems(
                Update,
                (
                    load_item_icons,
                    sync_inventory,
                    update_slot_uis,
                    handle_inventory_input,
                    handle_slot_clicks,
                    update_toast,
                ),
            );
    }
}

// ============================================================================
// Spawning
// ============================================================================

fn slot_bundle(marker: impl Component, size: f32) -> impl Bundle {
    (
        marker,
        Button,
        Node {
            position_type: PositionType::Relative,
            width: Val::Px(size),
            height: Val::Px(size),
            margin: UiRect::horizontal(Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(SLOT_BG),
        BorderColor::all(SLOT_BORDER),
        Interaction::default(),
    )
}

fn spawn_texts(parent: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    parent.spawn((
        Name::new("slot-icon"),
        SlotIcon,
        Node {
            width: Val::Percent(78.0),
            height: Val::Percent(78.0),
            ..default()
        },
        ImageNode::default(),
        Visibility::Hidden,
    ));
    parent.spawn((
        SlotCount,
        Text::new(""),
        TextFont { font: FontSource::Handle(font.clone()), font_size: 13.0.into(), ..default() },
        TextColor(Color::srgb(1.0, 0.95, 0.6)),
        TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(3.0),
            bottom: Val::Px(1.0),
            ..default()
        },
    ));
}

fn spawn_hotbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    commands
        .spawn((
            Name::new("hotbar"),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|bar| {
            bar.spawn((
                ToastText,
                Node { position_type: PositionType::Absolute, bottom: Val::Px(70.0), left: Val::Px(0.0), right: Val::Px(0.0), justify_content: JustifyContent::Center, ..default() },
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 12.0.into(), ..default() },
                TextColor(Color::srgb(1.0, 0.8, 0.5)),
                TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
            ));
            bar.spawn((
                ActiveItemLabel,
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: 12.0.into(),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            bar.spawn((Node {
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                ..default()
            },))
            .with_children(|row| {
                for i in 0..9 {
                    row.spawn(slot_bundle(HotbarSlot(i), SLOT_SIZE))
                        .with_children(|cell| spawn_texts(cell, &font));
                }
            });
        });
}

fn spawn_inventory_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    // Full-screen wrapper does the centering (auto margins are unreliable).
    let wrapper = commands
        .spawn((Name::new("inventory-wrapper"), Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0), right: Val::Px(0.0),
            top: Val::Px(0.0), bottom: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Start,
            ..default()
        },))
        .id();

    let panel_id = commands
        .spawn((
            Name::new("inventory-panel"),
            InventoryPanel,
            Node {
                margin: UiRect::top(Val::Px(90.0)),
                width: Val::Px(9.0 * (CELL_SIZE + 4.0) + 20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Inventory"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 14.0.into(), ..default() },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
                Node { margin: UiRect::bottom(Val::Px(6.0)), ..default() },
            ));
            for row in 0..4 {
                panel
                    .spawn((Node {
                        display: Display::Flex,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(0.0),
                        ..default()
                    },))
                    .with_children(|row_node| {
                        for col in 0..9 {
                            let idx = row * 9 + col;
                            row_node
                                .spawn(slot_bundle(InvSlot(idx), CELL_SIZE))
                                .with_children(|cell| spawn_texts(cell, &font));
                        }
                    });
            }
            panel.spawn((
                Text::new("[Tab] close · click two cells to move a stack"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 10.0.into(), ..default() },
                TextColor(Color::srgb(0.6, 0.65, 0.75)),
                Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
            ));
        })
        .id();

    commands.entity(wrapper).add_child(panel_id);
    commands.insert_resource(PanelRoot(Some(panel_id)));
}

// ============================================================================
// Input
// ============================================================================

fn handle_inventory_input(keys: Res<ButtonInput<KeyCode>>, mut inv: ResMut<Inventory>) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            inv.active = i;
        }
    }
    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyI) {
        inv.open = !inv.open;
        inv.picked = None;
    }
    if keys.just_pressed(KeyCode::Escape) && inv.open {
        inv.open = false;
        inv.picked = None;
    }
}

/// Mirror replicated `player_item` rows into arrangement + live counts.
/// Throttled: the table scan + map rebuilds are not free per-frame work.
fn sync_inventory(
    time: Res<Time>,
    net: Res<crate::net::plugin::Net>,
    mut inv: ResMut<Inventory>,
    mut next_run: Local<f64>,
) {
    if time.elapsed_secs_f64() < *next_run {
        return;
    }
    *next_run = time.elapsed_secs_f64() + 0.25;
    let Some(conn) = net.conn.as_ref() else { return };
    let Some(mine) = net.address.clone() else { return };

    inv.counts.clear();
    for row in conn.db.player_item().iter() {
        if row.player == mine && row.count > 0 {
            inv.counts.insert(row.item.clone(), row.count);
        }
    }

    // Drop items that vanished.
    let counts = inv.counts.clone();
    let mut placed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for slot in inv.slots.iter_mut() {
        match slot {
            Some(name) if counts.contains_key(name) => {
                placed.insert(name.clone());
            }
            _ => *slot = None,
        }
    }

    // Auto-place newly acquired items, preferring the hotbar (like pickup).
    for name in counts.keys() {
        if placed.contains(name) {
            continue;
        }
        let target = (HOTBAR_BASE..TOTAL_SLOTS)
            .chain(0..HOTBAR_BASE)
            .find(|i| inv.slots[*i].is_none());
        if let Some(i) = target {
            inv.slots[i] = Some(name.clone());
        }
    }
}

/// Click a cell: pick up a stack, place/move it, or select a hotbar slot.
fn handle_slot_clicks(
    mut inv: ResMut<Inventory>,
    hotbar_q: Query<(&Interaction, &HotbarSlot), Changed<Interaction>>,
    inv_q: Query<(&Interaction, &InvSlot), Changed<Interaction>>,
) {
    for (interaction, slot) in &hotbar_q {
        if *interaction == Interaction::Pressed {
            inv.active = slot.0;
        }
    }
    if !inv.open {
        return;
    }
    for (interaction, slot) in &inv_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = slot.0;
        match inv.picked {
            None => {
                if inv.slots[idx].is_some() {
                    inv.picked = Some(idx);
                }
            }
            Some(from) => {
                if from == idx {
                    inv.picked = None;
                } else {
                    inv.slots.swap(from, idx);
                    inv.picked = None;
                }
            }
        }
    }
}

// ============================================================================
// Rendering state → UI
// ============================================================================

/// Refresh all visible slot UIs (icon, count, fill tint, borders, panel).
#[allow(clippy::type_complexity)]
fn update_slot_uis(
    inv: Res<Inventory>,
    icons: Option<Res<ItemIcons>>,
    panel_root: Res<PanelRoot>,
    mut panel_display: Query<&mut Node, With<InventoryPanel>>,
    hotbar_q: Query<(Entity, &HotbarSlot, &Children)>,
    inv_q: Query<(Entity, &InvSlot, &Children)>,
    mut icon_q: Query<(&mut ImageNode, &mut Visibility), With<SlotIcon>>,
    mut count_texts: Query<&mut Text, (With<SlotCount>, Without<ActiveItemLabel>)>,
    mut colors: Query<&mut BackgroundColor, With<Button>>,
    mut borders: Query<&mut BorderColor, With<Button>>,
    mut active_label: Query<&mut Text, With<ActiveItemLabel>>,
) {
    if let Some(entity) = panel_root.0 {
        if let Ok(mut node) = panel_display.get_mut(entity) {
            let want = if inv.open { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }

    // Active item name above the hotbar.
    if let Ok(mut label) = active_label.single_mut() {
        let want = inv.active_item().cloned().unwrap_or_default();
        if label.0 != want {
            label.0 = want;
        }
    }

    let write_slot =
        |entity: Entity,
         children: &Children,
         idx: usize,
         colors: &mut Query<&mut BackgroundColor, With<Button>>,
         borders: &mut Query<&mut BorderColor, With<Button>>,
         icons: &Option<Res<ItemIcons>>,
         icon_q: &mut Query<(&mut ImageNode, &mut Visibility), With<SlotIcon>>,
         count_texts: &mut Query<
             &mut Text,
             (With<SlotCount>, Without<ActiveItemLabel>),
         >| {
            let item = inv.slots[idx].clone();
            let count = item
                .as_ref()
                .and_then(|n| inv.counts.get(n))
                .copied()
                .unwrap_or(0);
            let tint = item.as_deref().map(item_badge).map(|(c, _)| c);

            if let Ok(mut bg) = colors.get_mut(entity) {
                let want = match &tint {
                    Some(c) => {
                        let s = c.to_srgba();
                        Color::srgba(s.red * 0.4, s.green * 0.4, s.blue * 0.4, 0.92)
                    }
                    None => SLOT_BG,
                };
                if bg.0 != want {
                    bg.0 = want;
                }
            }
            if let Ok(mut border) = borders.get_mut(entity) {
                let want = if inv.picked == Some(idx) {
                    PICKED_BORDER
                } else if idx >= HOTBAR_BASE && inv.active == idx - HOTBAR_BASE {
                    ACTIVE_BORDER
                } else {
                    SLOT_BORDER
                };
                if border.top != want {
                    *border = BorderColor::all(want);
                }
            }
            for child in children.iter() {
                if let Ok((mut image, mut vis)) = icon_q.get_mut(child) {
                    let want_visible = item.is_some();
                    if *vis != (if want_visible { Visibility::Visible } else { Visibility::Hidden })
                    {
                        *vis = if want_visible {
                            Visibility::Visible
                        } else {
                            Visibility::Hidden
                        };
                    }
                    if let Some(name) = &item {
                        if let Some(icons) = icons.as_ref() {
                            if let Some(handle) = icons.by_item.get(name) {
                                if image.image.id() != handle.id() {
                                    image.image = handle.clone();
                                }
                            }
                        }
                    }
                    continue;
                }
                if let Ok(mut text) = count_texts.get_mut(child) {
                    let label = if count > 0 { count.to_string() } else { String::new() };
                    if text.0 != label {
                        text.0 = label;
                    }
                }
            }
        };

    for (entity, slot, children) in &hotbar_q {
        // Hotbar entities are numbered 0..9 but live in the top band of the
        // arrangement — render from the hotbar slice, not storage.
        write_slot(entity, children, HOTBAR_BASE + slot.0, &mut colors, &mut borders, &icons, &mut icon_q, &mut count_texts);
    }
    for (entity, slot, children) in &inv_q {
        write_slot(entity, children, slot.0, &mut colors, &mut borders, &icons, &mut icon_q, &mut count_texts);
    }
}

/// Surface the newest server/log line above the hotbar for a few seconds,
/// so action results stay visible even with the debug panel hidden.
fn update_toast(
    net: Res<crate::net::plugin::Net>,
    time: Res<Time>,
    mut toast: Query<&mut Text, With<ToastText>>,
    mut shown: Local<(String, f64)>,
) {
    let Ok(mut text) = toast.single_mut() else { return };
    if let Some(latest) = net.log.back() {
        if *latest != shown.0 {
            shown.0 = latest.clone();
            shown.1 = time.elapsed_secs_f64();
        }
    }
    let want = if time.elapsed_secs_f64() - shown.1 < 3.5 {
        shown.0.clone()
    } else {
        String::new()
    };
    if text.0 != want {
        text.0 = want;
    }
}
