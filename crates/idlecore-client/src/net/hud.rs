//! Network HUD — connection status, authoritative player stats, and buttons
//! that invoke server reducers (login, plant, harvest, clean, claim idle,
//! vehicles). Server replies surface in the log box.

use bevy::prelude::*;

use crate::player::ClientPlayer;
use super::gen::*;
use super::plugin::{Net, NetEvent, NetStatus};

#[derive(Debug, Clone, Copy, Component)]
enum HudAction {
    Connect,
    Plant,
    Harvest,
    Clean,
    ClaimIdle,
    BuyBicycle,
    EquipBicycle,
}

const TEXT_COLOR: Color = Color::srgb(0.9, 0.95, 1.0);
const BUTTON_COLOR: Color = Color::srgb(0.15, 0.3, 0.55);
const BUTTON_HOVER: Color = Color::srgb(0.2, 0.4, 0.7);

pub struct NetHudPlugin;

impl Plugin for NetHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_hud_text, hud_buttons));
    }
}

#[derive(Component)]
struct HudStatusText;

#[derive(Component)]
struct HudStatsText;

#[derive(Component)]
struct HudLogText;

fn button_style(width: f32, height: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(height),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        margin: UiRect::all(Val::Px(2.0)),
        ..default()
    }
}

fn spawn_hud(mut commands: Commands) {
    let mut root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.85)),
    ));
    root.with_children(|parent| {
        parent.spawn((
            Text::new("Connecting..."),
            TextFont { font_size: 13.0.into(), ..default() },
            TextColor(TEXT_COLOR),
            HudStatusText,
        ));
        parent.spawn((
            Text::new(""),
            TextFont { font_size: 13.0.into(), ..default() },
            TextColor(TEXT_COLOR),
            HudStatsText,
        ));

        for (label, action) in [
            ("Connect & Login", HudAction::Connect),
            ("Plant Wheat (10G)", HudAction::Plant),
            ("Harvest (15G)", HudAction::Harvest),
            ("Clean (20G)", HudAction::Clean),
            ("Claim Idle Gains", HudAction::ClaimIdle),
            ("Buy Bicycle (500G)", HudAction::BuyBicycle),
            ("Equip Bicycle", HudAction::EquipBicycle),
        ] {
            parent
                .spawn((
                    Button,
                    button_style(150.0, 30.0),
                    BackgroundColor(BUTTON_COLOR),
                    action,
                ))
                .with_child((
                    Text::new(label),
                    TextFont { font_size: 13.0.into(), ..default() },
                    TextColor(TEXT_COLOR),
                ));
        }

        parent.spawn((
            Text::new(""),
            TextFont { font_size: 12.0.into(), ..default() },
            TextColor(TEXT_COLOR),
            HudLogText,
        ));
    });
}

/// Refresh status/stats/log text every frame from `Net` + `ClientPlayer`.
fn update_hud_text(
    net: Res<Net>,
    player: Option<Query<&ClientPlayer>>,
    mut status_q: Query<&mut Text, (With<HudStatusText>, Without<HudStatsText>, Without<HudLogText>)>,
    mut stats_q: Query<&mut Text, (With<HudStatsText>, Without<HudStatusText>, Without<HudLogText>)>,
    mut log_q: Query<&mut Text, (With<HudLogText>, Without<HudStatusText>, Without<HudStatsText>)>,
) {
    let status = match &net.status {
        NetStatus::Connected => "Connected".to_string(),
        NetStatus::Connecting => "Connecting...".to_string(),
        NetStatus::Error(e) => format!("Error: {e}"),
        NetStatus::Disconnected => "Disconnected".to_string(),
    };
    let mut status_line = status.clone();
    if !net.identity.is_empty() {
        status_line.push_str(&format!("\nidentity {}", short(&net.identity)));
    }
    if let Some(addr) = &net.address {
        status_line.push_str(&format!("\nwallet {addr}"));
    }
    let online = net
        .players
        .values()
        .filter(|p| p.online)
        .count();
    status_line.push_str(&format!("\nplayers: {online} online"));
    if let Ok(mut t) = status_q.single_mut() {
        t.0 = status_line;
    }

    let mut stats = String::new();
    if let Some(p) = player.as_ref().and_then(|q| q.single().ok()) {
        stats.push_str(&format!(
            "LV {} | XP {} | Gold {} | Eco {} | Vehicle {}",
            p.level,
            p.xp,
            p.gold,
            p.eco_points,
            p.owned_vehicle
                .as_ref()
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "None".to_string())
        ));
    }
    if let Ok(mut t) = stats_q.single_mut() {
        t.0 = stats;
    }

    if let Ok(mut t) = log_q.single_mut() {
        t.0 = net.log.iter().cloned().collect::<Vec<_>>().join("\n");
    }
}

fn short(s: &str) -> String {
    if s.len() > 10 {
        format!("{}...", &s[..10])
    } else {
        s.to_string()
    }
}

/// Build a reducer callback that reports the outcome into the event channel.
/// `name` is reused for constructing Arg structs when the reducer has none.
fn reducer_report(name: &'static str, tx: std::sync::mpsc::Sender<NetEvent>, hex: u64) -> impl FnOnce(&super::gen::ReducerEventContext, Result<Result<(), String>, spacetimedb_sdk::__codegen::InternalError>) + Send + 'static {
    move |_ctx, res| {
        let (ok, msg) = match &res {
            Ok(Ok(())) => (true, format!("{name} ok (hex {hex})")),
            Ok(Err(e)) => (false, e.clone()),
            Err(e) => (false, format!("send error: {e}")),
        };
        let _ = tx.send(NetEvent::ReducerResult { name, ok, msg });
    }
}

/// Invoke a reducer, reporting local send failures to the HUD log.
fn send_reducer(
    conn: &DbConnection,
    tx: &std::sync::mpsc::Sender<NetEvent>,
    f: impl FnOnce(&RemoteReducers) -> Result<(), spacetimedb_sdk::Error>,
) {
    if let Err(e) = f(&conn.reducers) {
        let _ = tx.send(NetEvent::ServerMessage(format!("send failed: {e}")));
    }
}

/// Click handling: invoke the corresponding server reducer.
fn hud_buttons(mut net: ResMut<Net>, player: Option<Query<&ClientPlayer>>, mut interactions: Query<(&Interaction, &HudAction), Changed<Interaction>>) {
    for (interaction, action) in interactions.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let tx = net.sender();
        let _ = tx.send(NetEvent::ServerMessage(format!("click: {action:?}")));
        match action {
            // Must work with no existing connection — it opens one.
            // Login is issued from `Net::drain` once the handshake completes.
            HudAction::Connect => {
                net.connect();
            }
            _ => {
                let Some(conn) = net.conn.as_ref() else { continue };
                match action {
                    HudAction::Plant => {
                        let Some(pos) = player.as_ref().and_then(|q| q.single().ok()).map(|p| p.position) else { continue };
                        let hex = Net::hex_id_at(pos.x, pos.z);
                        send_reducer(conn, &tx, |r| r.plant_then(hex, "Wheat".to_string(), reducer_report("plant", tx.clone(), hex)));
                    }
                    HudAction::Harvest => {
                        let Some(pos) = player.as_ref().and_then(|q| q.single().ok()).map(|p| p.position) else { continue };
                        let hex = Net::hex_id_at(pos.x, pos.z);
                        send_reducer(conn, &tx, |r| r.harvest_then(hex, reducer_report("harvest", tx.clone(), hex)));
                    }
                    HudAction::Clean => {
                        let Some(pos) = player.as_ref().and_then(|q| q.single().ok()).map(|p| p.position) else { continue };
                        let hex = Net::hex_id_at(pos.x, pos.z);
                        send_reducer(conn, &tx, |r| r.clean_then(hex, reducer_report("clean", tx.clone(), hex)));
                    }
                    HudAction::ClaimIdle => {
                        send_reducer(conn, &tx, |r| r.claim_idle_gains_then(reducer_report("claim_idle_gains", tx.clone(), 0)));
                    }
                    HudAction::BuyBicycle => {
                        send_reducer(conn, &tx, |r| r.buy_vehicle_then("Bicycle".to_string(), reducer_report("buy_vehicle", tx.clone(), 0)));
                    }
                    HudAction::EquipBicycle => {
                        send_reducer(conn, &tx, |r| r.equip_vehicle_then("Bicycle".to_string(), reducer_report("equip_vehicle", tx.clone(), 0)));
                    }
                    HudAction::Connect => {}
                }
            }
        }
    }
}