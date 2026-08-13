//! SpacetimeDB connection plugin.
//!
//! Owns the `DbConnection` (idlebot module, local node), advances it every
//! frame via `frame_tick`, mirrors the authoritative `player` table into
//! [`Net`], renders other players as markers, and exposes reducer actions to
//! the rest of the client.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use spacetimedb_sdk::DbContext;
use spacetimedb_sdk::__codegen::{TableLike, WithDelete, WithInsert};

use idlecore_core::hex::{world_pos_to_hex, HexCoord};
use idlecore_core::world_gen::WorldGenConfig;
use crate::player::ClientPlayer;

use super::gen::*;

/// Node address of the local SpacetimeDB instance.
pub const SERVER_URI: &str = "http://127.0.0.1:3000";
/// Module (database) name published by `idlecore-server`.
pub const MODULE_NAME: &str = "idlebot";
/// Demo wallet used until chain verification lands (Spec 013/014).
pub const DEMO_WALLET: &str = "0xIdleBotDemo0001";

/// One-way messages from the SDK callbacks (network thread) to the main
/// thread, drained by [`Net::drain`] each frame.
#[derive(Debug, Clone)]
pub enum NetEvent {
    Connected { identity: String },
    ConnectError(String),
    Disconnected(String),
    ReducerResult { name: &'static str, ok: bool, msg: String },
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
    pub address: String,
    pub x: f32,
    pub y: f32,
    pub hex_id: u64,
    pub level: u32,
    pub vehicle: String,
    pub cosmetics: String,
    pub online: bool,
}

/// World-level network state resource.
#[derive(Resource)]
pub struct Net {
    pub status: NetStatus,
    pub conn: Option<DbConnection>,
    /// Our bound wallet address (set after a successful `login`).
    pub address: Option<String>,
    /// Our identity as issued by the server.
    pub identity: String,
    /// Snapshot of every player row we know about (including ourselves).
    pub players: HashMap<String, ServerPlayerSnapshot>,
    /// Recent server/connection messages for the HUD log.
    pub log: VecDeque<String>,
    /// Timestamp of the last reducer send (movement throttling).
    last_move_send: std::time::Instant,
    tx: Sender<NetEvent>,
    rx: std::sync::Mutex<Receiver<NetEvent>>,
}

impl Default for Net {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            status: NetStatus::Disconnected,
            conn: None,
            address: None,
            identity: String::new(),
            players: HashMap::new(),
            log: VecDeque::new(),
            last_move_send: std::time::Instant::now(),
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
    pub fn drain(&mut self) {
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
                    // Reducers can only be sent once the handshake is done,
                    // so log in from here rather than on button click.
                    // The server normalises wallet addresses to lowercase.
                    self.address = Some(DEMO_WALLET.to_lowercase());
                    if let Some(conn) = &self.conn {
                        let tx = self.tx.clone();
                        let _ = conn.reducers.login_then(DEMO_WALLET.to_string(), {
                            move |_ctx, res| {
                                let (ok, msg) = match &res {
                                    Ok(Ok(())) => (true, format!("logged in as {DEMO_WALLET}")),
                                    Ok(Err(e)) => (false, e.clone()),
                                    Err(e) => (false, format!("send error: {e}")),
                                };
                                let _ = tx.send(NetEvent::ReducerResult { name: "login", ok, msg });
                            }
                        });
                    }
                }
                NetEvent::ConnectError(msg) => {
                    self.status = NetStatus::Error(msg.clone());
                    self.log_line(&format!("Connect error: {msg}"));
                }
                NetEvent::Disconnected(reason) => {
                    self.status = NetStatus::Disconnected;
                    self.log_line(&format!("Disconnected: {reason}"));
                }
                NetEvent::ReducerResult { name, ok, msg } => {
                    let line = if ok {
                        format!("{name}: {msg}")
                    } else {
                        format!("{name} FAILED: {msg}")
                    };
                    self.log_line(&line);
                }
                NetEvent::ServerMessage(msg) => self.log_line(&msg),
            }
        }
    }

    fn log_line(&mut self, line: &str) {
        self.log.push_back(line.to_string());
        while self.log.len() > 6 {
            self.log.pop_front();
        }
    }

    /// Open the connection and start receiving (call once).
    pub fn connect(&mut self) {
        if !matches!(self.status, NetStatus::Disconnected) {
            return;
        }
        self.status = NetStatus::Connecting;
        let saved = load_saved_token();
        let conn = match DbConnection::builder()
            .with_uri(SERVER_URI)
            .with_database_name(MODULE_NAME)
            .with_token(saved)
            .on_connect({
                let tx = self.tx.clone();
                move |_ctx, identity, token| {
                    save_token(&identity.to_string(), token);
                    tx.send(NetEvent::Connected { identity: identity.to_string() }).ok();
                }
            })
            .on_connect_error({
                let tx = self.tx.clone();
                move |_ctx, err| {
                    tx.send(NetEvent::ConnectError(err.to_string())).ok();
                }
            })
            .on_disconnect({
                let tx = self.tx.clone();
                move |_ctx, reason| {
                    tx.send(NetEvent::Disconnected(format!("{reason:?}"))).ok();
                }
            })
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                self.status = NetStatus::Error(e.to_string());
                return;
            }
        };

        // Table callbacks → events.
        conn.db.player().on_insert({
            let tx = self.tx.clone();
            move |_ctx, row| {
                let _ = tx.send(NetEvent::ServerMessage(format!("player joined {}", row.address)));
            }
        });
        conn.db.player().on_delete({
            let tx = self.tx.clone();
            move |_ctx, row| {
                let _ = tx.send(NetEvent::ServerMessage(format!("player left {}", row.address)));
            }
        });

        // Subscribe to everything the module exposes.
        conn.subscription_builder()
            .on_error({
                let tx = self.tx.clone();
                move |_ctx, err| {
                    let _ = tx.send(NetEvent::ConnectError(format!("subscription: {err}")));
                }
            })
            .subscribe_to_all_tables();

        self.conn = Some(conn);
    }

    /// Best-effort send of a reducer; result arrives as `NetEvent::ReducerResult`.
    pub fn invoke(&self, f: impl FnOnce(&RemoteReducers) -> Result<(), spacetimedb_sdk::Error>) {
        if let Some(conn) = &self.conn {
            let _ = f(&conn.reducers);
        }
    }

    /// Send movement to the server at most every ~0.75 s (throttled).
    pub fn sync_movement(&mut self, dir_x: f32, dir_y: f32, speed: f32) {
        if self.last_move_send.elapsed().as_secs_f32() < 0.75 {
            return;
        }
        self.last_move_send = std::time::Instant::now();
        if self.address.is_none() {
            return;
        }
        if let Some(conn) = &self.conn {
            let _ = conn
                .reducers
                .move_player_then(dir_x, dir_y, speed, 0.75, |_ctx, _res| {});
        }
    }

    /// The hex id (server encoding) at a world position.
    pub fn hex_id_at(x: f32, z: f32) -> u64 {
        let (q, r) = world_pos_to_hex(x, z, WorldGenConfig::HEX_SIZE);
        HexCoord::new(q, r).to_id()
    }
}

/// Marker component for spawned remote-player markers.
#[derive(Component)]
pub struct RemotePlayerMarker(pub String);

/// Path of the persisted identity token (same identity across restarts).
fn identity_token_path() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    )
    .join(".idlebot_client_identity")
}

/// Load the saved access token, if any.
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
fn save_token(identity: &str, token: &str) {
    let _ = std::fs::write(identity_token_path(), format!("{identity}\n{token}\n"));
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Net::default())
            .add_systems(Startup, auto_connect)
            .add_systems(PreUpdate, net_drain)
            .add_systems(Update, (net_advance, sync_remote_players, interact_key_press));
    }
}

/// Server-side `Plant` serialised on the hex row (mirror of `types.rs`).
#[derive(serde::Deserialize)]
struct HexPlant {
    plant_type: String,
    planted_at: u64,
    growth_time: u64,
}

/// Spec 004 T5.1 — `E` performs a context action on the hex under the player:
/// harvest a mature crop, clean pollution, or plant Wheat on empty grass.
fn interact_key_press(
    keyboard: Res<ButtonInput<KeyCode>>,
    net: Res<Net>,
    player: Option<Query<&ClientPlayer>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }
    if let Some(p) = player.as_ref().and_then(|q| q.single().ok()) {
        let hex_id = Net::hex_id_at(p.position.x, p.position.z);
        let Some(conn) = net.conn.as_ref() else {
            net.push(NetEvent::ServerMessage("E: not connected".to_string()));
            return;
        };
        let tx = net.sender();
        let Some(hex) = conn.db.hex_tile().hex_id().find(&hex_id) else {
            net.push(NetEvent::ServerMessage(format!("E: hex {hex_id} not found")));
            return;
        };
        if let Some(plant_json) = &hex.plant {
            match serde_json::from_str::<HexPlant>(plant_json) {
                Ok(plant) => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if now >= plant.planted_at + plant.growth_time {
                        super::hud::send_reducer(conn, &tx, |r| {
                            r.harvest_then(hex_id, super::hud::reducer_report("harvest", tx.clone(), hex_id))
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
        } else if hex.is_polluted {
            super::hud::send_reducer(conn, &tx, |r| {
                r.clean_then(hex_id, super::hud::reducer_report("clean", tx.clone(), hex_id))
            });
        } else if hex.terrain == "Grass" || hex.terrain == "Forest" {
            super::hud::send_reducer(conn, &tx, |r| {
                r.plant_then(
                    hex_id,
                    "Wheat".to_string(),
                    super::hud::reducer_report("plant", tx.clone(), hex_id),
                )
            });
        } else {
            net.push(NetEvent::ServerMessage(format!("E: cannot interact on {}", hex.terrain)));
        }
    }
}

/// Connect and log in automatically at startup (no button needed).
fn auto_connect(mut net: ResMut<Net>) {
    net.log_line("auto-connecting...");
    net.connect();
}

/// Move events off the queue before other systems read the resource.
fn net_drain(mut net: ResMut<Net>) {
    net.drain();
}

/// Pump the SDK connection (processes incoming messages and pending sends).
fn net_advance(net: ResMut<Net>) {
    if let Some(conn) = &net.conn {
        let _ = conn.frame_tick();
    }
}

/// Mirror the authoritative `player` table into `Net.players`, spawn 3D
/// markers for remote players, and keep the local `ClientPlayer` resource in
/// sync with the server row for our wallet.
fn sync_remote_players(
    mut net: ResMut<Net>,
    mut player: Query<&mut ClientPlayer>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut markers: Query<(Entity, &RemotePlayerMarker, &mut Transform)>,
) {
    let Some(conn) = net.conn.as_ref() else { return };
    let Some(mine) = net.address.clone() else { return };

    let rows: Vec<Player> = conn.db.player().iter().collect();
    let mut known: Vec<String> = Vec::new();
    for row in &rows {
        if row.address == *mine {
            // Authoritative state for our own wallet → mirror into the local sim.
            if let Ok(mut p) = player.single_mut() {
                p.gold = row.gold;
                p.usdt = row.usdt;
                p.xp = row.total_xp;
                p.level = row.level;
                p.eco_points = row.eco_points as u64;
            }
            continue;
        }
        known.push(row.address.clone());
        net.players.insert(
            row.address.clone(),
            ServerPlayerSnapshot {
                address: row.address.clone(),
                x: row.position_x,
                y: row.position_y,
                hex_id: row.hex_id,
                level: row.level,
                vehicle: row.vehicle.clone(),
                cosmetics: row.cosmetics.clone(),
                online: row.status == "online",
            },
        );
        let pos = Vec3::new(row.position_x, 0.4, row.position_y);
        let mut found = false;
        for (entity, marker, mut transform) in markers.iter_mut() {
            let _ = entity;
            if marker.0 == row.address {
                transform.translation = pos;
                found = true;
                break;
            }
        }
        if found {
            continue;
        }
        let mat = if row.status == "online" {
            materials.add(StandardMaterial::from(Color::srgb(0.2, 0.8, 1.0)))
        } else {
            materials.add(StandardMaterial::from(Color::srgb(0.4, 0.4, 0.5)))
        };
        commands.spawn((
            RemotePlayerMarker(row.address.clone()),
            Mesh3d(meshes.add(Cuboid::new(0.6, 1.2, 0.6))),
            MeshMaterial3d(mat),
            Transform::from_translation(pos),
            RigidBody::Fixed,
        ));
    }

    net.players.retain(|k, _| known.contains(k));

    // Despawn markers for players no longer in the table.
    for (entity, marker, _) in markers.iter_mut() {
        if !known.contains(&marker.0) {
            commands.entity(entity).despawn();
        }
    }
}