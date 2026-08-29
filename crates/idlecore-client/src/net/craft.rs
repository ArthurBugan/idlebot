//! Craft bench menu (Spec 022 §4): a 2×2 discovery-crafting grid.
//!
//! The player fills the four cells by clicking (each click cycles to the next
//! ingredient they actually carry) and hits Craft. The server matches the
//! multiset against a fixed recipe table — unknown combinations fail with
//! "nothing happened" and recipes are never listed anywhere in the UI.

use bevy::prelude::*;

use crate::inventory::Inventory;
use crate::net::gen::craft_reducer::craft;
use crate::net::plugin::{Net, NetEvent};

/// Ingredients the grid cycles through — mirrors the server's
/// `CRAFT_INGREDIENTS` (types.rs).
pub const INGREDIENTS: [&str; 4] = ["Wood", "Log", "Stone", "Grass"];

const CELL_SIZE: f32 = 52.0;
const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);
const SLOT_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.88);
const SLOT_BORDER: Color = Color::srgb(0.35, 0.35, 0.40);
const CRAFT_BORDER: Color = Color::srgb(1.0, 0.8, 0.4);

/// Craft menu state: open flag, the bench being used, the four grid cells
/// and when the menu was opened (so the opening `E` press can't close it).
#[derive(Resource)]
pub struct CraftMenu {
    pub open: bool,
    /// Bench target: (hex_id, slot_x, slot_y) of the plot holding the bench.
    pub bench: Option<(u64, i32, i32)>,
    /// The four grid cells (item name or empty).
    pub cells: [Option<String>; 4],
    /// `Time.elapsed_secs_f64` at open; close keys are ignored right after.
    pub opened_at: f64,
}

impl Default for CraftMenu {
    fn default() -> Self {
        Self {
            open: false,
            bench: None,
            cells: [None, None, None, None],
            opened_at: f64::NEG_INFINITY,
        }
    }
}

impl CraftMenu {
    /// Open the menu over the bench at the given plot, resetting the grid.
    pub fn open_at(&mut self, hex_id: u64, slot_x: i32, slot_y: i32, now: f64) {
        self.open = true;
        self.bench = Some((hex_id, slot_x, slot_y));
        self.cells = [None, None, None, None];
        self.opened_at = now;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.bench = None;
    }
}

/// Pure grid logic: the next ingredient to put in a cell, cycling through
/// items the player actually carries (inventory-count-limited, Spec 022 §4).
/// `None` when the player carries no craft ingredients at all.
pub fn next_ingredient(
    current: Option<&str>,
    counts: &std::collections::HashMap<String, u64>,
) -> Option<String> {
    let available: Vec<&str> = INGREDIENTS
        .iter()
        .copied()
        .filter(|i| counts.get(*i).copied().unwrap_or(0) > 0)
        .collect();
    let pos = current.and_then(|c| available.iter().position(|a| *a == c));
    match pos {
        None => available.first().map(|s| s.to_string()),
        Some(p) => Some(available[(p + 1) % available.len()].to_string()),
    }
}

/// Pure pre-send validation: every cell filled and every ingredient covered
/// by the player's counts (the server re-validates authoritatively).
pub fn missing_for_craft(
    cells: &[Option<String>; 4],
    counts: &std::collections::HashMap<String, u64>,
) -> Option<String> {
    let mut wanted: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for cell in cells {
        let Some(item) = cell else {
            return Some("Fill all four cells".to_string());
        };
        *wanted.entry(item.as_str()).or_insert(0) += 1;
    }
    for (item, count) in wanted {
        if counts.get(item).copied().unwrap_or(0) < count {
            return Some(format!("Not enough {item}"));
        }
    }
    None
}

// --- UI markers ---

#[derive(Component)]
struct CraftPanel;

#[derive(Component)]
struct CraftCell(usize);

#[derive(Component)]
struct CraftButton;

#[derive(Component)]
struct CraftIcon;

#[derive(Component)]
struct CraftCount;

#[derive(Component)]
struct CraftStatus;

/// Entity of the panel root, filled by [`spawn_craft_panel`].
#[derive(Resource, Default)]
struct PanelRoot(Option<Entity>);

pub struct CraftPlugin;

impl Plugin for CraftPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftMenu>()
            .init_resource::<PanelRoot>()
            .add_systems(Startup, spawn_craft_panel)
            .add_systems(
                Update,
                (
                    update_craft_panel,
                    handle_craft_input,
                    handle_craft_clicks,
                ),
            );
    }
}

fn spawn_craft_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");
    let wrapper = commands
        .spawn((Name::new("craft-wrapper"), Node {
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
            Name::new("craft-panel"),
            CraftPanel,
            Node {
                margin: UiRect::top(Val::Px(90.0)),
                width: Val::Px(2.0 * (CELL_SIZE + 8.0) + 28.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Workbench — Crafting Station"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 14.0.into(), ..default() },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
            ));
            for row in 0..2 {
                panel
                    .spawn((Node {
                        display: Display::Flex,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },))
                    .with_children(|row_node| {
                        for col in 0..2 {
                            let idx = row * 2 + col;
                            row_node
                                .spawn((
                                    Name::new(format!("craft-cell-{idx}")),
                                    CraftCell(idx),
                                    Button,
                                    Node {
                                        width: Val::Px(CELL_SIZE),
                                        height: Val::Px(CELL_SIZE),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(SLOT_BG),
                                    BorderColor::all(SLOT_BORDER),
                                    Interaction::default(),
                                ))
                                .with_children(|cell| {
                                    cell.spawn((
                                        CraftIcon,
                                        Node { width: Val::Percent(78.0), height: Val::Percent(78.0), ..default() },
                                        ImageNode::default(),
                                        Visibility::Hidden,
                                    ));
                                    cell.spawn((
                                        CraftCount,
                                        Text::new(""),
                                        TextFont { font: FontSource::Handle(font.clone()), font_size: 12.0.into(), ..default() },
                                        TextColor(Color::srgb(1.0, 0.95, 0.6)),
                                        TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
                                        Node {
                                            position_type: PositionType::Absolute,
                                            right: Val::Px(3.0),
                                            bottom: Val::Px(1.0),
                                            ..default()
                                        },
                                    ));
                                });
                        }
                    });
            }
            panel
                .spawn((Node {
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Name::new("craft-go"),
                        CraftButton,
                        Button,
                        Node {
                            width: Val::Px(112.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.16, 0.14, 0.10, 0.95)),
                        BorderColor::all(CRAFT_BORDER),
                        Interaction::default(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Craft"),
                            TextFont { font: FontSource::Handle(font.clone()), font_size: 13.0.into(), ..default() },
                            TextColor(Color::srgb(1.0, 0.9, 0.6)),
                        ));
                    });
                });
            panel.spawn((
                CraftStatus,
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(Color::srgb(0.8, 0.85, 0.95)),
            ));
            panel.spawn((
                Text::new("[Esc] close · click a cell to change ingredient"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 10.0.into(), ..default() },
                TextColor(Color::srgb(0.6, 0.65, 0.75)),
            ));
        })
        .id();

    commands.entity(wrapper).add_child(panel_id);
    commands.insert_resource(PanelRoot(Some(panel_id)));
}

/// Toggle visibility and refresh cell icons/counts + status line.
#[allow(clippy::type_complexity)]
fn update_craft_panel(
    menu: Res<CraftMenu>,
    inv: Res<Inventory>,
    icons: Option<Res<crate::inventory::ItemIcons>>,
    panel_root: Res<PanelRoot>,
    net: Res<Net>,
    mut panel_display: Query<&mut Node, With<CraftPanel>>,
    cells_q: Query<(Entity, &CraftCell, &Children)>,
    mut icon_q: Query<(&mut ImageNode, &mut Visibility), With<CraftIcon>>,
    mut count_q: Query<&mut Text, (With<CraftCount>, Without<CraftStatus>)>,
    mut status_q: Query<&mut Text, (With<CraftStatus>, Without<CraftCount>)>,
    mut colors: Query<&mut BackgroundColor, With<Button>>,
) {
    if let Some(entity) = panel_root.0 {
        if let Ok(mut node) = panel_display.get_mut(entity) {
            let want = if menu.open { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }
    if !menu.open {
        return;
    }
    for (entity, cell, children) in &cells_q {
        let item = menu.cells[cell.0].clone();
        let count = item
            .as_ref()
            .and_then(|n| inv.counts.get(n))
            .copied()
            .unwrap_or(0);
        if let Ok(mut bg) = colors.get_mut(entity) {
            let want = if item.is_some() {
                Color::srgba(0.14, 0.16, 0.12, 0.92)
            } else {
                SLOT_BG
            };
            if bg.0 != want {
                bg.0 = want;
            }
        }
        for child in children.iter() {
            if let Ok((mut image, mut vis)) = icon_q.get_mut(child) {
                let want_vis = if item.is_some() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *vis != want_vis {
                    *vis = want_vis;
                }
                if let (Some(name), Some(icons)) = (&item, icons.as_ref()) {
                    if let Some(handle) = icons.by_item.get(name) {
                        if image.image.id() != handle.id() {
                            image.image = handle.clone();
                        }
                    }
                }
                continue;
            }
            if let Ok(mut text) = count_q.get_mut(child) {
                let label = if count > 0 { count.to_string() } else { String::new() };
                if text.0 != label {
                    text.0 = label;
                }
            }
        }
    }
    if let Ok(mut status) = status_q.single_mut() {
        let want = match missing_for_craft(&menu.cells, &inv.counts) {
            Some(m) => m,
            None => match net.log.back() {
                Some(line) if line.starts_with("craft") => line.clone(),
                _ => "Ready — hit Craft".to_string(),
            },
        };
        if status.0 != want {
            status.0 = want;
        }
    }
}

/// `Esc` closes the menu (and `E` toggles it closed via the interaction
/// system). Never closes on the very press that opened it.
fn handle_craft_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut menu: ResMut<CraftMenu>,
) {
    if !menu.open {
        return;
    }
    if time.elapsed_secs_f64() - menu.opened_at < 0.25 {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        menu.close();
    }
}

/// Cell clicks cycle ingredients; the Craft button sends the reducer.
#[allow(clippy::type_complexity)]
fn handle_craft_clicks(
    mut menu: ResMut<CraftMenu>,
    mut net: ResMut<Net>,
    inv: Res<Inventory>,
    cell_q: Query<(&Interaction, &CraftCell), Changed<Interaction>>,
    button_q: Query<&Interaction, (With<CraftButton>, Changed<Interaction>)>,
) {
    if !menu.open {
        return;
    }
    for (interaction, cell) in &cell_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = cell.0;
        let next = next_ingredient(menu.cells[idx].as_deref(), &inv.counts);
        menu.cells[idx] = next;
    }
    for interaction in &button_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some((hex_id, sx, sy)) = menu.bench else {
            continue;
        };
        if let Some(missing) = missing_for_craft(&menu.cells, &inv.counts) {
            net.push(NetEvent::ServerMessage(format!("Craft: {missing}")));
            continue;
        }
        let ingredients: Vec<String> = menu
            .cells
            .iter()
            .map(|c| c.clone().unwrap_or_default())
            .collect();
        let tx = net.sender();
        super::hud::send_reducer(&mut net, |r| {
            r.craft_then(
                hex_id,
                sx,
                sy,
                ingredients,
                super::hud::reducer_report("craft", tx.clone(), hex_id),
            )
        });
        menu.cells = [None, None, None, None];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, u64)]) -> std::collections::HashMap<String, u64> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect()
    }

    #[test]
    fn cycling_starts_at_first_carried_ingredient() {
        let c = counts(&[("Stone", 3), ("Grass", 1)]);
        assert_eq!(next_ingredient(None, &c).as_deref(), Some("Stone"));
        // Wood comes first canonically when carried.
        let c = counts(&[("Wood", 2), ("Stone", 3)]);
        assert_eq!(next_ingredient(None, &c).as_deref(), Some("Wood"));
    }

    #[test]
    fn cycling_wraps_through_carried_items_only() {
        let c = counts(&[("Log", 4), ("Grass", 2)]);
        assert_eq!(next_ingredient(Some("Log"), &c).as_deref(), Some("Grass"));
        assert_eq!(next_ingredient(Some("Grass"), &c).as_deref(), Some("Log"));
        // A single carried ingredient cycles onto itself.
        let c = counts(&[("Stone", 1)]);
        assert_eq!(next_ingredient(Some("Stone"), &c).as_deref(), Some("Stone"));
    }

    #[test]
    fn cycling_skips_items_that_ran_out() {
        let c = counts(&[("Wood", 0), ("Stone", 2)]);
        assert_eq!(next_ingredient(Some("Wood"), &c).as_deref(), Some("Stone"));
        assert_eq!(next_ingredient(None, &c).as_deref(), Some("Stone"));
        let none = counts(&[]);
        assert_eq!(next_ingredient(None, &none), None);
    }

    #[test]
    fn craft_validation_requires_full_cells_and_counts() {
        let c = counts(&[("Wood", 2), ("Stone", 4)]);
        let empty: [Option<String>; 4] = [None, None, None, None];
        assert_eq!(
            missing_for_craft(&empty, &c).as_deref(),
            Some("Fill all four cells")
        );
        let pickaxe: [Option<String>; 4] = [
            Some("Stone".into()),
            Some("Stone".into()),
            Some("Stone".into()),
            Some("Wood".into()),
        ];
        assert_eq!(missing_for_craft(&pickaxe, &c), None);
        // Multiplicity matters: a pickaxe needs 3 stones.
        let c = counts(&[("Stone", 2), ("Wood", 1)]);
        assert_eq!(
            missing_for_craft(&pickaxe, &c).as_deref(),
            Some("Not enough Stone")
        );
    }
}
