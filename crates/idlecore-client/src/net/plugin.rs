//! SpacetimeDB connection plugin.
//!
//! Owns the `DbConnection` (idlebot module, local node), advances it every
//! frame via `frame_tick`, mirrors the authoritative `player` table into
//! [`Net`], renders other players as markers, and exposes reducer actions to
//! the rest of the client.

use super::hud::{reducer_report, send_reducer};
use crate::plugins::player::PhysicsBody;
use bevy::prelude::*;
use spacetimedb_sdk::__codegen::{TableLike, TableWithPrimaryKey, WithDelete, WithInsert};
use spacetimedb_sdk::DbContext;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::player::ClientPlayer;
use idlecore_core::hex::{world_pos_to_hex, HexCoord};
use idlecore_core::world_gen::WorldGenConfig;

use super::gen::*;
use crate::time_ext::{Instant, now_unix_secs};

/// Shared handle to the live `DbConnection`. Wrapped so the wasm connection
/// (established inside an async `spawn_local` that outlives `connect`) can
/// publish the connection back into the [`Net`] resource.
type ConnSlot = std::sync::Arc<std::sync::Mutex<Option<DbConnection>>>;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

/// Node address of the local SpacetimeDB instance.
pub const SERVER_URI: &str = "https://db.nestfeed.app";
/// Module (database) name published by `idlecore-server`.
pub const MODULE_NAME: &str = "nestfeed";

/// One-way messages from the SDK callbacks (network thread) to the main
/// thread, drained by [`Net::drain`] each frame.
#[derive(Debug, Clone)]
pub enum NetEvent {
    Connected {
        identity: String,
    },
    ConnectError(String),
    Disconnected(String),
    /// The server rejected our saved identity token (401/InvalidSignature —
    /// happens after the server re-keys). The token file has been cleared;
    /// reconnect once without it.
    StaleToken,
    ReducerResult {
        name: &'static str,
        ok: bool,
        msg: String,
    },
    /// A teleport confirmed by the server; the sim must move the player.
    Teleported {
        q: i32,
        r: i32,
    },
    ServerMessage(String),
}

/// Connection lifecycle.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NetStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Snapshot of another player's authoritative state (for markers/HUD).
#[derive(Debug, Clone, Default)]
pub struct ServerPlayerSnapshot {
    pub x: f32,
    pub y: f32,
    pub online: bool,
}

/// World-level network state resource.
#[derive(Resource)]
pub struct Net {
    pub status: NetStatus,
    pub conn: ConnSlot,
    /// Our bound wallet address (set after a successful `login`).
    pub address: Option<String>,
    /// Name picked on the login page, awaiting the login reducer result.
    pub pending_name: Option<String>,
    /// Our identity as issued by the server.
    pub identity: String,
    /// Snapshot of every player row we know about (including ourselves).
    pub players: HashMap<String, ServerPlayerSnapshot>,
    /// Recent server/connection messages for the HUD log.
    pub log: VecDeque<String>,
    /// One-shot guard: after a stale-token reconnect we never auto-retry
    /// again (prevents loops if even the anonymous connect fails).
    auth_retried: bool,
    /// Version counter bumped by SDK row callbacks; the per-frame
    /// `sync_remote_players` rebuild early-outs while it is unchanged.
    players_dirty: std::sync::Arc<std::sync::atomic::AtomicU64>,
    players_dirty_last: u64,
    tx: Sender<NetEvent>,
    rx: std::sync::Mutex<Receiver<NetEvent>>,
}

impl Default for Net {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            status: NetStatus::Disconnected,
            conn: std::sync::Arc::new(std::sync::Mutex::new(None)),
            address: None,
            pending_name: None,
            identity: String::new(),
            players: HashMap::new(),
            log: VecDeque::new(),
            auth_retried: false,
            players_dirty: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            players_dirty_last: 0,
            tx,
            rx: std::sync::Mutex::new(rx),
        }
    }
}

impl Net {
    /// Push an event onto the main-thread queue (called from SDK callbacks).
    pub fn push(&self, ev: NetEvent) {
        let _ = self.tx.send(ev);
    }

    /// Clone of the event sender, captured by reducer callbacks.
    pub fn sender(&self) -> Sender<NetEvent> {
        self.tx.clone()
    }

    /// Drain pending events and apply them to this resource.
    pub fn drain(&mut self) -> Vec<(i32, i32)> {
        let mut teleports = Vec::new();
        let mut pending = Vec::new();
        {
            let rx = self.rx.get_mut().unwrap();
            while let Ok(ev) = rx.try_recv() {
                pending.push(ev);
            }
        }
        for ev in pending {
            match ev {
                NetEvent::Connected { identity } => {
                    self.identity = identity;
                    self.status = NetStatus::Connected;
                    self.log_line("Connected to SpacetimeDB");
                    // Reducers can only be sent once the handshake is done.
                    // The login itself waits for the name picked on the
                    // login page (no auto-demo-login anymore).
                    match self.pending_name.clone() {
                        Some(name) => self.send_login(&name),
                        None => self.log_line("enter a name to join the world"),
                    }
                }
                NetEvent::ConnectError(msg) => {
                    self.status = NetStatus::Error(msg.clone());
                    self.log_line(&format!("Connect error: {msg}"));
                }
                NetEvent::StaleToken => {
                    if self.auth_retried {
                        self.log_line("stale token, but already retried — giving up");
                        continue;
                    }
                    self.auth_retried = true;
                    self.status = NetStatus::Disconnected;
                    *self.conn.lock().unwrap() = None;
                    self.log_line("token rejected (server re-key?) — reconnecting anonymously");
                    self.connect();
                }
                NetEvent::Disconnected(reason) => {
                    self.status = NetStatus::Disconnected;
                    self.log_line(&format!("Disconnected: {reason}"));
                }
                NetEvent::ReducerResult { name, ok, msg } => {
                    if name == "login" {
                        if ok {
                            // Bind the account only on server confirmation.
                            if self.address.is_none() {
                                self.address = self.pending_name.clone();
                            }
                            self.pending_name = None;
                            self.log_line(&format!("login: {msg}"));
                        } else {
                            // Free the name so the player can retry on the
                            // login page (e.g. rapid-login ban).
                            self.pending_name = None;
                            self.log_line(&format!("login FAILED: {msg}"));
                        }
                        continue;
                    }
                    let line = if ok {
                        format!("{name}: {msg}")
                    } else {
                        format!("{name} FAILED: {msg}")
                    };
                    self.log_line(&line);
                }
                NetEvent::Teleported { q, r } => teleports.push((q, r)),
                NetEvent::ServerMessage(msg) => self.log_line(&msg),
            }
        }
        teleports
    }

    fn log_line(&mut self, line: &str) {
        self.log.push_back(line.to_string());
        while self.log.len() > 6 {
            self.log.pop_front();
        }
    }

    /// Queue a login for `name` — the account key until wallet auth lands
    /// (Spec 013 chain SDK). Sanitized to ≤20 alphanumeric/`_` chars (the
    /// server lowercases). Fires immediately when already connected,
    /// otherwise on the next successful handshake.
    pub fn request_login(&mut self, raw: String) {
        // The server lowercases the address into the player row — normalize
        // here too, or our own row would look "remote" to the sync pass
        // (stray marker + name label chasing the player).
        let name: String = raw
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .take(20)
            .collect::<String>()
            .to_lowercase();
        if name.is_empty() {
            self.log_line("pick a name first (letters, numbers, _)");
            return;
        }
        self.pending_name = Some(name.clone());
        if matches!(self.status, NetStatus::Connected) {
            self.send_login(&name);
        } else {
            self.log_line(&format!("connecting as {name}…"));
        }
    }

    /// Send the `login` reducer for `name` (requires an open connection).
    fn send_login(&mut self, name: &str) {
        let guard = self.conn.lock().unwrap();
        let Some(conn) = guard.as_ref() else { return };
        let tx = self.tx.clone();
        let name = name.to_string();
        let _ = conn.reducers.login_then(name.clone(), move |_ctx, res| {
            let (ok, msg) = match &res {
                Ok(Ok(())) => (true, format!("welcome, {name}")),
                Ok(Err(e)) => (false, e.clone()),
                Err(e) => (false, format!("send error: {e}")),
            };
            let _ = tx.send(NetEvent::ReducerResult {
                name: "login",
                ok,
                msg,
            });
        });
    }

    /// Open the connection and start receiving (call once).
    pub fn connect(&mut self) {
        if !matches!(self.status, NetStatus::Disconnected) {
            return;
        }
        self.status = NetStatus::Connecting;
        let saved = load_saved_token();
        let had_saved = saved.is_some();
        let tx = self.tx.clone();

        let builder = DbConnection::builder()
            .with_uri(SERVER_URI)
            .with_database_name(MODULE_NAME)
            .with_token(saved)
            .on_connect({
                let tx = tx.clone();
                move |_ctx, identity, token| {
                    save_token(&identity.to_string(), token);
                    tx.send(NetEvent::Connected {
                        identity: identity.to_string(),
                    })
                    .ok();
                }
            })
            .on_connect_error({
                let tx = tx.clone();
                move |_ctx, err| {
                    let msg = err.to_string();
                    tx.send(NetEvent::ConnectError(msg.clone())).ok();
                    // A re-keyed server invalidates every old token
                    // (401 / InvalidSignature). Drop the stale file and let
                    // the drain loop reconnect anonymously once.
                    if had_saved
                        && (msg.contains("401")
                            || msg.contains("InvalidSignature")
                            || msg.to_lowercase().contains("invalid token")
                            || msg.to_lowercase().contains("unauthorized"))
                    {
                        clear_saved_token();
                        tx.send(NetEvent::StaleToken).ok();
                    }
                }
            })
            .on_disconnect({
                let tx = tx.clone();
                move |_ctx, reason| {
                    tx.send(NetEvent::Disconnected(format!("{reason:?}"))).ok();
                }
            });

        // The connection must be driven differently per platform: native pumps
        // it from `net_frame_tick` (frame_tick); wasm has no threads, so the
        // browser SDK spawns its own background task via `run_background_task`.
        let dirty = self.players_dirty.clone();
        let conn_slot = self.conn.clone();
        #[cfg(target_arch = "wasm32")]
        {
            spawn_local(async move {
                let conn = match builder.build().await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(NetEvent::ConnectError(e.to_string()));
                        return;
                    }
                };
                register_net_callbacks(&conn, tx.clone(), dirty.clone());
                conn.run_background_task();
                *conn_slot.lock().unwrap() = Some(conn);
            });
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let conn = match builder.build() {
                Ok(c) => c,
                Err(e) => {
                    self.status = NetStatus::Error(e.to_string());
                    return;
                }
            };
            register_net_callbacks(&conn, tx.clone(), dirty.clone());
            *conn_slot.lock().unwrap() = Some(conn);
        }
    }

    /// Mark the replicated `player` set as changed (called after any reducer
    /// we send that mutates player rows; SDK row-insert/delete also bump it).
    pub fn mark_players_dirty(&mut self) {
        self.players_dirty
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// True if the replicated `player` table changed since the last poll.
    pub fn poll_players_dirty(&mut self) -> bool {
        let version = self
            .players_dirty
            .load(std::sync::atomic::Ordering::Relaxed);
        let changed = version != self.players_dirty_last;
        self.players_dirty_last = version;
        changed
    }

    /// The hex id (server encoding) at a world position.
    pub fn hex_id_at(x: f32, z: f32) -> u64 {
        let (q, r) = world_pos_to_hex(x, z, WorldGenConfig::HEX_SIZE);
        HexCoord::new(q, r).to_id()
    }
}

/// Register the table/subscription callbacks that mirror the authoritative
/// `player` table into [`Net`] and bump the dirty counter. Shared by the
/// native and wasm connect paths (the latter registers inside the async
/// `spawn_local` once the future resolves).
fn register_net_callbacks(
    conn: &DbConnection,
    tx: Sender<NetEvent>,
    dirty: std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    conn.db.player().on_insert({
        let tx = tx.clone();
        let dirty = dirty.clone();
        move |_ctx, row| {
            let _ = tx.send(NetEvent::ServerMessage(format!(
                "player joined {}",
                row.address
            )));
            dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });
    conn.db.player().on_delete({
        let tx = tx.clone();
        let dirty = dirty.clone();
        move |_ctx, row| {
            let _ = tx.send(NetEvent::ServerMessage(format!(
                "player left {}",
                row.address
            )));
            dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });
    conn.db.player().on_update({
        let dirty = dirty.clone();
        move |_ctx, _old, _new| {
            dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });

    conn.subscription_builder()
        .on_error({
            let tx = tx.clone();
            move |_ctx, err| {
                let _ = tx.send(NetEvent::ConnectError(format!("subscription: {err}")));
            }
        })
        .subscribe_to_all_tables();
}

/// Marker component for spawned remote-player markers.
#[derive(Component)]
pub struct RemotePlayerMarker(pub String);

/// Interpolation state for a remote marker gliding toward its last replicated
/// position (remote rows update every ~2 s).
#[derive(Component, Clone)]
struct MarkerMove {
    target: Vec3,
    speed: f32,
    last_seen: Instant,
}

/// Each frame, glide every remote marker toward its replicated target at the
/// speed it was last observed moving, so players don't teleport every ~2 s.
fn animate_remote_markers(
    time: Res<Time>,
    mut markers: Query<(&mut Transform, &mut MarkerMove), With<RemotePlayerMarker>>,
) {
    let dt = time.delta_secs();
    for (mut transform, interp) in &mut markers {
        let delta = interp.target - transform.translation;
        let dist = delta.length();
        if dist <= 0.001 {
            continue;
        }
        let step = (interp.speed * dt).min(dist);
        transform.translation += delta / dist * step;
    }
}

/// Path of the persisted identity token (same identity across restarts).
#[cfg(not(target_arch = "wasm32"))]
fn identity_token_path() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(".idlebot_client_identity")
}

/// Load the saved access token, if any.
#[cfg(not(target_arch = "wasm32"))]
fn load_saved_token() -> Option<String> {
    let p = identity_token_path();
    let content = std::fs::read_to_string(&p).ok()?;
    let mut lines = content.lines();
    let _identity = lines.next()?;
    let token = lines.next()?.to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Persist `identity` + `token` so reconnects keep the same identity
/// (the server rejects re-binding a wallet to a new identity).
#[cfg(not(target_arch = "wasm32"))]
fn save_token(identity: &str, token: &str) {
    let _ = std::fs::write(identity_token_path(), format!("{identity}\n{token}\n"));
}

/// Delete the saved identity pair (stale after a server re-key).
#[cfg(not(target_arch = "wasm32"))]
fn clear_saved_token() {
    let _ = std::fs::remove_file(identity_token_path());
}

// --- wasm: persist the token in the browser's localStorage ---
#[cfg(target_arch = "wasm32")]
fn load_saved_token() -> Option<String> {
    let storage = web_sys::window()?.local_storage().ok()??;
    let v = storage.get_item("idlebot_token").ok()??;
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[cfg(target_arch = "wasm32")]
fn save_token(identity: &str, token: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item("idlebot_identity", &identity.to_string());
            let _ = storage.set_item("idlebot_token", &token.to_string());
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn clear_saved_token() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item("idlebot_token");
            let _ = storage.remove_item("idlebot_identity");
        }
    }
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Net::default())
            .init_resource::<PositionSync>()
            .add_plugins(super::login::LoginPlugin)
            .add_systems(Startup, auto_connect)
            .add_systems(PreUpdate, (net_frame_tick, net_drain).chain())
            .add_systems(
                Update,
                (
                    animate_remote_markers,
                    send_heartbeat,
                    sync_remote_players,
                    sync_player_position,
                    interact_key_press,
                    super::hud::toggle_debug_hud,
                ),
            );
    }
}

/// Keep the server's online window alive while the game is running: standing
/// still without a heartbeat would let `ONLINE_WINDOW_SECS` expire the player
/// and release their hex to other players.
fn send_heartbeat(time: Res<Time>, net: Res<Net>, mut last: Local<f64>) {
    let now = time.elapsed_secs_f64();
    if now - *last < 30.0 {
        return;
    }
    *last = now;
    if let Some(conn) = net.conn.lock().unwrap().as_ref() {
        let _ = conn.reducers().heartbeat_then(|_ctx, _res| {});
    }
}

/// Server-side `Plant` serialised on the hex row (mirror of `types.rs`).
#[derive(serde::Deserialize)]
struct HexPlant {
    plant_type: String,
    planted_at: u64,
    growth_time: u64,
}

/// Spec 004 T5.1 / Spec 022 §5 — `E` acts on the selected slot, routed by
/// held item + slot contents:
///   1. held tool → tool action (Pickaxe→Rock, Axe→Tree, Shovel→Grass tuft,
///      Hoe→till Wheat into the empty plot)
///   2. craft bench on the slot → open the craft menu
///   3. gather node nearest the slot (grass/rock/log/mature tree)
///   4. harvest crop → clean pollution
///   5. plant by held item: Seed → tree, Grass → grass tuft, else carrying
///      4+ logs → build a craft bench
fn interact_key_press(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut net: ResMut<Net>,
    mut craft_menu: ResMut<crate::net::craft::CraftMenu>,
    player: Option<Query<&ClientPlayer>>,
    target: Res<crate::world_floor::ActionTarget>,
    inventory: Res<crate::inventory::Inventory>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }
    // Ground actions aim at the Stardew-style targeting box, not the feet.
    let hex_id = idlecore_core::hex::HexCoord::new(target.q, target.r).to_id();
    if let Some(_p) = player.as_ref().and_then(|q| q.single().ok()) {
        let tx = net.sender();
        let now = now_unix_secs();

        // Every replicated node on the hex, ranked by distance to the
        // targeted slot (Stardew-style aiming).
        let (hex_cx, hex_cy) = idlecore_core::hex_grid::HexGrid::axial_to_world(
            target.q,
            target.r,
            idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
        );
        let slot_dx = idlecore_core::slots::slot_center(target.slot_x, target.slot_y).0 - hex_cx;
        let slot_dy = idlecore_core::slots::slot_center(target.slot_x, target.slot_y).1 - hex_cy;
        // Scope the connection lock to just the read so later `&mut net`
        // reducer sends don't conflict with a held guard.
        let nodes: Vec<(f32, u64, String, bool)> = {
            let guard = net.conn.lock().unwrap();
            let Some(conn) = guard.as_ref() else {
                net.push(NetEvent::ServerMessage("E: not connected".to_string()));
                return;
            };
            let mut nodes = Vec::new();
            for obj in conn.db.world_object().iter().filter(|o| o.hex_id == hex_id) {
                let dx = obj.offset_x - slot_dx;
                let dy = obj.offset_y - slot_dy;
                let dist2 = dx * dx + dy * dy;
                let mature = obj.mature_at == 0 || now >= obj.mature_at;
                let kind = match &obj.kind[..] {
                    k @ ("Grass" | "Rock" | "Tree" | "Log" | "CraftBench") => k,
                    _ => continue,
                };
                nodes.push((dist2, obj.object_id, kind.to_string(), mature));
            }
            nodes
        };
        // A node is "on the selected slot" only if it sits inside the targeted
        // cell — this keeps gather/harvest honoured to the plot you aim at
        // (Spec 022 §5 selected-plot invariant), so planting an empty slot
        // never gets hijacked by a nearby growing node.
        let half_slot = idlecore_core::slots::SLOT_SIZE * 0.5;
        let nearest = |kind: &str| -> Option<(u64, bool)> {
            nodes
                .iter()
                .filter(|(_, _, k, _)| k == kind)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id, _, m)| (*id, *m))
        };

        // --- 1. Held tools act on the selected slot's contents (Spec 022 §5).
        let held = inventory.active_item().cloned();
        match held.as_deref() {
            Some("Pickaxe") => {
                match nearest("Rock") {
                    Some((object_id, _)) => super::hud::send_reducer(&mut net, |r| {
                        r.gather_object_then(
                            object_id,
                            super::hud::reducer_report("mine", tx.clone(), hex_id),
                        )
                    }),
                    None => net.push(NetEvent::ServerMessage("E: no rock here to mine".to_string())),
                }
                return;
            }
            Some("Axe") => {
                match nearest("Tree") {
                    Some((object_id, true)) => super::hud::send_reducer(&mut net, |r| {
                        r.gather_object_then(
                            object_id,
                            super::hud::reducer_report("harvest_tree", tx.clone(), hex_id),
                        )
                    }),
                    Some((_, false)) => {
                        net.push(NetEvent::ServerMessage("E: tree still growing".to_string()))
                    }
                    None => net.push(NetEvent::ServerMessage("E: no tree here to chop".to_string())),
                }
                return;
            }
            Some("Shovel") => {
                match nearest("Grass") {
                    Some((object_id, true)) => super::hud::send_reducer(&mut net, |r| {
                        r.gather_object_then(
                            object_id,
                            super::hud::reducer_report("gather", tx.clone(), hex_id),
                        )
                    }),
                    Some((_, false)) => {
                        net.push(NetEvent::ServerMessage("E: grass still growing".to_string()))
                    }
                    None => net.push(NetEvent::ServerMessage("E: no grass here to dig".to_string())),
                }
                return;
            }
            Some("Hoe") => {
                // Till the empty plot: the targeted cell must hold no node.
                let cell_free = !nodes.iter().any(|(d, _, _, _)| *d < half_slot * half_slot);
                if cell_free {
                    super::hud::send_reducer(&mut net, |r| {
                        r.till_then(
                            hex_id,
                            super::hud::reducer_report("till", tx.clone(), hex_id),
                        )
                    });
                } else {
                    net.push(NetEvent::ServerMessage("E: clear the plot before tilling".to_string()));
                }
                return;
            }
            _ => {}
        }

        // --- 2. A craft bench on the targeted slot opens the craft menu.
        if nodes
            .iter()
            .any(|(d, _, k, _)| k == "CraftBench" && *d < half_slot * half_slot)
        {
            craft_menu.open_at(hex_id, target.slot_x, target.slot_y, time.elapsed_secs_f64());
            net.push(NetEvent::ServerMessage("E: craft bench — fill the grid".to_string()));
            return;
        }

        // --- 3. Gather the node on the selected slot (never another cell).
        //        Priority within the cell: mature tree > grass/rock/log.
        let rank = |kind: &str, mature: bool| match (kind, mature) {
            ("Tree", true) => 0,
            ("Tree", false) => 2,
            _ => 1,
        };
        let node = nodes
            .iter()
            .filter(|(d, _, k, _)| *k != "CraftBench" && *d < half_slot * half_slot)
            .min_by(|a, b| {
                let ka = rank(&a.2, a.3);
                let kb = rank(&b.2, b.3);
                ka.cmp(&kb).then(a.0.total_cmp(&b.0))
            })
            .map(|(_, id, kind, mature)| (*id, kind.clone(), *mature));
        if let Some((object_id, kind, mature)) = node {
            if kind == "Tree" && inventory.counts.get("Axe").copied().unwrap_or(0) == 0 {
                net.push(NetEvent::ServerMessage(
                    "E: need an Axe to chop — gather logs, build a bench, then craft one".to_string(),
                ));
                return;
            }
            if !mature {
                net.push(NetEvent::ServerMessage(format!("E: {kind} still growing")));
                return;
            }
            let action = match kind.as_str() {
                "Grass" => "gather",
                "Rock" => "mine",
                "Log" => "gather_log",
                _ => "harvest_tree",
            };
            super::hud::send_reducer(&mut net, |r| {
                r.gather_object_then(
                    object_id,
                    super::hud::reducer_report(action, tx.clone(), hex_id),
                )
            });
            return;
        }
        let hex_opt = {
            let guard = net.conn.lock().unwrap();
            let Some(conn) = guard.as_ref() else {
                net.push(NetEvent::ServerMessage("E: not connected".to_string()));
                return;
            };
            conn.db.hex_tile().hex_id().find(&hex_id)
        };
        let Some(hex) = hex_opt else {
            net.push(NetEvent::ServerMessage(format!(
                "E: hex {hex_id} not found"
            )));
            return;
        };
        // --- 4. Harvest a mature crop or clean pollution (hex-level).
        if let Some(plant_json) = &hex.plant {
            match serde_json::from_str::<HexPlant>(plant_json) {
                Ok(plant) => {
                    if now >= plant.planted_at + plant.growth_time {
                        super::hud::send_reducer(&mut net, |r| {
                            r.harvest_then(
                                hex_id,
                                super::hud::reducer_report("harvest", tx.clone(), hex_id),
                            )
                        });
                    } else {
                        net.push(NetEvent::ServerMessage(format!(
                            "E: {} still growing ({}s left)",
                            plant.plant_type,
                            plant.planted_at + plant.growth_time - now
                        )));
                    }
                }
                Err(_) => net.push(NetEvent::ServerMessage("E: corrupt plant data".to_string())),
            }
            return;
        }
        if hex.is_polluted {
            super::hud::send_reducer(&mut net, |r| {
                r.clean_then(
                    hex_id,
                    super::hud::reducer_report("clean", tx.clone(), hex_id),
                )
            });
            return;
        }
        // --- 5. Plant by held item on the empty plot (Spec 022 §1/§3).
        let (slot_x, slot_y) = (target.slot_x, target.slot_y);
        match held.as_deref() {
            Some("Seed") => super::hud::send_reducer(&mut net, |r| {
                r.plant_tree_then(
                    hex_id,
                    slot_x,
                    slot_y,
                    super::hud::reducer_report("plant_tree", tx.clone(), hex_id),
                )
            }),
            Some("Grass") => super::hud::send_reducer(&mut net, |r| {
                r.plant_grass_then(
                    hex_id,
                    slot_x,
                    slot_y,
                    super::hud::reducer_report("plant_grass", tx.clone(), hex_id),
                )
            }),
            _ if inventory.counts.get("Log").copied().unwrap_or(0) >= 4 => {
                // Carrying 4+ logs and nothing else to do: build the bench
                // (placing IS building — Spec 022 §3).
                super::hud::send_reducer(&mut net, |r| {
                    r.place_craft_bench_then(
                        hex_id,
                        slot_x,
                        slot_y,
                        super::hud::reducer_report("place_craft_bench", tx.clone(), hex_id),
                    )
                });
            }
            _ => {
                net.push(NetEvent::ServerMessage(format!(
                    "E: nothing to interact with on {}",
                    hex.terrain
                )));
            }
        }
    }
}

/// Connect and log in automatically at startup (no button needed).
fn auto_connect(mut net: ResMut<Net>) {
    net.log_line("auto-connecting...");
    net.connect();
}

/// Advance the SDK connection every frame: pumps the websocket and fires the
/// row/reducer callbacks on the main thread. Native builds run no network
/// thread unless `run_threaded` is used, so without this tick the connection
/// is built but never progresses (on_connect/subscription never fire).
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
fn net_frame_tick(net: Res<Net>) {
    // Native pumps the connection from the main thread; on wasm the
    // `run_background_task` spawned during `connect` drives it instead.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(conn) = net.conn.lock().unwrap().as_ref() {
        let _ = conn.frame_tick();
    }
}

/// Move events off the queue before other systems read the resource.
fn net_drain(
    mut net: ResMut<Net>,
    mut burst: ResMut<crate::assets::BurstFx>,
    mut latency: ResMut<ServerLatency>,
    mut player_transform: ResMut<crate::player::PlayerTransform>,
    mut bodies: Query<
        (&mut Transform, &mut crate::player::ClientPlayer),
        With<crate::plugins::player::PhysicsBody>,
    >,
) {
    let teleports = net.drain();
    if teleports.is_empty() {
        return;
    }
    let Ok((mut transform, mut player)) = bodies.single_mut() else {
        return;
    };
    for (q, r) in teleports {
        let (wx, wy) =
            idlecore_core::hex_grid::HexGrid::axial_to_world(q, r, WorldGenConfig::HEX_SIZE);
        // 2D world: x = east, y = north; z carries the draw order.
        transform.translation = Vec3::new(wx, wy, crate::world_floor::prop_depth(wy) + 50.0);
        player.position = Vec3::new(wx, wy, 0.0);
        player.current_hex = Some(crate::player::CurrentHex { q, r });
        player_transform.translation = transform.translation;
        burst.request(Vec3::new(wx, wy, 0.0));
        latency.resolve();
    }
}

/// Coarse position persistence: every few seconds, forward the movement
/// since the last send to the server via `move_player`, so reconnects can
/// restore the player where they left off. The server caps speed/distance,
/// so this is safe to call at any cadence.
#[derive(Resource)]
struct PositionSync {
    last_send: Instant,
    last_pos: Option<Vec2>,
}

impl Default for PositionSync {
    fn default() -> Self {
        Self {
            last_send: Instant::now(),
            last_pos: None,
        }
    }
}

const POSITION_SYNC_INTERVAL: f32 = 2.0;

/// Pure: reduce a movement delta since the last sync into a direction and
/// speed for `move_player`, or `None` when the player barely moved.
fn movement_report(prev: Vec2, cur: Vec2, dt: f32) -> Option<(Vec2, f32)> {
    let delta = cur - prev;
    if delta.length() < 0.5 {
        return None;
    }
    let dir = delta / delta.length();
    Some((dir, delta.length() / dt.max(0.001)))
}

fn sync_player_position(
    mut net: ResMut<Net>,
    mut state: ResMut<PositionSync>,
    bodies: Query<(&Transform, &ClientPlayer), (With<PhysicsBody>, Without<RemotePlayerMarker>)>,
) {
    let Ok((body, player)) = bodies.single() else {
        return;
    };
    if net.conn.lock().unwrap().is_none() || net.address.is_none() {
        return;
    }
    // Don't push local positions before the authoritative row snapped us
    // back to the persisted spot — otherwise a spawn-position delta would
    // be sent as phantom movement and drag the saved position off.
    if !player.position_restored {
        return;
    }
    if state.last_send.elapsed().as_secs_f32() < POSITION_SYNC_INTERVAL {
        return;
    }
    let dt = state.last_send.elapsed().as_secs_f32().max(0.001);
    state.last_send = Instant::now();

    let pos = Vec2::new(body.translation.x, body.translation.y);
    let Some(prev) = state.last_pos else {
        state.last_pos = Some(pos);
        return;
    };
    state.last_pos = Some(pos);

    let Some((dir, speed)) = movement_report(prev, pos, dt) else {
        return;
    };
    // Log the hex the movement ended in (pos is the reported destination).
    let hex_id = Net::hex_id_at(pos.x, pos.y);
    let tx = net.sender();
    send_reducer(&mut net, |r| {
        r.move_player_then(
            dir.x,
            dir.y,
            speed,
            dt,
            pos.x,
            pos.y,
            reducer_report("move", tx.clone(), hex_id),
        )
    });
}

/// Mirror the authoritative `player` table into `Net.players`, spawn 2D
/// markers for remote players, and keep the local `ClientPlayer` resource in
/// sync with the server row for our wallet.
fn sync_remote_players(
    mut net: ResMut<Net>,
    mut player: Query<&mut ClientPlayer>,
    mut bodies: Query<&mut Transform, (With<PhysicsBody>, Without<RemotePlayerMarker>)>,
    mut skins: ResMut<crate::skins::PlayerSkins>,
    mut player_transform: ResMut<crate::player::PlayerTransform>,
    mut pos_sync: ResMut<PositionSync>,
    mut commands: Commands,
    mut markers: Query<(
        Entity,
        &RemotePlayerMarker,
        &mut Transform,
        Option<&mut MarkerMove>,
    )>,
) {
    let Some(mine) = net.address.clone() else {
        return;
    };

    // Perf: rebuild is O(rows * markers); skip it entirely unless the
    // replicated player set changed (row callbacks or reducers we sent).
    // The first frame after connect always runs (players map is empty).
    if !net.poll_players_dirty() && !net.players.is_empty() {
        return;
    }
    // Spec 018 T2.4: only players within 3 hexes are visible.
    let own_hex: Option<(i32, i32)> = player
        .single()
        .ok()
        .and_then(|p| p.current_hex.map(|h| (h.q, h.r)))
        .or_else(|| {
            player.single().ok().map(|p| {
                idlecore_core::hex::world_pos_to_hex(
                    p.position.x,
                    p.position.y,
                    WorldGenConfig::HEX_SIZE,
                )
            })
        });
    let in_view = |row_hex_id: u64| -> bool {
        let Some((q, r)) = own_hex else { return true };
        let rc = idlecore_core::hex::HexCoord::from_id(row_hex_id);
        axial_hex_distance((q, r), (rc.q, rc.r)) <= 3
    };

    let rows: Vec<Player> = {
        let conn_guard = net.conn.lock().unwrap();
        let Some(conn) = conn_guard.as_ref() else {
            return;
        };
        conn.db.player().iter().collect()
    };
    let tx = net.sender();
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for row in &rows {
        if row.address.eq_ignore_ascii_case(mine.as_str()) {
            // Authoritative state for our own wallet → mirror into the local sim.
            if let Ok(mut p) = player.single_mut() {
                let old_level = p.level;
                p.gold = row.gold;
                p.usdt = row.usdt;
                p.xp = row.total_xp;
                p.level = row.level;
                p.eco_points = row.eco_points as u64;
                // Spec 006 T6.3: restore equipped vehicle from the authoritative row.
                p.owned_vehicle = vehicle_from_str(&row.vehicle);
                // Spec 014 FR4: avatar drives the local character's skin.
                p.avatar = row.avatar.clone();
                // Persisted look: the avatar column stores a skin file name.
                crate::skins::set_skin_by_name(&mut skins, &row.avatar);
                // Persisted spawn: once per session, land on the row position.
                if !p.position_restored {
                    if let Ok(mut t) = bodies.single_mut() {
                        t.translation = Vec3::new(
                            row.position_x,
                            row.position_y,
                            crate::world_floor::prop_depth(row.position_y) + 50.0,
                        );
                    }
                    p.position = Vec3::new(row.position_x, row.position_y, 0.0);
                    p.current_hex = Some(crate::player::CurrentHex {
                        q: row.hex_q,
                        r: row.hex_r,
                    });
                    player_transform.translation = p.position;
                    p.position_restored = true;
                    // Baseline the position sync at the restored spot so the
                    // next delta reflects real movement only.
                    pos_sync.last_send = Instant::now();
                    pos_sync.last_pos = Some(Vec2::new(row.position_x, row.position_y));
                    let _ = tx.send(NetEvent::ServerMessage(format!(
                        "session restored: position ({:.0},{:.0}) hex ({},{})",
                        row.position_x, row.position_y, row.hex_q, row.hex_r
                    )));
                }
                if row.level > old_level {
                    let _ = tx.send(NetEvent::ServerMessage(format!(
                        "LEVEL UP! Now level {}",
                        row.level
                    )));
                }
            }
            continue;
        }
        if !in_view(row.hex_id) || row.status != "online" {
            // Outside the view radius — or an offline session: no marker.
            // Frozen ghosts of old sessions read as dark squares lurking
            // around the spawn. Keep the row known so existing markers for
            // it despawn cleanly.
            known.insert(row.address.clone());
            continue;
        }
        known.insert(row.address.clone());
        net.players.insert(
            row.address.clone(),
            ServerPlayerSnapshot {
                x: row.position_x,
                y: row.position_y,
                online: row.status == "online",
            },
        );
        // 2D: x = east, y = north; z = draw order (above tiles, below the
        // local player's +50 offset).
        let pos = Vec3::new(
            row.position_x,
            row.position_y,
            crate::world_floor::prop_depth(row.position_y) + 40.0,
        );
        let mut found = false;
        for (entity, marker, transform, interp_slot) in markers.iter_mut() {
            let _ = entity;
            if marker.0 == row.address {
                // Remote rows update every ~2 s; glide toward the new spot
                // instead of snapping, so movement stays smooth.
                let now = Instant::now();
                let mut interp = match &interp_slot {
                    Some(i) => (**i).clone(),
                    None => MarkerMove {
                        target: pos,
                        speed: 0.0,
                        last_seen: now,
                    },
                };
                let dist = (pos - transform.translation).length();
                let since = now
                    .saturating_duration_since(interp.last_seen)
                    .as_secs_f32()
                    .max(0.05);
                interp.speed = dist / since;
                interp.target = pos;
                interp.last_seen = now;
                if let Some(mut slot) = interp_slot {
                    *slot = interp;
                } else {
                    commands.entity(entity).insert(interp);
                }
                found = true;
                break;
            }
        }
        if found {
            continue;
        }
        let color = if row.status == "online" {
            Color::srgb(0.2, 0.8, 1.0)
        } else {
            Color::srgb(0.4, 0.4, 0.5)
        };
        commands.spawn((
            RemotePlayerMarker(row.address.clone()),
            MarkerMove {
                target: pos,
                speed: 0.0,
                last_seen: Instant::now(),
            },
            Sprite {
                color,
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            Transform::from_translation(pos),
        ));
    }

    net.players.retain(|k, _| known.contains(k));

    // Despawn markers for players no longer in the table.
    for (entity, marker, _, _) in markers.iter_mut() {
        if !known.contains(&marker.0) {
            commands.entity(entity).despawn();
        }
    }
}

/// Map a server-side vehicle string back to the core Vehicle enum.
fn vehicle_from_str(s: &str) -> Option<idlecore_core::Vehicle> {
    match s {
        "Car" | "Bicycle" => Some(idlecore_core::Vehicle::Bicycle),
        "Scooter" => Some(idlecore_core::Vehicle::Scooter),
        "Motorcycle" => Some(idlecore_core::Vehicle::Motorcycle),
        "Boat" => Some(idlecore_core::Vehicle::Boat),
        "Airplane" => Some(idlecore_core::Vehicle::Airplane),
        _ => None,
    }
}

/// Axial hex-grid distance (max of the three cube coordinates).
fn axial_hex_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dq = (a.0 - b.0).abs();
    let dr = (a.1 - b.1).abs();
    let ds = (a.0 + a.1 - b.0 - b.1).abs();
    dq.max(dr).max(ds)
}

// ============================================================================
// Server latency instrumentation (Spec 018 T6.2)
// ============================================================================

/// Rolling window of server round-trip samples in milliseconds.
#[derive(Debug, Clone)]
pub struct LatencyWindow {
    samples: VecDeque<f32>,
    max: usize,
}

impl Default for LatencyWindow {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
            max: 60,
        }
    }
}

impl LatencyWindow {
    pub fn push_ms(&mut self, ms: f32) {
        if self.samples.len() == self.max {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
    }

    pub fn avg_ms(&self) -> Option<f32> {
        if self.samples.is_empty() {
            return None;
        }
        Some(self.samples.iter().sum::<f32>() / self.samples.len() as f32)
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Most recent measured RTT (used by tests).
    #[cfg(test)]
    pub fn latest_ms(&self) -> Option<f32> {
        self.samples.back().copied()
    }
}

/// RTT of a teleport round trip: request sent → server-confirmed arrival.
#[derive(Resource, Default)]
pub struct ServerLatency {
    pub window: LatencyWindow,
    request_sent_at: Option<Instant>,
}

impl ServerLatency {
    /// Call when the teleport reducer is invoked (request timestamp).
    pub fn note_request(&mut self) {
        self.request_sent_at = Some(Instant::now());
    }

    /// Call when the server-confirmed teleport arrives; returns the sample.
    pub fn resolve(&mut self) -> Option<f32> {
        let sent = self.request_sent_at.take()?;
        let ms = sent.elapsed().as_secs_f32() * 1000.0;
        self.window.push_ms(ms);
        Some(ms)
    }
}

#[cfg(test)]
mod latency_tests {
    use super::*;

    #[test]
    fn window_averages_and_rolls_off() {
        let mut w = LatencyWindow {
            max: 3,
            ..Default::default()
        };
        w.push_ms(10.0);
        w.push_ms(20.0);
        w.push_ms(30.0);
        assert_eq!(w.sample_count(), 3);
        assert_eq!(w.avg_ms().unwrap(), 20.0);
        assert_eq!(w.latest_ms().unwrap(), 30.0);
        w.push_ms(60.0); // rolls off the 10.0
        assert_eq!(w.sample_count(), 3);
        assert_eq!(w.avg_ms().unwrap(), 110.0 / 3.0);
    }

    #[test]
    fn empty_window_has_no_avg() {
        let w = LatencyWindow::default();
        assert_eq!(w.avg_ms(), None);
        assert_eq!(w.latest_ms(), None);
    }

    #[test]
    fn resolve_requires_request() {
        let mut latency = ServerLatency::default();
        assert_eq!(latency.resolve(), None);
        latency.note_request();
        assert!(latency.resolve().is_some());
        assert_eq!(latency.resolve(), None); // second call has nothing pending
        assert_eq!(latency.window.sample_count(), 1);
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn vehicle_from_str_maps_all_vehicles() {
        use idlecore_core::Vehicle;
        assert_eq!(vehicle_from_str("Bicycle"), Some(Vehicle::Bicycle));
        assert_eq!(vehicle_from_str("Scooter"), Some(Vehicle::Scooter));
        assert_eq!(vehicle_from_str("Motorcycle"), Some(Vehicle::Motorcycle));
        assert_eq!(vehicle_from_str("Boat"), Some(Vehicle::Boat));
        assert_eq!(vehicle_from_str("Airplane"), Some(Vehicle::Airplane));
        assert_eq!(vehicle_from_str("None"), None);
        assert_eq!(vehicle_from_str("bicycle"), None); // case-sensitive
        assert_eq!(vehicle_from_str(""), None);
    }

    #[test]
    fn axial_hex_distance_matches_cube_math() {
        // (0,0) to (3,0): 3 steps along +q.
        assert_eq!(axial_hex_distance((0, 0), (3, 0)), 3);
        // (0,0) to (2,-2): cube s unchanged (0 vs 0) -> max(2,2,0) = 2.
        assert_eq!(axial_hex_distance((0, 0), (2, -2)), 2);
        assert_eq!(axial_hex_distance((5, 3), (5, 3)), 0);
        // Symmetric.
        assert_eq!(
            axial_hex_distance((2, -2), (0, 0)),
            axial_hex_distance((0, 0), (2, -2))
        );
        // Adjacent hexes.
        assert_eq!(axial_hex_distance((0, 0), (1, 0)), 1);
        assert_eq!(axial_hex_distance((0, 0), (1, -1)), 1);
    }

    #[test]
    fn movement_report_reduces_delta() {
        let (dir, speed) = movement_report(Vec2::new(0.0, 0.0), Vec2::new(3.0, 4.0), 1.0).unwrap();
        assert!((dir - Vec2::new(0.6, 0.8)).length() < 1e-5);
        assert!((speed - 5.0).abs() < 1e-5);
    }

    #[test]
    fn movement_report_ignores_jitter() {
        assert!(movement_report(Vec2::new(0.0, 0.0), Vec2::new(0.2, 0.1), 2.0).is_none());
        assert!(movement_report(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0), 2.0).is_none());
    }

    #[test]
    fn movement_report_scales_speed_with_dt() {
        let (_, speed) = movement_report(Vec2::ZERO, Vec2::new(6.0, 0.0), 2.0).unwrap();
        assert!((speed - 3.0).abs() < 1e-5);
        let (_, speed) = movement_report(Vec2::ZERO, Vec2::new(6.0, 0.0), 0.5).unwrap();
        assert!((speed - 12.0).abs() < 1e-5);
    }
}
