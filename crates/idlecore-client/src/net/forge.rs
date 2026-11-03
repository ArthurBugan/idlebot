//! Weapon forge panel (Spec 027 §14 + Spec 033): arms and armors the player.
//!
//! A panel listing every tier from the core weapon matrix (7 kinds × 10
//! materials) plus every armor piece — stat lines, one-time cost lines,
//! owned/equipped/affordable state — plus an Arrow batch-craft row. Toggled
//! with **G**; buttons send `equip_weapon` / `craft_armor` / `craft_arrows`.
//! Material state mirrors the `Inventory` resource the same way the
//! craft-bench menu does; the server re-validates authoritatively.

use bevy::prelude::*;
// 0.20: `bevy::ui::Button` is a deprecated alias — use `ui_widgets::Button`.
use bevy::ui_widgets::Button;
use idlecore_core::player_combat::{
    armors, weapons, ArmorDef, Weapon, ARROW_BATCH_CAP, ARROW_COST,
};

use crate::inventory::Inventory;
use crate::net::gen::craft_arrows_reducer::craft_arrows;
use crate::net::gen::craft_armor_reducer::craft_armor;
use crate::net::gen::equip_weapon_reducer::equip_weapon;
use crate::net::plugin::Net;
use crate::ui::panel::{self, CommandPanel, Tab, TabContent};

/// Arrows crafted per forge-button press.
pub const FORGE_ARROW_BATCH: u32 = 10;

// Spec 029 §3.2: shared chrome tokens live in `ui::panel` — re-point for
// local readability instead of redefining.
const ROW_BG: Color = panel::ROW_BG;
const ROW_BORDER: Color = panel::FRAME_BORDER;
const BUY_BORDER: Color = panel::ACCENT;
const PROOF_BORDER: Color = panel::GOOD;
// Forge-specific row-state tints (owned by this module).
const ROW_OWNED: Color = Color::srgba(0.12, 0.17, 0.14, 0.92);
const ROW_EQUIPPED: Color = Color::srgba(0.17, 0.17, 0.09, 0.95);
const ROW_UNAFFORDABLE: Color = Color::srgba(0.12, 0.09, 0.10, 0.9);

// --- pure helpers (unit-tested) -------------------------------------------
/// "dmg 12 · 2s cd · 3 hex" + an ammo reminder for bows.
pub fn stat_line(w: &Weapon) -> String {
    let mut s = format!("dmg {} · {}s cd · {} hex", w.dmg, w.cd_secs, w.range_hexes);
    if let Some(ammo) = w.ammo {
        s.push_str(&format!(" · 1 {ammo}/shot"));
    }
    s
}

/// "6 Wood" / "4 Bone + 2 Wood" — the purchase multiset in display order.
pub fn cost_text(cost: &[(&'static str, u32)]) -> String {
    cost.iter()
        .map(|(item, qty)| format!("{qty} {item}"))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Can the stacks cover the whole purchase? (Server re-checks.)
pub fn affordable(
    cost: &[(&'static str, u32)],
    counts: &std::collections::HashMap<String, u64>,
) -> bool {
    cost.iter()
        .all(|(item, qty)| counts.get(*item).copied().unwrap_or(0) >= u64::from(*qty))
}

/// Max Arrows craftable right now, bounded by `ARROW_BATCH_CAP`; 0 when no
/// full Arrow material set exists.
pub fn arrows_affordable(counts: &std::collections::HashMap<String, u64>) -> u32 {
    let mut bound = u32::MAX;
    for (item, qty) in ARROW_COST {
        if *qty == 0 {
            continue;
        }
        let n = (counts.get(*item).copied().unwrap_or(0) / u64::from(*qty)) as u32;
        bound = bound.min(n);
    }
    if bound == u32::MAX {
        return 0;
    }
    bound.min(ARROW_BATCH_CAP)
}

/// Row state → background + button label (pure, pinned by tests).
#[derive(Debug, PartialEq, Eq)]
pub enum RowState {
    /// Not owned and unaffordable.
    Locked,
    /// Not owned, stacks cover it.
    BuyNow,
    /// Owned, not equipped.
    Owned,
    /// Currently equipped.
    Equipped,
}

pub fn row_state_from(owned: bool, equipped: bool, costs_affordable: bool) -> RowState {
    if equipped {
        RowState::Equipped
    } else if owned {
        RowState::Owned
    } else if costs_affordable {
        RowState::BuyNow
    } else {
        RowState::Locked
    }
}

pub fn row_bg(state: &RowState) -> Color {
    match state {
        RowState::Locked => ROW_UNAFFORDABLE,
        RowState::BuyNow | RowState::Owned => ROW_OWNED,
        RowState::Equipped => ROW_EQUIPPED,
    }
}

pub fn row_btn_text(state: &RowState, tier: u8, is_armor: bool) -> String {
    match state {
        RowState::Equipped => "*".to_string(),
        RowState::Owned => if is_armor { "*".to_string() } else { "Equip".to_string() },
        RowState::BuyNow | RowState::Locked => {
            if tier == 0 {
                "Equip".to_string()
            } else if is_armor {
                "Craft".to_string()
            } else {
                "Buy".to_string()
            }
        }
    }
}

// --- UI markers ------------------------------------------------------------
//
// Row text markers are FLAT (no child-walk needed): every button label gets
// its own marker naming the row, so the update system queries them directly.
// Weapon rows wear ids 0..70 (0 = Fists); armor rows wear 200 + armor id.

#[derive(Component)]
struct ForgeRow(u8);

#[derive(Component)]
struct ForgeBuy(u8);

#[derive(Component)]
struct ForgeArrowRow;

#[derive(Component)]
struct ForgeArrowBtn;

/// Button label inside weapon row `tier`.
#[derive(Component)]
struct ForgeRowLabel(u8);

/// The arrow row's button label.
#[derive(Component)]
struct ForgeArrowLabel;

#[derive(Component)]
struct ForgeStatus;

/// Scroll container for the (potentially 111-row) forge list.
#[derive(Component)]
struct ForgeList;

pub struct ForgePlugin;

impl Plugin for ForgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_forge_panel).add_systems(
            Update,
            (update_forge_panel, handle_forge_clicks, forge_wheel_scroll),
        );
    }
}

/// Armor rows wear `200 + armor_id`; weapons stay `weapon_id`.
const ARMOR_ROW_BASE: u8 = 200;

fn forge_button(
    tier_btn_marker: ForgeBuy,
    label_marker: ForgeRowLabel,
    font: &Handle<Font>,
    initial: &str,
) -> impl Bundle {
    let initial = initial.to_string();
    (
        tier_btn_marker,
        Button,
        Interaction::default(),
        Node {
            width: Val::Px(64.0),
            height: Val::Px(20.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.16, 0.14, 0.10, 0.95)),
        BorderColor::all(BUY_BORDER),
        children![(
            label_marker,
            Text::new(initial),
            TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
            TextColor(Color::srgb(1.0, 0.9, 0.6)),
        )],
    )
}

fn row_shard(font: &Handle<Font>, label: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(label),
        TextFont { font: FontSource::Handle(font.clone()), font_size: size.into(), ..default() },
        TextColor(color),
    )
}

/// Section header inside the scroll list (pure text separator).
fn section_shard(font: &Handle<Font>, label: &str) -> impl Bundle {
    (
        Text::new(label),
        TextFont { font: FontSource::Handle(font.clone()), font_size: 12.0.into(), ..default() },
        TextColor(crate::ui::panel::HEAD_TEXT),
    )
}

fn spawn_item_row(
    _commands: &mut Commands,
    _font: &Handle<Font>,
) {}

fn spawn_forge_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");

    // Spec 029: the forge is the Forge tab of the shared command panel —
    // no own wrapper/root/panel `Display` logic; visibility and the frame
    // chrome are owned by `ui::panel`.
    let panel_id = commands
        .spawn((
            Name::new("forge-tab"),
            TabContent(Tab::Forge),
            Node {
                width: Val::Px(340.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();

    // Spec 033: 111 rows no longer fit — the list lives in a scroll
    // container (OverflowAxis::Scroll + ScrollPosition; the wheel reader
    // `forge_wheel_scroll` moves it while the Forge tab is open).
    let list_id = commands
        .spawn((
            Name::new("forge-list"),
            ForgeList,
            Node {
                height: Val::Px(430.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                flex_wrap: FlexWrap::NoWrap,
                overflow: Overflow {
                    x: OverflowAxis::Visible,
                    y: OverflowAxis::Scroll,
                },
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    commands.entity(panel_id).add_child(list_id);

    // One row per weapon tier, data-driven off the core matrix.
    for w in weapons().iter() {
        let row = commands
            .spawn((
                Name::new(format!("forge-row-{}", w.id)),
                ForgeRow(w.id),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
            ))
            .id();
        let head = commands
            .spawn((Node {
                display: Display::Flex,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },))
            .id();
        let (name_shard, stat_shard, cost_shard) = (
            commands
                .spawn(row_shard(&font, &w.name, 13.0, Color::srgb(0.95, 0.95, 1.0)))
                .id(),
            commands
                .spawn(row_shard(
                    &font,
                    &stat_line(w),
                    10.0,
                    Color::srgb(0.7, 0.75, 0.85),
                ))
                .id(),
            commands
                .spawn(row_shard(
                    &font,
                    &cost_text(&w.cost),
                    10.0,
                    Color::srgb(1.0, 0.95, 0.6),
                ))
                .id(),
        );
        let (btn, label) = (ForgeBuy(w.id), ForgeRowLabel(w.id));
        let buy = commands
            .spawn(forge_button(btn, label, &font, "Buy"))
            .id();
        commands.entity(head).add_children(&[name_shard, buy]);
        commands.entity(row).add_children(&[head, stat_shard, cost_shard]);
        commands.entity(list_id).add_child(row);
    }

    // Armor section (Spec 033): owning a piece wears it.
    let armor_head = commands.spawn(section_shard(&font, "⛨ Armor — owning a piece wears it")).id();
    commands.entity(list_id).add_child(armor_head);
    for a in armors().iter() {
        let row_id = ARMOR_ROW_BASE + a.id;
        let row = commands
            .spawn((
                Name::new(format!("forge-armor-{}", a.id)),
                ForgeRow(row_id),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
            ))
            .id();
        let head = commands
            .spawn((Node {
                display: Display::Flex,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },))
            .id();
        let (name_shard, stat_shard, cost_shard) = (
            commands
                .spawn(row_shard(&font, &a.name, 13.0, Color::srgb(0.85, 0.95, 1.0)))
                .id(),
            commands
                .spawn(row_shard(
                    &font,
                    &format!("-{} dmg", a.def),
                    10.0,
                    Color::srgb(0.7, 0.75, 0.85),
                ))
                .id(),
            commands
                .spawn(row_shard(
                    &font,
                    &cost_text(&a.cost),
                    10.0,
                    Color::srgb(1.0, 0.95, 0.6),
                ))
                .id(),
        );
        let (btn, label) = (ForgeBuy(row_id), ForgeRowLabel(row_id));
        let buy = commands
            .spawn(forge_button(btn, label, &font, "Craft"))
            .id();
        commands.entity(head).add_children(&[name_shard, buy]);
        commands.entity(row).add_children(&[head, stat_shard, cost_shard]);
        commands.entity(list_id).add_child(row);
    }

    // Arrow batch row (marker `ForgeBuy(u8::MAX)` cases the border tint).
    let arrow_row = commands
        .spawn((
            Name::new("forge-arrow-row"),
            ForgeArrowRow,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ROW_BG),
            BorderColor::all(ROW_BORDER),
        ))
        .id();
    let arrow_head = commands
        .spawn((Node {
            display: Display::Flex,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .id();
    let (arrow_name, arrow_cost, arrow_note) = (
        commands
            .spawn(row_shard(&font, "Arrow", 13.0, Color::srgb(0.95, 0.95, 1.0)))
            .id(),
        commands
            .spawn(row_shard(
                &font,
                &format!("×1 = {}", cost_text(ARROW_COST)),
                10.0,
                Color::srgb(1.0, 0.95, 0.6),
            ))
            .id(),
        commands
            .spawn(row_shard(
                &font,
                "Bow ammunition — every bow shot spends one.",
                10.0,
                Color::srgb(0.7, 0.75, 0.85),
            ))
            .id(),
    );
    let arrow_btn = commands
        .spawn((
            ForgeArrowBtn,
            Button,
            Interaction::default(),
            Node {
                width: Val::Px(64.0),
                height: Val::Px(20.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.16, 0.14, 0.10, 0.95)),
            BorderColor::all(PROOF_BORDER),
            children![(
                ForgeArrowLabel,
                Text::new(format!("Craft ×{FORGE_ARROW_BATCH}")),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(Color::srgb(0.7, 1.0, 0.8)),
            )],
        ))
        .id();
    commands
        .entity(arrow_head)
        .add_children(&[arrow_name, arrow_btn]);
    commands
        .entity(arrow_row)
        .add_children(&[arrow_head, arrow_cost, arrow_note]);
    commands.entity(list_id).add_child(arrow_row);

    let (status, hint) = (
        commands
            .spawn((
                ForgeStatus,
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: 11.0.into(), ..default() },
                TextColor(Color::srgb(0.8, 0.85, 0.95)),
            ))
            .id(),
        commands
            .spawn(row_shard(
                &font,
                "[G] toggle · [Esc] close · prices are paid from your stacks",
                10.0,
                crate::ui::panel::BODY_TEXT,
            ))
            .id(),
    );
    commands.entity(panel_id).add_children(&[status, hint]);
}

/// Refresh the weapon + armor rows and the arrow row (visibility is the
/// shell's job — `ui::panel` drives tab `Display`).
#[allow(clippy::type_complexity)]
fn update_forge_panel(
    panel: Res<CommandPanel>,
    player: Query<&crate::player::ClientPlayer>,
    inv: Res<Inventory>,
    net: Res<Net>,
    mut rows_node: Query<(Entity, &ForgeRow, &mut BackgroundColor), Without<ForgeArrowRow>>,
    mut arrow_bg: Query<&mut BackgroundColor, With<ForgeArrowRow>>,
    mut labels: Query<(&ForgeRowLabel, &mut Text)>,
    mut arrow_label: Query<&mut Text, (With<ForgeArrowLabel>, Without<ForgeRowLabel>)>,
    mut status: Query<&mut Text, (With<ForgeStatus>, Without<ForgeRowLabel>, Without<ForgeArrowLabel>)>,
) {
    if !panel.is_open(Tab::Forge) {
        return;
    }
    let Ok(p) = player.single() else { return };
    for (_e, row, mut bg) in &mut rows_node {
        let row_id = row.0;
        let is_armor = row_id >= ARMOR_ROW_BASE;
        let (name, owned, affordable_here) = if is_armor {
            match armors().get((row_id - ARMOR_ROW_BASE) as usize) {
                Some(a) => (
                    a.name.clone(),
                    inv.counts.get(&a.name).copied().unwrap_or(0) > 0,
                    affordable(&a.cost, &inv.counts),
                ),
                None => continue,
            }
        } else {
            match weapons().iter().find(|w| w.id == row_id) {
                Some(w) => (
                    w.name.clone(),
                    inv.counts.get(&w.name).copied().unwrap_or(0) > 0,
                    affordable(&w.cost, &inv.counts),
                ),
                None => continue,
            }
        };
        let state = if is_armor {
            row_state_from(owned, false, affordable_here)
        } else {
            row_state_from(
                owned || row_id == 0,
                p.weapon == row_id,
                affordable_here,
            )
        };
        let want = row_bg(&state);
        if bg.0 != want {
            bg.0 = want;
        }
        let want_text = row_btn_text(&state, row_id, is_armor);
        let _ = name; // owned state reads the stacks directly
        if let Some((_, mut text)) = labels.iter_mut().find(|(l, _)| l.0 == row_id) {
            if text.0 != want_text {
                text.0 = want_text;
            }
        }
    }
    if let Ok(mut bg) = arrow_bg.single_mut() {
        let want = if arrows_affordable(&inv.counts) > 0 { ROW_BG } else { ROW_UNAFFORDABLE };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    if let Ok(mut text) = arrow_label.single_mut() {
        let craftable = arrows_affordable(&inv.counts);
        let want = if craftable >= FORGE_ARROW_BATCH {
            format!("Craft ×{FORGE_ARROW_BATCH}")
        } else if craftable > 0 {
            format!("Craft ×{craftable}")
        } else {
            "No materials".to_string()
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    if let Ok(mut s) = status.single_mut() {
        let want = match net.log.back() {
            Some(line) if line.starts_with("equip") || line.starts_with("craft") => line.clone(),
            _ => {
                let def: u32 = armors()
                    .iter()
                    .filter(|a| inv.counts.get(&a.name).copied().unwrap_or(0) > 0)
                    .map(|a| a.def)
                    .sum();
                match p.weapon_def().ammo {
                    Some(ammo) => format!(
                        "{} equipped · ⛨{def} — {} {} in stack",
                        p.weapon_def().name,
                        inv.counts.get(ammo).copied().unwrap_or(0),
                        ammo
                    ),
                    None => format!("{} equipped · ⛨{def}", p.weapon_def().name),
                }
            }
        };
        if s.0 != want {
            s.0 = want;
        }
    }
}

/// Wheel scrolls the forge list while the tab is open (Bevy 0.20 scroll
/// containers read `ScrollPosition`; there is no automatic wheel bridge).
fn forge_wheel_scroll(
    panel: Res<CommandPanel>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    mut list: Query<&mut ScrollPosition, With<ForgeList>>,
) {
    if !panel.is_open(Tab::Forge) {
        scroll.clear();
        return;
    }
    let Ok(mut pos) = list.single_mut() else { scroll.clear(); return };
    for ev in scroll.read() {
        pos.y -= ev.y * 24.0;
    }
}

fn handle_forge_clicks(
    panel: Res<CommandPanel>,
    mut net: ResMut<Net>,
    buy_q: Query<(&Interaction, &ForgeBuy), Changed<Interaction>>,
    arrow_q: Query<&Interaction, (With<ForgeArrowBtn>, Changed<Interaction>)>,
) {
    if !panel.is_open(Tab::Forge) {
        return;
    }
    for (interaction, buy) in &buy_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = buy.0;
        if id >= ARMOR_ROW_BASE {
            // Armor row: `200 + armor_id` → `craft_armor`.
            let armor_id = id - ARMOR_ROW_BASE;
            let tx = net.sender();
            super::hud::send_reducer(&mut net, |r| {
                r.craft_armor_then(
                    armor_id,
                    super::hud::reducer_report("craft_armor", tx.clone(), 0),
                )
            });
        } else {
            let tx = net.sender();
            super::hud::send_reducer(&mut net, |r| {
                r.equip_weapon_then(id, super::hud::reducer_report("equip", tx.clone(), 0))
            });
        }
    }
    for interaction in &arrow_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let tx = net.sender();
        super::hud::send_reducer(&mut net, |r| {
            r.craft_arrows_then(
                FORGE_ARROW_BATCH,
                super::hud::reducer_report("craft", tx.clone(), 0),
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, u64)]) -> std::collections::HashMap<String, u64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn weapon_by_name(name: &str) -> &'static Weapon {
        weapons()
            .iter()
            .find(|w| w.name == name)
            .unwrap_or_else(|| panic!("weapon {name} missing"))
    }

    #[test]
    fn stat_lines_keep_the_sheet_honest() {
        assert_eq!(
            stat_line(&weapon_by_name("Wood Sword")),
            "dmg 8 · 1s cd · 1 hex"
        );
        assert_eq!(
            stat_line(&weapon_by_name("Obsidian Bow")),
            "dmg 24 · 2s cd · 3 hex · 1 Arrow/shot"
        );
    }

    #[test]
    fn cost_lines_join_in_display_order() {
        assert_eq!(cost_text(&[]), "");
        // REV drop economy: mats past Wood are paid in raid drops.
        assert_eq!(
            cost_text(&weapon_by_name("Gold Sword").cost),
            "4 MetalPart + 3 Bone + 6 SlimeGel"
        );
        assert_eq!(
            cost_text(&weapon_by_name("Obsidian Bow").cost),
            "10 MetalPart + 9 Bone + 18 SlimeGel + 10 Grass"
        );
    }

    #[test]
    fn affordability_needs_every_ingredient() {
        let c = counts(&[("Wood", 2), ("Bone", 2), ("MetalPart", 1), ("Grass", 1)]);
        assert!(affordable(&weapon_by_name("Wood Bow").cost, &c));
        let wood_only = counts(&[("Wood", 9)]);
        assert!(
            !affordable(&weapon_by_name("Copper Axe").cost, &wood_only),
            "mats need drops too"
        );
        let metal_only = counts(&[("MetalPart", 9)]);
        assert!(
            !affordable(&weapon_by_name("Copper Axe").cost, &metal_only),
            "needs Bone + SlimeGel too"
        );
        assert!(affordable(&weapon_by_name("Fists").cost, &counts(&[])), "fists are free");
    }

    #[test]
    fn arrow_batches_bounded_by_cap_and_stacks() {
        let rich = counts(&[("Wood", 100), ("Stone", 100)]);
        assert_eq!(arrows_affordable(&rich), ARROW_BATCH_CAP, "folded down to cap");
        let tight = counts(&[("Wood", 7), ("Stone", 2)]);
        assert_eq!(arrows_affordable(&tight), 2, "Stone scarcity binds");
        let none = counts(&[("Wood", 0), ("Stone", 0)]);
        assert_eq!(arrows_affordable(&none), 0);
    }

    #[test]
    fn row_states_read_owned_equipped_and_affordability() {
        // The matrix reads ownership straight from the stacks — no bitmask.
        assert_eq!(row_state_from(true, false, false), RowState::Owned);
        assert_eq!(row_state_from(true, true, false), RowState::Equipped);
        assert_eq!(row_state_from(false, true, false), RowState::Equipped);
        assert_eq!(row_state_from(false, false, true), RowState::BuyNow);
        assert_eq!(row_state_from(false, false, false), RowState::Locked);
    }

    #[test]
    fn row_button_labels_follow_the_state() {
        assert_eq!(row_btn_text(&RowState::Equipped, 1, false), "*");
        assert_eq!(row_btn_text(&RowState::Owned, 1, false), "Equip");
        assert_eq!(row_btn_text(&RowState::BuyNow, 1, false), "Buy");
        assert_eq!(row_btn_text(&RowState::Locked, 1, false), "Buy");
        assert_eq!(row_btn_text(&RowState::Locked, 0, false), "Equip");
        // Armor never equips — owned armor is worn armor.
        assert_eq!(row_btn_text(&RowState::Owned, ARMOR_ROW_BASE, true), "*");
        assert_eq!(row_btn_text(&RowState::BuyNow, ARMOR_ROW_BASE, true), "Craft");
    }

    /// §14 + Spec 033: the server grants one `player_item` row per weapon
    /// named exactly `weapon.name`; the client's icon map keys those same
    /// strings — both sides build names from the same core helpers, so the
    /// counts must line up exactly once.
    #[test]
    fn matrix_covers_every_kind_material_without_collisions() {
        assert_eq!(weapons().len(), 71);
        assert_eq!(armors().len(), 40);
        let mut seen = std::collections::HashSet::new();
        for w in weapons() {
            assert!(seen.insert(w.name.clone()), "duplicate weapon name");
        }
        for a in armors() {
            assert!(seen.insert(a.name.clone()), "name collides with a weapon");
        }
        // Every weapon kind appears for every material.
        for kind in idlecore_core::player_combat::WEA_KINDS {
            for mat in idlecore_core::player_combat::WEA_MATERIALS {
                assert!(
                    weapons()
                        .iter()
                        .any(|w| w.name == format!("{} {}", mat.0, kind.name)),
                    "missing {mat:?} {kind:?}"
                );
            }
        }
    }
}
