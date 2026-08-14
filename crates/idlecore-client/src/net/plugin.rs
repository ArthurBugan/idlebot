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
    /// A teleport confirmed by the server; the sim must move the player.
    Teleported { q: i32, r: i32 },
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
    pub conn: Option<DbConnection>,
    /// Our bound wallet address (set after a successful `login`).
    pub address: Option<String>,
    /// Our identity as issued by the server.
    pub identity: String,
    /// Snapshot of every player row we know about (including ourselves).
    pub players: HashMap<String, ServerPlayerSnapshot>,
    /// Recent server/connection messages for the HUD log.
    pub log: VecDeque<String>,
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
            conn: None,
            address: None,
            identity: String::new(),
            players: HashMap::new(),
            log: VecDeque::new(),
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

        // Table callbacks → events + a dirty counter so per-frame sync work
        // only runs when the replicated set actually changed.
        let dirty = self.players_dirty.clone();
        conn.db.player().on_insert({
            let tx = self.tx.clone();
            let dirty = dirty.clone();
            move |_ctx, row| {
                let _ = tx.send(NetEvent::ServerMessage(format!("player joined {}", row.address)));
                dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        });
        conn.db.player().on_delete({
            let tx = self.tx.clone();
            let dirty = dirty.clone();
            move |_ctx, row| {
                let _ = tx.send(NetEvent::ServerMessage(format!("player left {}", row.address)));
                dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

    /// Mark the replicated `player` set as changed (called after any reducer
    /// we send that mutates player rows; SDK row-insert/delete also bump it).
    pub fn mark_players_dirty(&mut self) {
        self.players_dirty.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// True if the replicated `player` table changed since the last poll.
    pub fn poll_players_dirty(&mut self) -> bool {
        let version = self.players_dirty.load(std::sync::atomic::Ordering::Relaxed);
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
    mut net: ResMut<Net>,
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
                        super::hud::send_reducer(&mut net, |r| {
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
            super::hud::send_reducer(&mut net, |r| {
                r.clean_then(hex_id, super::hud::reducer_report("clean", tx.clone(), hex_id))
            });
        } else if hex.terrain == "Grass" || hex.terrain == "Forest" {
            super::hud::send_reducer(&mut net, |r| {
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
fn net_drain(
    mut net: ResMut<Net>,
    mut burst: ResMut<crate::assets::BurstFx>,
    mut latency: ResMut<ServerLatency>,
    mut bodies: Query<
        (&mut Transform, &mut Velocity, &mut crate::player::ClientPlayer),
        With<crate::plugins::player::PhysicsBody>,
    >,
) {
    let teleports = net.drain();
    if teleports.is_empty() {
        return;
    }
    let Ok((mut transform, mut velocity, mut player)) = bodies.single_mut() else {
        return;
    };
    for (q, r) in teleports {
        let (wx, wz) = idlecore_core::hex_grid::HexGrid::axial_to_world(
            q,
            r,
            WorldGenConfig::HEX_SIZE,
        );
        let y = transform.translation.y.max(0.5);
        transform.translation = Vec3::new(wx, y, wz);
        velocity.linear = Vec3::ZERO;
        player.position = Vec3::new(wx, y, wz);
        player.current_hex = Some(crate::player::CurrentHex { q, r });
        burst.request(Vec3::new(wx, y, wz));
        latency.resolve();
    }
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
    let Some(mine) = net.address.clone() else { return };

    // Perf: rebuild is O(rows * markers); skip it entirely unless the
    // replicated player set changed (row callbacks or reducers we sent).
    // The first frame after connect always runs (players map is empty).
    if !net.poll_players_dirty() && !net.players.is_empty() {
        return;
    }
    let Some(conn) = net.conn.as_ref() else { return };

    // Spec 018 T2.4: only players within 3 hexes are visible.
    let own_hex: Option<(i32, i32)> = player
        .single()
        .ok()
        .and_then(|p| p.current_hex.map(|h| (h.q, h.r)))
        .or_else(|| {
            player
                .single()
                .ok()
                .map(|p| idlecore_core::hex::world_pos_to_hex(p.position.x, p.position.z, WorldGenConfig::HEX_SIZE))
        });
    let in_view = |row_hex_id: u64| -> bool {
        let Some((q, r)) = own_hex else { return true };
        let rc = idlecore_core::hex::HexCoord::from_id(row_hex_id);
        axial_hex_distance((q, r), (rc.q, rc.r)) <= 3
    };

    let rows: Vec<Player> = conn.db.player().iter().collect();
    let tx = net.sender();
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for row in &rows {
        if row.address == *mine {
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
                if row.level > old_level {
                    let _ = tx.send(NetEvent::ServerMessage(format!(
                        "LEVEL UP! Now level {}",
                        row.level
                    )));
                }
            }
            continue;
        }
        if !in_view(row.hex_id) {
            // Outside the view radius: don't light the marker, but keep the
            // row known so the marker despawn logic leaves it alone.
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
        commands
            .spawn((
                RemotePlayerMarker(row.address.clone()),
                Mesh3d(meshes.add(Cuboid::new(0.6, 1.2, 0.6))),
                MeshMaterial3d(mat),
                Transform::from_translation(pos),
                RigidBody::Fixed,
            ))
            .with_child((
                Name::new("player-name-label"),
                Text2d::new(short_addr(&row.address)),
                TextFont { font_size: 30.0.into(), ..default() },
                TextColor(Color::srgb(0.95, 0.95, 1.0)),
                TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
                Transform::from_xyz(0.0, 1.8, 0.0),
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

/// Map a server-side vehicle string back to the core Vehicle enum.
fn vehicle_from_str(s: &str) -> Option<idlecore_core::Vehicle> {
    match s {
        "Bicycle" => Some(idlecore_core::Vehicle::Bicycle),
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

fn short_addr(s: &str) -> String {
    if s.len() > 12 {
        format!("{}...{}", &s[..5], &s[s.len() - 4..])
    } else {
        s.to_string()
    }
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
        Self { samples: VecDeque::new(), max: 60 }
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
    pub fn latest_ms(&self) -> Option<f32> {
        self.samples.back().copied()
    }
}

/// RTT of a teleport round trip: request sent → server-confirmed arrival.
#[derive(Resource, Default)]
pub struct ServerLatency {
    pub window: LatencyWindow,
    request_sent_at: Option<std::time::Instant>,
}

impl ServerLatency {
    /// Call when the teleport reducer is invoked (request timestamp).
    pub fn note_request(&mut self) {
        self.request_sent_at = Some(std::time::Instant::now());
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
        let mut w = LatencyWindow { max: 3, ..Default::default() };
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
