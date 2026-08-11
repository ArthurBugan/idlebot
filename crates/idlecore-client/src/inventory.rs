//! Inventory UI — toggleable skin selector panel.
//! Press **I** to open/close. Scroll to browse skins. Click to equip.

use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use crate::skins::{PlayerSkins, SKIN_FILES};

/// Resource tracking whether the inventory is currently open.
#[derive(Resource, Default)]
pub struct InventoryOpen(pub bool);

/// Marker for inventory UI root (panel + overlay).
#[derive(Component)]
pub struct InventoryRoot;

/// Attached to the root Node of each skin button container.
/// Holds the skin index for that button.
#[derive(Component, Debug, Clone, Copy)]
pub struct SkinButton(pub usize);

/// Attached to the close button root Node.
#[derive(Component, Debug, Clone, Copy)]
pub struct CloseButton;

/// Attached to the preview image Node.
#[derive(Component)]
pub struct PreviewImage;

/// Attached to the selected skin name Text entity.
#[derive(Component)]
pub struct SelectedName;

/// Attached to the scrollable skin list container.
#[derive(Component)]
pub struct SkinList;

/// The inventory plugin: spawn UI, handle I key, handle clicks.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryOpen>();
        app.init_resource::<SkinTextures>();
        app.add_systems(Startup, (spawn_inventory, spawn_overlay, load_skin_textures));
        app.add_systems(Update, (
            inventory_toggle,
            update_preview_on_open,
            handle_skin_click,
            handle_close_click,
            update_skin_preview_image,
        ));
    }
}

// ─── Spawning ────────────────────────────────────────────────────────

fn spawn_inventory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let font = FontSource::Handle(font);
    let white = Color::WHITE;
    let dim_text = Color::srgb(0.7, 0.7, 0.8);

    commands
        .spawn((
            Name::new("inventory_panel"),
            InventoryRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(70.0),
                height: Val::Percent(70.0),
                left: Val::Percent(15.0),
                top: Val::Percent(15.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(10.0),
                overflow: Overflow::clip(),
                ..default()
            },
            Transform::IDENTITY,
            GlobalTransform::default(),
            BackgroundColor(Color::srgba(0.04, 0.04, 0.08, 0.95)),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("Inventory \u{2014} Select Skin"),
                TextFont { font: font.clone(), font_size: FontSize::Px(20.0), ..default() },
                TextColor(white),
                TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
                Transform::IDENTITY,
                GlobalTransform::default(),
            ));

            // Preview row: image + name
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|pv| {
                    // Preview image container
                    pv.spawn((
                        Node {
                            width: Val::Px(160.0),
                            height: Val::Px(160.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.12, 0.18, 1.0)),
                        BorderColor::all(Color::srgba(0.3, 0.3, 0.5, 1.0)),
                        PreviewImage,
                    ));
                    // Selected skin name
                    pv.spawn((
                        Name::new("selected_name"),
                        SelectedName,
                        Text::new(&SKIN_FILES[0][..SKIN_FILES[0].len().min(20)]),
                        TextFont { font: font.clone(), font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(0.85, 0.85, 0.95)),
                        Transform::IDENTITY,
                        GlobalTransform::default(),
                    ));
                });

            // Scrollable skin list container
            parent
                .spawn((
                    Name::new("skin_list_container"),
                    SkinList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        width: Val::Percent(100.0),
                        height: Val::Px(300.0),
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.8)),
                    BorderColor::all(Color::srgba(0.2, 0.2, 0.35, 0.8)),
                ))
                .with_children(|scroll_container| {
                    // Inner content that will be taller than the container
                    scroll_container
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::FlexStart,
                            column_gap: Val::Px(6.0),
                            row_gap: Val::Px(6.0),
                            width: Val::Px(500.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        })
                        .with_children(|grid| {
                            for (i, name) in SKIN_FILES.iter().enumerate() {
                                let skin_name = name.to_string();
                                let f = font.clone();
                                grid.spawn((
                                    Name::new(format!("skin_{}", i)),
                                    SkinButton(i),
                                    Node {
                                        width: Val::Px(60.0),
                                        height: Val::Px(74.0),
                                        padding: UiRect::all(Val::Px(2.0)),
                                        flex_direction: FlexDirection::Column,
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.08, 0.08, 0.14, 0.95)),
                                    BorderColor::all(Color::srgba(0.2, 0.2, 0.35, 0.95)),
                                    Button,
                                ))
                                .with_children(|btn| {
                                    // Skin thumbnail (colored placeholder for now)
                                    btn.spawn((
                                        Name::new(format!("skin_thumb_{}", i)),
                                        Node {
                                            width: Val::Px(52.0),
                                            height: Val::Px(52.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.04, 0.04, 0.08, 1.0)),
                                        BorderColor::all(Color::srgba(0.15, 0.15, 0.25, 1.0)),
                                    ));
                                    // Skin name label
                                    let label = if skin_name.len() > 12 {
                                        format!("{}...", &skin_name[..12])
                                    } else {
                                        skin_name.clone()
                                    };
                                    btn.spawn((
                                        Text::new(label),
                                        TextFont { font: f, font_size: FontSize::Px(7.0), ..default() },
                                        TextColor(dim_text),
                                    ));
                                });
                            }
                        });
                });

            // Close button
            parent
                .spawn((
                    Name::new("close_btn"),
                    CloseButton,
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.55, 0.12, 0.12, 0.9)),
                    BorderColor::all(Color::srgba(0.7, 0.2, 0.2, 0.9)),
                    Button,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Close [I]"),
                        TextFont { font, font_size: FontSize::Px(12.0), ..default() },
                        TextColor(white),
                    ));
                });
        });
}

fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Name::new("inventory_overlay"),
        InventoryRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Transform::IDENTITY,
        GlobalTransform::default(),
    ));
}

// ─── Texture Loading ─────────────────────────────────────────────────

/// Resource to hold loaded skin textures for preview.
#[derive(Resource, Default)]
pub struct SkinTextures {
    pub images: Vec<Option<Handle<Image>>>,
}

fn load_skin_textures(
    asset_server: Res<AssetServer>,
    mut textures: ResMut<SkinTextures>,
) {
    if !textures.images.is_empty() {
        return;
    }
    textures.images = SKIN_FILES
        .iter()
        .map(|name| {
            Some(asset_server.load(format!("skins/{}.png", name)))
        })
        .collect();
}

// ─── Toggle ──────────────────────────────────────────────────────────

fn inventory_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<InventoryOpen>,
    mut root_vis: Query<&mut Visibility, With<InventoryRoot>>,
) {
    if keys.just_pressed(KeyCode::KeyI) {
        open.0 = !open.0;
        let vis = if open.0 { Visibility::Visible } else { Visibility::Hidden };
        for mut v in root_vis.iter_mut() {
            *v = vis;
        }
        info!("Inventory: {}", if open.0 { "open" } else { "closed" });
    }
}

// ─── Preview ─────────────────────────────────────────────────────────

fn update_preview_on_open(
    open: Res<InventoryOpen>,
    skins: Res<PlayerSkins>,
    mut name_query: Query<&mut Text, With<SelectedName>>,
) {
    if !open.0 {
        return;
    }
    let idx = skins.current;
    if let Some(mut text) = name_query.iter_mut().next() {
        let name = &SKIN_FILES[idx];
        **text = if name.len() > 24 {
            format!("{}...", &name[..24])
        } else {
            name.to_string()
        };
    }
}

fn update_skin_preview_image(
    _skins: Res<PlayerSkins>,
    _skin_textures: Res<SkinTextures>,
) {
    // Preview image would need ImageNode component setup
    // For now, the skin changes are applied to the 3D model
}

// ─── Click handling ──────────────────────────────────────────────────

/// Walk up the parent chain to find a SkinButton ancestor.
fn find_skin_button(
    entity: Entity,
    child_of: &Query<&ChildOf>,
    skin_buttons: &Query<&SkinButton>,
) -> Option<SkinButton> {
    let mut current = entity;
    let mut visited = std::collections::HashSet::new();
    visited.insert(current);

    loop {
        if let Ok(btn) = skin_buttons.get(current) {
            return Some(*btn);
        }
        if let Ok(child_of) = child_of.get(current) {
            let parent = child_of.0;
            if visited.contains(&parent) {
                break;
            }
            visited.insert(parent);
            current = parent;
        } else {
            break;
        }
    }
    None
}

/// Handle clicks on skin buttons.
fn handle_skin_click(
    mut events: MessageReader<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    skin_buttons: Query<&SkinButton>,
    mut skins: ResMut<PlayerSkins>,
    mut open: ResMut<InventoryOpen>,
) {
    for event in events.read() {
        let target = event.entity;
        if let Some(btn) = find_skin_button(target, &child_of, &skin_buttons) {
            equip_skin(btn.0, &mut skins, &mut open);
        }
    }
}

/// Handle clicks on the close button.
fn handle_close_click(
    mut events: MessageReader<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    close_buttons: Query<&CloseButton>,
    mut open: ResMut<InventoryOpen>,
) {
    for event in events.read() {
        let target = event.entity;
        let mut current = target;
        let mut visited = std::collections::HashSet::new();
        visited.insert(current);

        loop {
            if close_buttons.get(current).is_ok() {
                open.0 = false;
                return;
            }
            if let Ok(child_of) = child_of.get(current) {
                let parent = child_of.0;
                if visited.contains(&parent) {
                    break;
                }
                visited.insert(parent);
                current = parent;
            } else {
                break;
            }
        }
    }
}

/// Equip a skin at the given index.
fn equip_skin(index: usize, skins: &mut PlayerSkins, open: &mut InventoryOpen) {
    if index < skins.textures.len() {
        skins.current = index;
        skins.need_bake = true;
        info!("Equipped skin: {}", SKIN_FILES[index]);
        open.0 = false;
    }
}
