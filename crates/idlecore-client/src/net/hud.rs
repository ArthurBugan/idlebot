//! Network HUD — connection status, authoritative player stats, and buttons
//! that invoke server reducers (login, plant, harvest, clean, claim idle,
//! vehicles). Server replies surface in the log box.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::ClientPlayer;
use spacetimedb_sdk::Table;
use super::gen::*;
use super::plugin::{Net, NetEvent, NetStatus};

#[derive(Debug, Clone, Copy, Component)]
enum HudAction {
    Connect,
    Harvest,
    Clean,
    PlantTree,
    ClaimIdle,
    BuyBicycle,
    EquipBicycle,
    Teleport,
    EditName,
    AvatarNext,
}

/// Avatar shapes the server accepts (Spec 014 T1.1).

/// In-progress display-name edit (Spec 014 T4.2): ENTER submits, Backspace
/// deletes, printable key chars append (alphanumeric only, max 20).
#[derive(Resource, Default)]
pub struct NameEdit {
    pub editing: bool,
    pub buffer: String,
}

const TEXT_COLOR: Color = Color::srgb(0.9, 0.95, 1.0);
const BUTTON_COLOR: Color = Color::srgb(0.15, 0.3, 0.55);

pub struct NetHudPlugin;

/// Throttle the per-frame HUD text rebuild to keep idle frames cheap.
#[derive(Resource)]
pub(crate) struct HudThrottle {
    last: std::time::Instant,
}

impl Default for HudThrottle {
    fn default() -> Self {
        Self { last: std::time::Instant::now() }
    }
}

const HUD_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

impl Plugin for NetHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NameEdit>()
            .init_resource::<super::plugin::ServerLatency>()
            .init_resource::<HudThrottle>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (update_hud_text, hud_buttons, name_input),
            );
    }
}

/// Toggled with F1 so it never blocks inventory interaction.
#[derive(Component)]
struct DebugHudRoot;

#[derive(Resource, Default)]
struct DebugHudVisible(bool);

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
    commands.insert_resource(DebugHudVisible(false));
    let mut root = commands.spawn((
        DebugHudRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            display: Display::None,
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
            ("Harvest (15G)", HudAction::Harvest),
            ("Clean (20G)", HudAction::Clean),
            ("Plant Tree (seed)", HudAction::PlantTree),
            ("Claim Idle Gains", HudAction::ClaimIdle),
            ("Buy Bicycle (500G)", HudAction::BuyBicycle),
            ("Equip Bicycle", HudAction::EquipBicycle),
            ("Teleport (hex)", HudAction::Teleport),
            ("Edit Name", HudAction::EditName),
            ("Avatar -> Next", HudAction::AvatarNext),
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
    mut throttle: ResMut<HudThrottle>,
    net: Res<Net>,
    minimap_state: Res<crate::minimap::MinimapState>,
    latency: Res<super::plugin::ServerLatency>,
    player: Option<Query<&ClientPlayer>>,
    mut status_q: Query<&mut Text, (With<HudStatusText>, Without<HudStatsText>, Without<HudLogText>)>,
    mut stats_q: Query<&mut Text, (With<HudStatsText>, Without<HudStatusText>, Without<HudLogText>)>,
    mut log_q: Query<&mut Text, (With<HudLogText>, Without<HudStatusText>, Without<HudStatsText>)>,
    name_edit: Res<NameEdit>,
) {
    if throttle.last.elapsed() < HUD_REFRESH_INTERVAL {
        return;
    }
    throttle.last = std::time::Instant::now();
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
    if name_edit.editing {
        status_line.push_str(&format!("\nname: {}_", name_edit.buffer));
    }
    if let Some(addr) = &net.address {
        let display_name = net
            .conn
            .as_ref()
            .and_then(|c| c.db.player().address().find(addr))
            .and_then(|p| p.display_name.clone())
            .unwrap_or_default();
        if !display_name.is_empty() {
            status_line.push_str(&format!("\n{display_name} ({addr})"));
        } else {
            status_line.push_str(&format!("\nwallet {addr}"));
        }
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
            "LV {} | XP {} | Gold {} | USDT {} | Eco {} | Vehicle {}",
            p.level,
            p.xp,
            p.gold,
            p.usdt,
            p.eco_points,
            p.owned_vehicle
                .as_ref()
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "None".to_string())
        ));
        // Spec 017: XP progress toward next level (threshold 100 * level^2).
        if p.level < 100 {
            let need = 100u64 * (p.level as u64) * (p.level as u64);
            stats.push_str(&format!(
                "\nXP {}/{} to Level {}",
                p.xp.min(need),
                need,
                p.level + 1
            ));
        }
        // Spec 001: show unclaimed idle gains banked server-side.
        if let (Some(conn), Some(mine)) = (&net.conn, &net.address) {
            if let Some(g) = conn.db.idle_gain().player().find(mine) {
                if g.pending_gold > 0 || g.pending_xp > 0 {
                    stats.push_str(&format!(
                        "\nIdle pending: +{}G +{}XP",
                        g.pending_gold, g.pending_xp
                    ));
                }
            }
        }
        // Spec 020 T2.5/T3.2: eco rank by player EP, with next-unlock hint.
        let (rank, next) = eco_rank(p.eco_points);
        stats.push_str(&format!("\nEco rank: {rank} ({} EP)", p.eco_points));
        if let Some(unlock_at) = next {
            stats.push_str(&format!(" next at {unlock_at}"));
        }
        // Spec 020 T2.4: eco rating of the hex under the player.
        if let Some(conn) = &net.conn {
            let hex_id = crate::net::plugin::Net::hex_id_at(p.position.x, p.position.y);
            if let Some(h) = conn.db.hex_tile().hex_id().find(&hex_id) {
                // Spec 020 T5.4: "Eco-Friendly" marker for 100+ hexes.
                let flag = if h.eco_rating >= 100 { " (Eco-Friendly)" } else { "" };
                stats.push_str(&format!(
                    "\nHex eco: {} {}{}",
                    h.eco_rating,
                    eco_title(h.eco_rating),
                    flag
                ));
                // Earth replica: gatherable materials on this biome.
                let mats = idlecore_core::earth::materials_for_name(&h.terrain);
                if !mats.is_empty() {
                    let list = mats
                        .iter()
                        .map(|(name, yield_per)| format!("{name}×{yield_per}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    stats.push_str(&format!("\nHere: {list}"));
                }
            }
            // Nearest real-world city, with distance in km.
            if let Some((city, dist_km)) =
                idlecore_core::earth::nearest_city(p.position.x, p.position.y)
            {
                stats.push_str(&format!("\nNear: {} ({:.0} km)", city.name, dist_km));
            }
        }
        // Resource-node inventory (seeds fuel tree planting).
        if let Some(conn) = &net.conn {
            use spacetimedb_sdk::Table;
            if let Some(item) = conn
                .db
                .player_item()
                .iter()
                .find(|i| Some(&i.player) == net.address.as_ref() && i.item == "Seed")
            {
                stats.push_str(&format!("\nSeeds: {}", item.count));
            }
        }
        // Spec 006 T3.4: vehicle inventory from the subscription cache.
        if let (Some(conn), Some(mine)) = (&net.conn, &net.address) {
            let owned: Vec<String> = conn
                .db
                .player_vehicle()
                .iter()
                .filter(|v| v.player == *mine)
                .map(|v| v.vehicle_type.clone())
                .collect();
            if !owned.is_empty() {
                stats.push_str(&format!("\nOwned: {}", owned.join(", ")));
            }
        }
        // Spec 009 T3.2: show the selected teleport destination and cost.
        match minimap_state.selected_hex {
            Some((q, r)) => {
                let cost = (100.0 * (p.level as f32).sqrt()) as u64;
                stats.push_str(&format!("\nTeleport -> hex ({q},{r}) cost {cost}G"));
            }
            None => {}
        }
    }
    // Spec 018 T6.2: measured server round trip (teleport echo).
    if let Some(avg) = latency.window.avg_ms() {
        stats.push_str(&format!("\nNet: {:.0} ms avg ({} samples)", avg, latency.window.sample_count()));
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
pub(crate) fn reducer_report(name: &'static str, tx: std::sync::mpsc::Sender<NetEvent>, hex: u64) -> impl FnOnce(&super::gen::ReducerEventContext, Result<Result<(), String>, spacetimedb_sdk::__codegen::InternalError>) + Send + 'static {
    move |_ctx, res| {
        let (ok, msg) = match &res {
            Ok(Ok(())) => (true, format!("{name} ok (hex {hex})")),
            Ok(Err(e)) => (false, e.clone()),
            Err(e) => (false, format!("send error: {e}")),
        };
        let _ = tx.send(NetEvent::ReducerResult { name, ok, msg });
    }
}

/// Reducer callback for `teleport_player` (SDK drops the (x, y) return value).
fn teleport_report(name: &'static str, tx: std::sync::mpsc::Sender<NetEvent>, q: i32, r: i32) -> impl FnOnce(&super::gen::ReducerEventContext, Result<Result<(), String>, spacetimedb_sdk::__codegen::InternalError>) + Send + 'static {
    move |_ctx, res| {
        let (ok, msg) = match &res {
            Ok(Ok(())) => (true, format!("teleport ok -> hex ({q},{r})")),
            Ok(Err(e)) => (false, e.clone()),
            Err(e) => (false, format!("send error: {e}")),
        };
        if ok {
            let _ = tx.send(NetEvent::Teleported { q, r });
        }
        let _ = tx.send(NetEvent::ReducerResult { name, ok, msg });
    }
}

/// Invoke a reducer, reporting local send failures to the HUD log.
pub(crate) fn send_reducer(
    net: &mut Net,
    f: impl FnOnce(&RemoteReducers) -> Result<(), spacetimedb_sdk::Error>,
) {
    let Some(conn) = net.conn.as_ref() else {
        let _ = net.sender().send(NetEvent::ServerMessage("not connected — click Connect first".to_string()));
        return;
    };
    let tx = net.sender();
    match f(&conn.reducers) {
        Ok(()) => net.mark_players_dirty(),
        Err(e) => {
            let _ = tx.send(NetEvent::ServerMessage(format!("send failed: {e}")));
        }
    }
}

/// Send the teleport for the currently selected hex (shared by the HUD
/// button and the minimap/world double-click shortcut). Clears the selection
/// once sent; returns false when nothing is selected or not connected.
pub(crate) fn try_send_teleport(
    net: &mut Net,
    minimap_state: &mut crate::minimap::MinimapState,
    latency: &mut super::plugin::ServerLatency,
) -> bool {
    let Some((q, r)) = minimap_state.selected_hex else {
        let _ = net.sender().send(NetEvent::ServerMessage(
            "Teleport: click a hex on the map or minimap first".to_string(),
        ));
        return false;
    };
    let tx = net.sender();
    latency.note_request();
    send_reducer(net, |reducers| {
        reducers.teleport_player_then(q, r, teleport_report("teleport", tx.clone(), q, r))
    });
    minimap_state.selected_hex = None;
    minimap_state.selected_px = None;
    true
}

/// Click handling: invoke the corresponding server reducer.
fn hud_buttons(
    mut net: ResMut<Net>,
    mut minimap_state: ResMut<crate::minimap::MinimapState>,
    mut name_edit: ResMut<NameEdit>,
    mut latency: ResMut<super::plugin::ServerLatency>,
    mut skins: ResMut<crate::skins::PlayerSkins>,
    inventory: Res<crate::inventory::Inventory>,
    action_target: Res<crate::world_floor::ActionTarget>,
    _player: Option<Query<&ClientPlayer>>,
    mut interactions: Query<(&Interaction, &HudAction), Changed<Interaction>>,
) {
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
                let Some(_conn) = net.conn.as_ref() else {
                    let _ = net.sender().send(NetEvent::ServerMessage(
                        "not connected — click Connect first".to_string(),
                    ));
                    continue;
                };
                match action {
                    HudAction::Harvest => {
                        let hex = idlecore_core::hex::HexCoord::new(action_target.q, action_target.r).to_id();
                        send_reducer(&mut net, |r| r.harvest_then(hex, reducer_report("harvest", tx.clone(), hex)));
                    }
                    HudAction::Clean => {
                        let hex = idlecore_core::hex::HexCoord::new(action_target.q, action_target.r).to_id();
                        send_reducer(&mut net, |r| r.clean_then(hex, reducer_report("clean", tx.clone(), hex)));
                    }
                    HudAction::PlantTree => {
                        // Minecraft-style: the seed must be in hand (selected
                        // in the hotbar).
                        let holding_seed = inventory
                            .active_item()
                            .map(String::as_str)
                            == Some("Seed");
                        if !holding_seed {
                            let _ = tx.send(NetEvent::ServerMessage(
                                "Select Seeds in your hotbar first [1-9]".to_string(),
                            ));
                            continue;
                        }
                        let hex = idlecore_core::hex::HexCoord::new(action_target.q, action_target.r).to_id();
                        send_reducer(&mut net, |r| {
                            r.plant_tree_then(hex, reducer_report("plant_tree", tx.clone(), hex))
                        });
                    }
                    HudAction::ClaimIdle => {
                        send_reducer(&mut net, |r| r.claim_idle_gains_then(reducer_report("claim_idle_gains", tx.clone(), 0)));
                    }
                    HudAction::BuyBicycle => {
                        send_reducer(&mut net, |r| r.buy_vehicle_then("Bicycle".to_string(), reducer_report("buy_vehicle", tx.clone(), 0)));
                    }
                    HudAction::EquipBicycle => {
                        send_reducer(&mut net, |r| r.equip_vehicle_then("Bicycle".to_string(), reducer_report("equip_vehicle", tx.clone(), 0)));
                    }
                    HudAction::Teleport => {
                        crate::net::hud::try_send_teleport(&mut net, &mut minimap_state, &mut latency);
                    }
                    HudAction::EditName => {
                        name_edit.editing = !name_edit.editing;
                        if name_edit.editing {
                            name_edit.buffer.clear();
                        }
                    }
                    HudAction::AvatarNext => {
                        // Visible effect, same as the ] key: cycle the skin
                        // painted on the model, then persist the choice as
                        // the avatar column so it survives reconnects.
                        crate::skins::cycle_skin_dir(&mut skins, 1);
                        let skin_name = crate::skins::TOON_CHARACTERS
                            .get(skins.current)
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        send_reducer(&mut net, |r| r.update_profile_then(
                            None,
                            Some(skin_name),
                            None,
                            reducer_report("update_profile", tx.clone(), 0),
                        ));
                    }
                    HudAction::Connect => {}
                }
            }
        }
    }
}

/// Spec 020 T2.5: title for a hex's eco rating.
/// Spec 014 T4.2: keyboard capture while name editing. ENTER submits via
/// update_profile; Backspace deletes; printable alphanumeric chars append
/// (server enforces ≤20 alphanumeric again).
fn name_input(
    mut keys: MessageReader<KeyboardInput>,
    mut name_edit: ResMut<NameEdit>,
    net: Res<Net>,
) {
    if !name_edit.editing {
        return;
    }
    for event in keys.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        let key = event.key_code;
        match key {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                    let name = name_edit.buffer.trim().to_string();
                    name_edit.editing = false;
                    name_edit.buffer.clear();
                    if name.is_empty() {
                        continue;
                    }
                    if let (Some(conn), Some(tx)) = (&net.conn, Some(net.sender())) {
                        let _ = tx.send(NetEvent::ServerMessage("submitting name".to_string()));
                        let _ = conn.reducers.update_profile_then(
                            Some(name),
                            None,
                            None,
                            reducer_report("update_profile", tx, 0),
                        );
                    }
                }
                KeyCode::Backspace => {
                    name_edit.buffer.pop();
                }
                KeyCode::Escape => {
                    name_edit.editing = false;
                    name_edit.buffer.clear();
                }
                _ => {
                    if let Some(text) = &event.text {
                        if name_edit.buffer.chars().count() >= 20 {
                            continue;
                        }
                        for ch in text.chars().filter(|c| c.is_alphanumeric()) {
                            name_edit.buffer.push(ch);
                        }
                    }
                }
            }
    }
}

fn eco_title(rating: i32) -> &'static str {
    if rating >= 80 {
        "Lush"
    } else if rating >= 50 {
        "Healthy"
    } else if rating >= 25 {
        "Strained"
    } else {
        "Degraded"
    }
}

/// Spec 020 T2.5/T3.2: player eco rank by EP, plus the next unlock threshold.
fn eco_rank(ep: u64) -> (&'static str, Option<u64>) {
    if ep >= 1000 {
        ("Eco Legend", None)
    } else if ep >= 500 {
        ("Eco Warrior", Some(1000))
    } else if ep >= 100 {
        ("Eco Enthusiast", Some(500))
    } else {
        ("Eco Scout", Some(100))
    }
}

/// F1 hides/shows the whole debug panel (status, buttons, log).
fn toggle_debug_hud(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DebugHudVisible>,
    mut panel: Query<&mut Node, With<DebugHudRoot>>,
) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }
    state.0 = !state.0;
    let want = if state.0 { Display::Flex } else { Display::None };
    if let Ok(mut node) = panel.single_mut() {
        node.display = want;
    }
}
