//! Marketplace (Spec 011) — browse `market_listing` rows from the local
//! subscription cache, buy with USDT, and publish preset listings.
//!
//! - Toggle the panel with `K`.
//! - Each listing row shows title, price, seller and a Buy button.
//! - Four preset Publish buttons (Agent/Code/Template/Snippet) create a
//!   10 USDT listing.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::net::hud::{reducer_report, send_reducer};
use crate::time_ext::now_unix_secs;
use crate::net::plugin::Net;
use crate::net::plugin::NetEvent;
use super::gen::*;
use spacetimedb_sdk::Table;

const PANEL_WIDTH: f32 = 320.0;
const ROW_HEIGHT: f32 = 26.0;
const MAX_ROWS: usize = 8;
const PUBLISH_PRICE_USDT: u64 = 10;
/// Platform fee permille (Spec 011): 50 = 5% of the price.
const PLATFORM_FEE_PERMILLE: u64 = 50;

/// Root of the marketplace panel.
#[derive(Component)]
pub struct MarketPanel;

/// Container for listing rows.
#[derive(Component)]
pub struct MarketRowList;

/// Buy button attached to one listing.
#[derive(Component)]
pub struct BuyListingButton {
    pub listing_id: u64,
}

/// Preset publish button (one per category).
#[derive(Component)]
pub struct PublishCategory {
    pub category: &'static str,
}

pub struct MarketPlugin;

impl Plugin for MarketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_market_panel)
            .add_systems(Update, (update_market_panel, market_buttons, market_toggle));
    }
}

fn button_style(width: f32, height: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(height),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn spawn_market_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let _font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");

    commands
        .spawn((
            Name::new("market-panel"),
            MarketPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(180.0),
                top: Val::Px(36.0),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(400.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.94)),
            BorderColor::all(Color::srgba(0.3, 0.45, 0.7, 1.0)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("MARKETPLACE (K)"),
                TextFont { font_size: 14.0.into(), ..default() },
                TextColor(Color::srgb(0.6, 0.9, 1.0)),
            ));
            parent.spawn((
                MarketRowList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new("Publish preset (10 USDT):"),
                TextFont { font_size: 12.0.into(), ..default() },
                TextColor(Color::srgb(0.8, 0.8, 0.9)),
            ));
            parent
                .spawn((
                    Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(3.0), ..default() },
                ))
                .with_children(|p| {
                    for (label, category) in [
                        ("Agent", "Agent"),
                        ("Code", "Code"),
                        ("Template", "Template"),
                        ("Snippet", "Snippet"),
                    ] {
                        p.spawn((
                            Button,
                            button_style(70.0, 24.0),
                            BackgroundColor(Color::srgb(0.2, 0.4, 0.5)),
                            PublishCategory { category },
                        ))
                        .with_child((
                            Text::new(label),
                            TextFont { font_size: 12.0.into(), ..default() },
                            TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        ));
                    }
                });
        });
}

fn now_unix() -> u64 {
    now_unix_secs()
}

/// Rebuild listing rows from the subscription cache every frame.
fn update_market_panel(
    net: Res<Net>,
    mut commands: Commands,
    market_row_list: Query<Entity, With<MarketRowList>>,
    row_entities: Query<Entity, With<BuyListingButton>>,
) {
    let Ok(list_entity) = market_row_list.single() else { return };

    for entity in row_entities.iter() {
        commands.entity(entity).despawn();
    }

    let conn_guard = net.conn.lock().unwrap();
    let Some(conn) = conn_guard.as_ref() else { return };
    let rows = super::gen::MarketListingTableAccess::market_listing(&conn.db).iter().collect::<Vec<_>>();
    let mine = net.address.clone().unwrap_or_default();
    let now = now_unix();

    for listing in rows.iter().take(MAX_ROWS) {
        let state = if listing.is_sold {
            "SOLD"
        } else if listing.expires_at < now {
            "EXPIRED"
        } else {
            ""
        };
        let title = if listing.title.len() > 20 {
            format!("{}..", &listing.title[..20])
        } else {
            listing.title.clone()
        };
        let is_self = listing.seller == mine;

        commands.entity(list_entity).with_children(|parent| {
            let mut row = parent.spawn((
                Name::new(format!("listing-{}", listing.listing_id)),
                Node {
                    height: Val::Px(ROW_HEIGHT),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            row.with_child((
                Text::new(format!(
                    "{title}  {} USDT  {}",
                    listing.price_usdt,
                    short(&listing.seller)
                )),
                TextFont { font_size: 11.0.into(), ..default() },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
            ));
            if state.is_empty() && !is_self {
                row.with_children(|p| {
                    p.spawn((
                        Button,
                        button_style(48.0, 20.0),
                        BackgroundColor(Color::srgb(0.16, 0.45, 0.3)),
                        BuyListingButton { listing_id: listing.listing_id },
                    ))
                    .with_child((
                        Text::new("Buy"),
                        TextFont { font_size: 11.0.into(), ..default() },
                        TextColor(Color::srgb(0.9, 1.0, 0.9)),
                    ));
                });
            } else if !state.is_empty() {
                row.with_child((
                    Text::new(state),
                    TextFont { font_size: 11.0.into(), ..default() },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
            }
        });
    }
}

fn market_buttons(
    mut net: ResMut<Net>,
    mut interactions: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Option<&BuyListingButton>,
            Option<&PublishCategory>,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, mut bg, buy, publish) in interactions.iter_mut() {
        match *interaction {
            Interaction::Hovered => bg.0 = Color::srgb(0.25, 0.5, 0.6),
            Interaction::None => bg.0 = Color::srgb(0.16, 0.45, 0.3),
            Interaction::Pressed => {}
        }
        if *interaction != Interaction::Pressed {
            continue;
        }
        let tx = net.sender();
        if net.conn.lock().unwrap().is_none() { continue; }

        if let Some(b) = buy {
            // Spec 011: bank takes 5% of the price, seller payout is escrowed 48 h.
            let fee_percent = PLATFORM_FEE_PERMILLE / 10;
            let _ = tx.send(NetEvent::ServerMessage(format!(
                "buy listing {} -> escrow, fee {}%",
                b.listing_id, fee_percent
            )));
            send_reducer(&mut net, |reducers| {
                reducers.buy_listing_then(
                    b.listing_id,
                    reducer_report("buy_listing", tx.clone(), 0),
                )
            });
        }
        if let Some(p) = publish {
            let Some(_mine) = net.address.clone() else { continue };
            let title = format!("{} pack #{}", p.category, now_unix() % 10000);
            let github_url =
                format!("https://github.com/example/agent-pack-{}", p.category.to_lowercase());
            send_reducer(&mut net, |reducers| {
                reducers.publish_listing_then(
                    title,
                    "Generated client preset".to_string(),
                    github_url,
                    PUBLISH_PRICE_USDT,
                    p.category.to_string(),
                    reducer_report("publish_listing", tx.clone(), 0),
                )
            });
        }
    }
}

fn market_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Visibility, With<MarketPanel>>,
) {
    let Ok(mut visibility) = panel.single_mut() else { return };
    if keyboard.just_pressed(KeyCode::KeyK) {
        *visibility = if *visibility == Visibility::Hidden {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn short(s: &str) -> String {
    if s.len() > 8 {
        format!("{}..", &s[..8])
    } else {
        s.to_string()
    }
}