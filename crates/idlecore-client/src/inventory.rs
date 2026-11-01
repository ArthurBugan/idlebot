//! Minecraft-style inventory + quick-access bar (hotbar).
//!
//! Items themselves are server-authoritative (`player_item` replication);
//! this module owns their *arrangement*: 36 client-side slots — 27 storage +
//! a 9-slot hotbar mirrored at the bottom of the screen. Controls:
//!   • 1–9 select the active hotbar slot
//!   • Tab opens/closes the inventory panel
//!   • click a cell, then another, to move a stack between cells

use bevy::prelude::*;
// 0.20: the old `bevy::ui::Button` is now a deprecated private type
// alias, so spawn/query the headless `ui_widgets::Button` instead.
use bevy::ui_widgets::Button;
use bevy::input::mouse::MouseButton;
use spacetimedb_sdk::Table;
use crate::net::gen::player_item_table::PlayerItemTableAccess;
use crate::net::gen::player_vehicle_table::PlayerVehicleTableAccess;
use crate::net::gen::defense_structure_table::DefenseStructureTableAccess;
use crate::net::gen::chest_item_table::ChestItemTableAccess;
use crate::net::gen::chest_deposit_reducer::chest_deposit;
use crate::net::gen::chest_withdraw_reducer::chest_withdraw;
use crate::net::hud::{reducer_report, send_reducer};
use crate::net::gen::equip_vehicle_reducer::equip_vehicle;
use crate::net::gen::place_craft_bench_reducer::place_craft_bench;
use crate::net::gen::gather_object_reducer::gather_object;
use crate::net::gen::world_object_table::WorldObjectTableAccess;
use crate::net::plugin::NetEvent;
use crate::player::ClientPlayer;
use crate::plugins::player::PhysicsBody;
use crate::world_floor::ActionTarget;

/// Icon texture per item name, loaded once at startup.
#[derive(Resource, Default)]
pub struct ItemIcons {
    pub by_item: std::collections::HashMap<String, Handle<Image>>,
}

pub fn load_item_icons(
    mut commands: Commands,
    props: Option<Res<crate::world_floor::PropTextures>>,
    td: Option<Res<crate::net::td::TdTextures>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(props) = props.as_ref() else { return };
    if !props.ready {
        return; // icons not built yet (waiting on the sliced atlas)
    }
    // Spec 023: tower-defense icons load through the TD texture set.
    let Some(td) = td.filter(|t| t.ready) else {
        return;
    };
    let mut by_item = std::collections::HashMap::new();
    by_item.insert("Seed".to_string(), props.icon_seed.clone());
    by_item.insert("Wood".to_string(), props.icon_wood.clone());
    by_item.insert("Stone".to_string(), props.icon_stone.clone());
    by_item.insert("Grass".to_string(), props.icon_grass.clone());
    // Spec 022: logs + crafted tools (inventory icons only).
    by_item.insert("Log".to_string(), props.icon_log.clone());
    by_item.insert("Pickaxe".to_string(), props.icon_pickaxe.clone());
    by_item.insert("Axe".to_string(), props.icon_axe.clone());
    by_item.insert("Shovel".to_string(), props.icon_shovel.clone());
    by_item.insert("Hoe".to_string(), props.icon_hoe.clone());
    by_item.insert("WateringCan".to_string(), props.icon_watering_can.clone());
    // Picked-up / placeable craft bench (Spec 022 §3).
    by_item.insert("Workbench".to_string(), props.bench.clone());
    // Tower-defense items (Spec 023 §3.3): capital core, drops, placables.
    by_item.insert("CapitalCore".to_string(), td.icon_core.clone());
    by_item.insert("MetalPart".to_string(), td.icon_part.clone());
    by_item.insert("Bone".to_string(), td.icon_bone.clone());
    by_item.insert("BoneBarricade".to_string(), td.icon_barricade.clone());
    by_item.insert("ArrowTower".to_string(), td.icon_arrow.clone());
    by_item.insert("Ballista".to_string(), td.icon_ballista.clone());
    // Vehicles appear as inventory items (equip/unequip from the panel).
    for v in ["Bicycle", "Car", "Scooter", "Motorcycle", "Boat", "Airplane"] {
        by_item.insert(v.to_string(), props.icon_car.clone());
    }
    commands.insert_resource(ItemIcons { by_item });
    *done = true;
}

const SLOT_SIZE: f32 = 48.0;
/// Full hotbar row width (9 slots + the 2px horizontal margins each).
const HOTBAR_W: f32 = SLOT_SIZE * 9.0 + 4.0 * 9.0;
const CELL_SIZE: f32 = 44.0;
const HOTBAR_BASE: usize = 27;
const TOTAL_SLOTS: usize = 36;

const SLOT_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.88);
const SLOT_BORDER: Color = Color::srgb(0.35, 0.35, 0.40);
const CRAFT_BORDER: Color = Color::srgb(1.0, 0.8, 0.4);
const ACTIVE_BORDER: Color = Color::srgb(1.0, 0.95, 0.6);
const PICKED_BORDER: Color = Color::srgb(1.0, 0.7, 0.1);
const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.95, 1.0);

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
    /// Item selected for click-to-place into the crafting grid (set by
    /// clicking an inventory slot; cleared when placing or selecting empty).
    pub held: Option<String>,
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
            held: None,
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
        "Log" => (Color::srgb(0.55, 0.38, 0.22), "Lo"),
        "Workbench" => (Color::srgb(0.80, 0.62, 0.34), "Wb"),
        "Pickaxe" => (Color::srgb(0.55, 0.62, 0.72), "Pi"),
        "Axe" => (Color::srgb(0.62, 0.55, 0.45), "Ax"),
        "Shovel" => (Color::srgb(0.50, 0.58, 0.62), "Sh"),
        "Hoe" => (Color::srgb(0.58, 0.50, 0.58), "Ho"),
        "WateringCan" => (Color::srgb(0.45, 0.62, 0.85), "Wc"),
        "Bicycle" => (Color::srgb(0.40, 0.44, 0.54), "Ca"),
        "Car" => (Color::srgb(0.40, 0.44, 0.54), "Ca"),
        "Scooter" => (Color::srgb(0.40, 0.44, 0.54), "Sc"),
        "Motorcycle" => (Color::srgb(0.40, 0.44, 0.54), "Mo"),
        "Boat" => (Color::srgb(0.40, 0.44, 0.54), "Bo"),
        "Airplane" => (Color::srgb(0.40, 0.44, 0.54), "Ai"),
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

/// Floating hover tooltip: the hovered slot's item (`name ×count`), parked
/// just off the cursor. One entity for all slots.
#[derive(Component)]
struct SlotTooltip;

/// Entity of the tooltip node, filled by [`spawn_inventory_panel`].
#[derive(Resource, Default)]
struct TooltipRoot(Option<Entity>);

/// The chest the player is standing on (`structure_id`), or None when not
/// near an own StorageChest — drives the inventory panel's chest tab
/// (Spec 024 §2.3).
#[derive(Resource, Default)]
pub struct ChestNear(pub Option<u64>);

#[derive(Component)]
struct ChestTabRoot;

/// The rows-carrying node — `update_chest_tab` rebuilds its children.
#[derive(Component)]
struct ChestRowList;

#[derive(Component)]
struct ChestRow {
    chest_id: u64,
    item: String,
    count: u64,
}

/// "Deposit all" button inside the chest tab → chest_deposit of the
/// active hotbar item's full stack.
#[derive(Component)]
struct ChestDepositBtn;

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

// --- Inventory crafting grid (Minecraft-style workbench recipe) ---

#[derive(Component)]
struct InvCraftCell(usize);

#[derive(Component)]
struct InvCraftResult;

#[derive(Component)]
struct InvCraftResultLabel;

#[derive(Component)]
struct InvCraftStatus;

/// Transient 2x2 crafting grid shown in the inventory: each cell holds the
/// material the player has placed (click to cycle), and a matching layout
/// (4 Logs) yields a Workbench in the result slot.
#[derive(Resource, Default)]
pub struct InventoryCraftGrid {
    pub cells: [Option<String>; 4],
}

/// Drag-and-drop state for moving an inventory item onto the crafting grid.
#[derive(Resource, Default)]
pub struct DragState {
    /// Item currently being dragged (from an inventory slot), if any.
    pub item: Option<String>,
    /// Floating ghost entity following the cursor while dragging.
    pub ghost: Option<Entity>,
}

/// Marker for the floating drag ghost (an item icon that follows the cursor).
#[derive(Component)]
struct DragGhost;

/// Icon shown in the crafting result slot when a Workbench is ready.
#[derive(Component)]
struct InvCraftResultIcon;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .init_resource::<PanelRoot>()
            .init_resource::<TooltipRoot>()
            .init_resource::<ChestNear>()
            .init_resource::<InventoryCraftGrid>()
            .init_resource::<DragState>()
            .add_systems(Startup, (spawn_hotbar, spawn_inventory_panel))
            .add_systems(
                Update,
                (
                    load_item_icons,
                    sync_inventory,
                    update_slot_uis,
                    update_slot_tooltip,
                    update_chest_near,
                    update_chest_tab,
                    chest_row_clicks,
                    chest_deposit_click,
                    handle_inventory_input,
                    handle_slot_clicks,
                    update_drag_ghost,
                    auto_unequip_on_deselect,
                    update_toast,
                    update_inv_craft_ui,
                    handle_inv_craft_clicks,
                    handle_world_click,
                    update_hotbar_xp_bar,
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
            // Minecraft-style XP header row above the bar: green level
            // number centered, small "xp / needed" readout at the right.
            bar.spawn((
                Name::new("hotbar-xp-header"),
                Node {
                    width: Val::Px(HOTBAR_W),
                    height: Val::Px(16.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|header| {
                header.spawn((
                    Text::new("1"),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: 15.0.into(),
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 1.0, 0.2)),
                    TextShadow {
                        color: Color::srgb(0.1, 0.25, 0.05),
                        offset: Vec2::new(1.0, 1.0),
                    },
                    XpLevelText,
                ));
                header.spawn((
                    Text::new("0 / 100"),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: 11.0.into(),
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.95, 0.85)),
                    TextShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(1.0, 1.0),
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(2.0),
                        ..default()
                    },
                    XpNeededText,
                ));
            });
            // Minecraft-style XP bar: thin dark trough with a green fill,
            // hugs the hotbar's full width.
            bar.spawn((
                Name::new("hotbar-xp-bar"),
                Node {
                    width: Val::Px(HOTBAR_W),
                    height: Val::Px(5.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
                BorderColor::all(Color::srgb(0.25, 0.25, 0.3)),
            ))
            .with_children(|trough| {
                trough.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.5, 1.0, 0.2)),
                    XpBarFill,
                ));
            });
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

/// Green level number atop the hotbar XP bar (Minecraft-style).
#[derive(Component)]
struct XpLevelText;

/// Small "xp into level / xp needed" readout at the bar's right end.
#[derive(Component)]
struct XpNeededText;

/// Hotbar XP bar fill (width = XP progress through the current level).
#[derive(Component)]
struct XpBarFill;

/// Refresh the hotbar XP bar from the server-synced player: green level
/// number, `xp / needed` readout, fill fraction across the current level
/// ring. Throttled because both the bar width and the numbers churn the UI.
fn update_hotbar_xp_bar(
    player: Query<&ClientPlayer>,
    time: Res<Time>,
    mut throttle: Local<f32>,
    mut fills: Query<&mut Node, With<XpBarFill>>,
    // Two Text writers — ParamSet avoids the Without dance.
    mut texts: ParamSet<(
        Query<&mut Text, With<XpLevelText>>,
        Query<&mut Text, With<XpNeededText>>,
    )>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.25 {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    *throttle = 0.0;

    let (level, into, needed, frac) = idlecore_core::progression::level_progress(player.xp);
    if let Ok(mut fill) = fills.single_mut() {
        fill.width = Val::Percent(100.0 * frac.clamp(0.0, 1.0));
    }
    if let Ok(mut level_text) = texts.p0().single_mut() {
        level_text.0 = level.to_string();
    }
    if let Ok(mut needed_text) = texts.p1().single_mut() {
        needed_text.0 = format!("{into} / {needed}");
    }
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

            // --- Crafting: Minecraft-style 2x2 grid to build a Workbench
            // (Spec 022 §3: 4 Logs -> bench). Click a box to cycle the
            // material you carry; a matching layout shows the Workbench in
            // the result slot — click it to build (consumes 4 Logs at the
            // aimed empty plot).
            panel.spawn((
                Text::new("Crafting"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 13.0.into(), ..default() },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
                Node { margin: UiRect::top(Val::Px(10.0)), ..default() },
            ));
            panel
                .spawn((
                    Node {
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    // 2x2 input grid (two rows of two cells).
                    row.spawn((
                        Node {
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            ..default()
                        },
                    ))
                    .with_children(|grid| {
                        for r in 0..2 {
                            grid.spawn((
                                Node {
                                    display: Display::Flex,
                                    column_gap: Val::Px(2.0),
                                    ..default()
                                },
                            ))
                            .with_children(|grid_row| {
                                for c in 0..2 {
                                    let idx = r * 2 + c;
                                    grid_row
                                        .spawn(slot_bundle(InvCraftCell(idx), CELL_SIZE))
                                        .with_children(|cell| spawn_texts(cell, &font));
                                }
                            });
                        }
                    });
                    // Arrow.
                    row.spawn((
                        Text::new("→"),
                        TextFont { font: FontSource::Handle(font.clone()), font_size: 18.0.into(), ..default() },
                        TextColor(Color::srgb(0.8, 0.85, 0.95)),
                    ));
                    // Result slot.
                    row.spawn((
                        InvCraftResult,
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
                        BorderColor::all(CRAFT_BORDER),
                        Interaction::default(),
                    ))
                    .with_children(|res| {
                        res.spawn((
                            InvCraftResultLabel,
                            Text::new(""),
                            TextFont { font: FontSource::Handle(font.clone()), font_size: 18.0.into(), ..default() },
                            TextColor(Color::srgb(1.0, 0.8, 0.4)),
                        ));
                        // Workbench icon (hidden until the recipe is ready).
                        res.spawn((
                            InvCraftResultIcon,
                            Node {
                                width: Val::Percent(82.0),
                                height: Val::Percent(82.0),
                                ..default()
                            },
                            ImageNode::default(),
                            Visibility::Hidden,
                        ));
                    });
                });
            panel.spawn((
                InvCraftStatus,
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 10.0.into(), ..default() },
                TextColor(Color::srgb(0.6, 0.65, 0.75)),
                Node { margin: UiRect::top(Val::Px(2.0)), ..default() },
            ));
        })
        .id();

    commands.entity(wrapper).add_child(panel_id);
    commands.insert_resource(PanelRoot(Some(panel_id)));
    // Chest tab (Spec 024 §2.3): hidden until standing on an own chest.
    let chest_id = commands
        .spawn((
            Name::new("chest-tab"),
            ChestTabRoot,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                margin: UiRect::top(Val::Px(6.0)),
                border: UiRect::top(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|tab| {
            tab.spawn((
                Text::new("🛡 CHEST — standing on it"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(Color::srgb(0.85, 0.66, 0.28)),
            ));
            tab.spawn((
                Button,
                Interaction::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(22.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.32, 0.2)),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ChestDepositBtn,
            ))
            .with_child((
                Text::new("Deposit all (active item)"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(TEXT_COLOR),
            ));
            // Chest rows live here — rebuilt by `update_chest_tab`.
            tab.spawn((
                Name::new("chest-rows"),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                ChestRowList,
            ));
        })
        .id();
    commands.entity(panel_id).add_child(chest_id);

    // Floating slot tooltip — top-level so it never inherits panel clipping.
    let tip_id = commands
        .spawn((
            Name::new("slot-tooltip"),
            SlotTooltip,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.95)),
            BorderColor::all(Color::srgb(0.3, 0.5, 0.65)),
            Text::new(""),
            TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
            TextColor(TEXT_COLOR),
            TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
            Visibility::Hidden,
        ))
        .id();
    commands.insert_resource(TooltipRoot(Some(tip_id)));
}

/// Which chest sits on the player's current hex (0.25 s). None when the
/// hex has no own StorageChest — hides the inventory chest tab.
fn update_chest_near(
    net: Res<crate::net::plugin::Net>,
    player: Query<&ClientPlayer>,
    mut chest: ResMut<ChestNear>,
) {
    let mut next = None;
    if let (Some(addr), Ok(p)) = (&net.address, player.single()) {
        if let Some(cur) = &p.current_hex {
            let token = (cur.x, cur.y);
            let guard = net.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                for s in conn.db.defense_structure().iter() {
                    if s.kind != "StorageChest" || s.owner != **addr {
                        continue;
                    }
                    let c = idlecore_core::grid::TileCoord::from_id(s.hex_id);
                    if (c.x, c.y) == token {
                        next = Some(s.structure_id);
                        break;
                    }
                }
            }
        }
    }
    if chest.0 != next {
        chest.0 = next;
    }
}

/// Rebuild the chest rows every 0.25 s while the tab is open: one
/// "Take" button per stored stack (Spec 024 §2.3).
fn update_chest_tab(
    mut commands: Commands,
    net: Res<crate::net::plugin::Net>,
    chest: Res<ChestNear>,
    time: Res<Time>,
    mut throttle: Local<f32>,
    mut tabs: Query<&mut Node, With<ChestTabRoot>>,
    lists: Query<Entity, With<ChestRowList>>,
    existing: Query<Entity, With<ChestRow>>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.25 {
        return;
    }
    *throttle = 0.0;
    let open = chest.0.is_some();
    if let Ok(mut node) = tabs.single_mut() {
        node.display = if open { Display::Flex } else { Display::None };
    }
    // No chest around → the stale rows vanish with the tab.
    for e in &existing {
        commands.entity(e).despawn();
    }
    let Some(chest_id) = chest.0 else { return };
    let guard = net.conn.lock().unwrap();
    let Some(conn) = guard.as_ref() else { return };
    let rows: Vec<(String, u64)> = conn
        .db
        .chest_item()
        .iter()
        .filter(|i| i.structure_id == chest_id)
        .map(|i| (i.item.clone(), i.count))
        .collect();
    drop(guard);
    let Ok(list) = lists.single() else { return };
    for (item, count) in rows {
        let row_entity = commands
            .spawn((
                Button,
                Interaction::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(20.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(SLOT_BG),
                BorderColor::all(SLOT_BORDER),
                ChestRow {
                    chest_id,
                    item: item.clone(),
                    count,
                },
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(format!("{item} ×{count}")),
                    TextFont { font_size: 11.0.into(), ..default() },
                    TextColor(TEXT_COLOR),
                ));
                row.spawn((
                    Text::new("TAKE →"),
                    TextFont { font_size: 10.0.into(), ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.4)),
                ));
            })
            .id();
        commands.entity(list).add_child(row_entity);
    }
}

/// Click a chest row → withdraw the whole stack into the inventory.
fn chest_row_clicks(
    mut net: ResMut<crate::net::plugin::Net>,
    rows: Query<(&Interaction, &ChestRow), Changed<Interaction>>,
) {
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let chest_id = row.chest_id;
        let item = row.item.clone();
        let count = row.count;
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.chest_withdraw_then(
                chest_id,
                item,
                count,
                reducer_report("chest_withdraw", tx.clone(), 0),
            )
        });
    }
}

/// Click "Deposit all" → send the active hotbar item's full stack in.
fn chest_deposit_click(
    mut net: ResMut<crate::net::plugin::Net>,
    inv: Res<Inventory>,
    chest: Res<ChestNear>,
    buttons: Query<(&Interaction, &ChestDepositBtn), Changed<Interaction>>,
) {
    for (interaction, _) in &buttons {
        if *interaction != Interaction::Pressed || chest.0.is_none() {
            continue;
        }
        let Some(item) = inv.active_item().cloned() else {
            let _ = net.sender().send(crate::net::plugin::NetEvent::ServerMessage(
                "Chest: select an item in the hotbar first".to_string(),
            ));
            continue;
        };
        let count = inv.counts.get(item.as_str()).copied().unwrap_or(0);
        if count == 0 {
            continue;
        }
        let chest_id = chest.0.unwrap();
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.chest_deposit_then(
                chest_id,
                item,
                count,
                reducer_report("chest_deposit", tx.clone(), 0),
            )
        });
    }
}

/// (Spec 024 polish: without it you have to click a slot to identify it.)
/// Hover a slot → the tooltip follows the cursor with `name ×count`.
fn update_slot_tooltip(
    inv: Res<Inventory>,
    tooltip: Res<TooltipRoot>,
    mut tip: Query<(&mut Text, &mut Node, &mut Visibility), With<SlotTooltip>>,
    hot_q: Query<(&Interaction, &HotbarSlot)>,
    inv_q: Query<(&Interaction, &InvSlot)>,
    windows: Query<&Window>,
) {
    let Some(entity) = tooltip.0 else { return };
    let Ok((mut text, mut node, mut vis)) = tip.get_mut(entity) else {
        return;
    };
    let hovered = hot_q
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .map(|(_, s)| HOTBAR_BASE + s.0)
        .or_else(|| {
            inv_q
                .iter()
                .find(|(i, _)| **i == Interaction::Hovered)
                .map(|(_, s)| s.0)
        });

    let show = hovered
        .and_then(|idx| inv.slots[idx].as_ref())
        .map(|item| {
            let count = inv.counts.get(item.as_str()).copied().unwrap_or(0);
            format!("{} ×{}", normalize_item_name(item), count)
        });
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    match (show, cursor) {
        (Some(label), Some(cursor)) => {
            if text.0 != label {
                text.0 = label;
            }
            let want = Visibility::Visible;
            if *vis != want {
                *vis = want;
            }
            let left = Val::Px(cursor.x + 14.0);
            let top = Val::Px(cursor.y + 10.0);
            if node.left != left {
                node.left = left;
            }
            if node.top != top {
                node.top = top;
            }
        }
        _ => {
            let want = Visibility::Hidden;
            if *vis != want {
                *vis = want;
            }
        }
    }
}

// ============================================================================
// Input
// ============================================================================

fn handle_inventory_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut inv: ResMut<Inventory>,
    mut touch: ResMut<crate::touch::TouchControls>,
) {
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
    if touch.inventory {
        touch.inventory = false;
        inv.open = !inv.open;
        inv.picked = None;
    }
}

/// True for item names that correspond to a `Vehicle` type (so they can be
/// equipped/unequipped from the inventory panel).
fn is_vehicle_item(name: &str) -> bool {
    matches!(
        name,
        "Bicycle" | "Car" | "Scooter" | "Motorcycle" | "Boat" | "Airplane"
    )
}

/// Display name for an inventory item. The legacy pre-rename vehicle is still
/// stored as "Bicycle" in the DB; surface it as "Car" everywhere it is shown
/// (the server keeps the old rows, per the no-migration decision).
pub fn normalize_item_name<'a>(name: &'a str) -> &'a str {
    match name {
        "Bicycle" => "Car",
        other => other,
    }
}

/// Equip a vehicle item, or unequip it if it is already equipped. Reads the
/// current state from the local `ClientPlayer`.
fn toggle_vehicle_equip(
    name: &str,
    player_q: &Query<&ClientPlayer, With<PhysicsBody>>,
    net: &mut ResMut<crate::net::plugin::Net>,
) {
    if let Ok(player) = player_q.single() {
        let equipped = player
            .owned_vehicle
            .as_ref()
            .map(|v| v.display_name() == name)
            .unwrap_or(false);
        let tx = net.sender();
        let target = if equipped {
            "None".to_string()
        } else {
            name.to_string()
        };
        send_reducer(
            net,
            |r| r.equip_vehicle_then(target, reducer_report("equip_vehicle", tx.clone(), 0)),
        );
    }
}

/// The quick slot is authoritative for the mount state:
///   • moving *onto* a vehicle slot (1–9 or click) mounts it,
///   • moving *off* a vehicle slot dismounts (back to walking).
/// Only acts on the transition, and never toggles an already-mounted vehicle
/// off, so a vehicle equipped via another path (e.g. the HUD button) is left
/// alone until the selection is actually taken off the vehicle slot.
fn auto_unequip_on_deselect(
    inv: Res<Inventory>,
    player_q: Query<&ClientPlayer, With<PhysicsBody>>,
    mut net: ResMut<crate::net::plugin::Net>,
    mut prev_active: Local<usize>,
) {
    let active = inv.active;
    if *prev_active == active {
        return;
    }
    let prev_was_vehicle = inv.slots[HOTBAR_BASE + *prev_active]
        .as_deref()
        .map(is_vehicle_item)
        .unwrap_or(false);
    let curr_item = inv.slots[HOTBAR_BASE + active].clone();
    let curr_is_vehicle = curr_item.as_deref().map(is_vehicle_item).unwrap_or(false);

    if prev_was_vehicle && !curr_is_vehicle {
        // Moved off a vehicle -> dismount.
        let equipped = player_q
            .single()
            .ok()
            .and_then(|p| p.owned_vehicle.as_ref())
            .is_some();
        if equipped {
            let tx = net.sender();
            send_reducer(
                &mut net,
                |r| {
                    r.equip_vehicle_then(
                        "None".to_string(),
                        reducer_report("equip_vehicle", tx.clone(), 0),
                    )
                },
            );
        }
    } else if curr_is_vehicle && !prev_was_vehicle {
        // Moved onto a vehicle -> mount it (unless already mounted).
        let already = player_q
            .single()
            .ok()
            .and_then(|p| p.owned_vehicle.as_ref())
            .map(|v| v.display_name() == curr_item.as_deref().unwrap_or(""))
            .unwrap_or(false);
        if !already {
            if let Some(name) = curr_item {
                let tx = net.sender();
                send_reducer(
                    &mut net,
                    |r| {
                        r.equip_vehicle_then(
                            name,
                            reducer_report("equip_vehicle", tx.clone(), 0),
                        )
                    },
                );
            }
        }
    }
    *prev_active = active;
}

/// True when the bench can be placed: the 2x2 grid holds 4 Logs *and* the
/// player carries >= 4 Logs, or the player already holds a picked-up
/// Workbench item (either path satisfies `place_craft_bench`).
fn inv_craft_ready(grid: &InventoryCraftGrid, counts: &std::collections::HashMap<String, u64>) -> bool {
    let has_bench = counts.get("Workbench").copied().unwrap_or(0) >= 1;
    let logs_ready = grid.cells.iter().all(|c| c.as_deref() == Some("Log"))
        && counts.get("Log").copied().unwrap_or(0) >= 4;
    has_bench || logs_ready
}

/// Refresh the inventory crafting grid: cell icons + result slot + status.
#[allow(clippy::type_complexity)]
fn update_inv_craft_ui(
    inv: Res<Inventory>,
    grid: Res<InventoryCraftGrid>,
    icons: Option<Res<ItemIcons>>,
    cells_q: Query<(Entity, &InvCraftCell, &Children)>,
    mut icon_q: Query<(&mut ImageNode, &mut Visibility), (With<SlotIcon>, Without<InvCraftResultIcon>)>,
    mut count_q: Query<&mut Text, (With<SlotCount>, Without<InvCraftStatus>, Without<InvCraftResultLabel>)>,
    mut result_label: Query<&mut Text, (With<InvCraftResultLabel>, Without<SlotCount>, Without<InvCraftStatus>)>,
    mut result_bg: Query<&mut BackgroundColor, With<InvCraftResult>>,
    mut result_icon: Query<(&mut ImageNode, &mut Visibility), (With<InvCraftResultIcon>, Without<SlotIcon>)>,
    mut status_q: Query<&mut Text, (With<InvCraftStatus>, Without<SlotCount>, Without<InvCraftResultLabel>)>,
) {
    let ready = inv_craft_ready(&grid, &inv.counts);
    for (_, cell, children) in &cells_q {
        let item = grid.cells[cell.0].clone();
        let count = item
            .as_ref()
            .and_then(|n| inv.counts.get(n))
            .copied()
            .unwrap_or(0);
        for child in children.iter() {
            if let Ok((mut image, mut vis)) = icon_q.get_mut(child) {
                let want_visible = item.is_some();
                let want = if want_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
                if let (Some(name), Some(ic)) = (&item, icons.as_ref()) {
                    if let Some(handle) = ic.by_item.get(name) {
                        if image.image.id() != handle.id() {
                            image.image = handle.clone();
                        }
                    }
                }
            } else if let Ok(mut text) = count_q.get_mut(child) {
                let label = if count > 0 { count.to_string() } else { String::new() };
                if text.0 != label {
                    text.0 = label;
                }
            }
        }
    }
    if let Ok(mut label) = result_label.single_mut() {
        // The result slot now shows a Workbench icon when ready; keep the text
        // empty so it doesn't overlap the icon.
        if label.0 != String::new() {
            label.0 = String::new();
        }
    }
    if let Ok((mut image, mut vis)) = result_icon.single_mut() {
        let want = if ready { Visibility::Visible } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if let Some(icons) = icons.as_ref() {
            if let Some(handle) = icons.by_item.get("Workbench") {
                if image.image.id() != handle.id() {
                    image.image = handle.clone();
                }
            }
        }
    }
    if let Ok(mut bg) = result_bg.single_mut() {
        let want = if ready {
            Color::srgba(0.18, 0.16, 0.10, 0.95)
        } else {
            SLOT_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    if let Ok(mut status) = status_q.single_mut() {
        let want = if ready {
            "Ready — click the result (or a Workbench in your bag), then aim at an empty tile".to_string()
        } else {
            "Place 4 Logs in the grid, or carry a Workbench, to build one".to_string()
        };
        if status.0 != want {
            status.0 = want;
        }
    }
}

/// Click a crafting cell to place the held item there (or clear it when
/// nothing is held); click the result to build the workbench at the aimed
/// empty plot (consumes 4 Logs, or a carried Workbench item).
fn handle_inv_craft_clicks(
    mut net: ResMut<crate::net::plugin::Net>,
    mut grid: ResMut<InventoryCraftGrid>,
    mut inv: ResMut<Inventory>,
    target: Res<ActionTarget>,
    cell_q: Query<(&Interaction, &InvCraftCell), Changed<Interaction>>,
    result_q: Query<&Interaction, (With<InvCraftResult>, Changed<Interaction>)>,
) {
    for (interaction, cell) in &cell_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Place the held item (if the player actually carries at least one),
        // otherwise clear the cell.
        match inv.held.clone() {
            Some(name) if inv.counts.get(&name).copied().unwrap_or(0) >= 1 => {
                grid.cells[cell.0] = Some(name);
            }
            _ => {
                grid.cells[cell.0] = None;
            }
        }
    }
    for interaction in &result_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !inv_craft_ready(&grid, &inv.counts) {
            net.push(NetEvent::ServerMessage(
                "Crafting: place 4 Logs in the grid (or carry a Workbench)".to_string(),
            ));
            continue;
        }
        let hex_id = idlecore_core::grid::TileCoord::new(target.x, target.y).to_id();
        let (slot_x, slot_y) = (target.slot_x, target.slot_y);
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.place_craft_bench_then(
                hex_id,
                slot_x,
                slot_y,
                reducer_report("place_craft_bench", tx.clone(), hex_id),
            )
        });
        grid.cells = [None, None, None, None];
        inv.held = None;
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
    let conn_guard = net.conn.lock().unwrap();
    let Some(conn) = conn_guard.as_ref() else { return };
    let Some(mine) = net.address.clone() else { return };

    inv.counts.clear();
    for row in conn.db.player_item().iter() {
        if row.player == mine && row.count > 0 {
            inv.counts.insert(row.item.clone(), row.count);
        }
    }
    // Owned vehicles surface as inventory items too (so a vehicle bought
    // before the add_item change — or without a player_item row — still
    // shows up, can be equipped/unequipped from the panel, etc.).
    for row in conn.db.player_vehicle().iter() {
        if row.player == mine {
            inv.counts.entry(row.vehicle_type.clone()).or_insert(1);
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
/// Clicking an item also selects it as the crafting "brush" (for click-to-
/// place into the 2x2 grid) and begins a drag (for drag-and-drop). Clicking a
/// Workbench in the bag places it at the aimed empty plot.
fn handle_slot_clicks(
    mut inv: ResMut<Inventory>,
    mut net: ResMut<crate::net::plugin::Net>,
    player_q: Query<&ClientPlayer, With<PhysicsBody>>,
    target: Res<ActionTarget>,
    mut drag: ResMut<DragState>,
    mut commands: Commands,
    hotbar_q: Query<(&Interaction, &HotbarSlot), Changed<Interaction>>,
    inv_q: Query<(&Interaction, &InvSlot), Changed<Interaction>>,
) {
    for (interaction, slot) in &hotbar_q {
        if *interaction == Interaction::Pressed {
            // Selecting a slot just changes the active quick slot; mount /
            // dismount is handled by `auto_unequip_on_deselect` based on
            // whether the selected slot holds a vehicle.
            inv.active = slot.0;
        }
    }
    if !inv.open {
        // Drop any in-flight drag if the panel was closed.
        if let Some(g) = drag.ghost.take() {
            commands.entity(g).despawn();
        }
        drag.item = None;
        return;
    }
    for (interaction, slot) in &inv_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = slot.0;
        // Clicking a Workbench in the bag builds it at the aimed plot.
        if let Some(name) = inv.slots[idx].clone() {
            if name == "Workbench" {
                try_place_bench(&mut net, &target);
                inv.picked = None;
                inv.held = None;
                continue;
            }
            if is_vehicle_item(&name) {
                toggle_vehicle_equip(&name, &player_q, &mut net);
                inv.picked = None;
                continue;
            }
        }
        // Select this item as the "brush" for click-to-place into the grid,
        // and begin a drag so the same press can be dropped on a grid cell.
        inv.held = inv.slots[idx].clone();
        if let Some(name) = inv.slots[idx].clone() {
            if drag.item.is_none() {
                drag.item = Some(name.clone());
                let ghost = commands
                    .spawn((
                        DragGhost,
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(CELL_SIZE),
                            height: Val::Px(CELL_SIZE),
                            ..default()
                        },
                        ImageNode::default(),
                        Visibility::Hidden,
                        ZIndex(100),
                    ))
                    .id();
                drag.ghost = Some(ghost);
            }
        } else {
            drag.item = None;
            if let Some(g) = drag.ghost.take() {
                commands.entity(g).despawn();
            }
        }
        // Existing stack-move (click two cells to swap).
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

/// Place a craft bench at the aimed empty plot (consumes a carried Workbench
/// item if present, else 4 Logs on the server). Shared by the inventory
/// Workbench click and the crafting result slot.
fn try_place_bench(net: &mut ResMut<crate::net::plugin::Net>, target: &ActionTarget) {
    let hex_id = idlecore_core::grid::TileCoord::new(target.x, target.y).to_id();
    let (slot_x, slot_y) = (target.slot_x, target.slot_y);
    let tx = net.sender();
    send_reducer(net, |r| {
        r.place_craft_bench_then(
            hex_id,
            slot_x,
            slot_y,
            reducer_report("place_craft_bench", tx.clone(), hex_id),
        )
    });
}

/// Drag-and-drop: float a ghost icon at the cursor while dragging an item
/// from the inventory, and drop it into a hovered crafting cell on release.
fn update_drag_ghost(
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<DragState>,
    mut commands: Commands,
    mut ghost_q: Query<(&mut Node, &mut ImageNode, &mut Visibility), With<DragGhost>>,
    icons: Option<Res<ItemIcons>>,
    craft_cells: Query<(Entity, &InvCraftCell, &Interaction)>,
    mut grid: ResMut<InventoryCraftGrid>,
    inv: Res<Inventory>,
) {
    let Some(item) = drag.item.clone() else {
        return;
    };
    // Follow the cursor with the ghost icon.
    if let Ok(window) = windows.single() {
        if let Some(cursor) = window.cursor_position() {
            if let Some(g) = drag.ghost {
                if let Ok((mut node, mut image, mut vis)) = ghost_q.get_mut(g) {
                    node.left = Val::Px(cursor.x - CELL_SIZE / 2.0);
                    node.top = Val::Px(cursor.y - CELL_SIZE / 2.0);
                    if let Some(icons) = icons.as_ref() {
                        if let Some(h) = icons.by_item.get(&item) {
                            if image.image.id() != h.id() {
                                image.image = h.clone();
                            }
                        }
                    }
                    *vis = Visibility::Visible;
                }
            }
        }
    }
    // Drop on mouse release.
    if mouse.just_released(MouseButton::Left) {
        if let Some((_, cell, _interaction)) =
            craft_cells.iter().find(|(_, _, i)| matches!(i, Interaction::Hovered))
        {
            if inv.counts.get(&item).copied().unwrap_or(0) >= 1 {
                grid.cells[cell.0] = Some(item.clone());
            }
        }
        if let Some(g) = drag.ghost.take() {
            commands.entity(g).despawn();
        }
        drag.item = None;
    }
}

/// Left-click a placed craft bench in the world to pick it up into the
/// inventory (grants a Workbench item via `gather_object`). Mirrors the
/// targeting logic of `interact_key_press` but acts only on benches.
fn handle_world_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut net: Option<ResMut<crate::net::plugin::Net>>,
    inv: Res<Inventory>,
    target: Res<ActionTarget>,
    widgets: Query<&Interaction>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(net) = net.as_deref_mut() else { return };
    if net.address.is_none() || inv.open {
        return;
    }
    // Ignore clicks that land on a UI widget (panels, buttons, touch controls).
    if widgets
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed))
    {
        return;
    }

    let hex_id = idlecore_core::grid::TileCoord::new(target.x, target.y).to_id();
    let (hex_cx, hex_cy) = idlecore_core::grid::WorldGrid::tile_center(
        target.x,
        target.y,
        idlecore_core::world_gen::WorldGenConfig::TILE_SIZE,
    );
    let slot_dx = idlecore_core::slots::slot_center(target.slot_x, target.slot_y).0 - hex_cx;
    let slot_dy = idlecore_core::slots::slot_center(target.slot_x, target.slot_y).1 - hex_cy;
    let half_slot = idlecore_core::slots::SLOT_SIZE * 0.5;

    let object_id = {
        let guard = net.conn.lock().unwrap();
        let Some(conn) = guard.as_ref() else {
            return;
        };
        let mut found: Option<u64> = None;
        for obj in spacetimedb_sdk::__codegen::TableLike::iter(&conn.db.world_object())
            .filter(|o| o.hex_id == hex_id)
        {
            let dx = obj.offset_x - slot_dx;
            let dy = obj.offset_y - slot_dy;
            if dx * dx + dy * dy < half_slot * half_slot && obj.kind == "CraftBench" {
                found = Some(obj.object_id);
                break;
            }
        }
        found
    };
    if let Some(id) = object_id {
        let tx = net.sender();
        send_reducer(net, |r| {
            r.gather_object_then(id, reducer_report("pickup_bench", tx.clone(), hex_id))
        });
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
        let raw = inv.active_item().cloned().unwrap_or_default();
        let want = normalize_item_name(&raw).to_string();
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
                        // Grass: a classic-meadow green stand, matching the
                        // world-floor grass tint rather than golden wheat.
                        let want_color = if name == "Grass" {
                            Color::srgb(0.42, 0.72, 0.38)
                        } else {
                            Color::WHITE
                        };
                        if image.color != want_color {
                            image.color = want_color;
                        }
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
