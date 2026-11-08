//! `idlebot-godot-bridge` — Rust sidecar for the IdleBot Godot client.
//!
//! Godot has no built-in SpacetimeDB client, so it talks to this bridge over a
//! local WebSocket. The bridge forwards the client's commands to the
//! SpacetimeDB `nestfeed` module (via the SpacetimeDB SDK) and relays the
//! authoritative `player` table changes + reducer results back to the
//! client(s).
//!
//! The SpacetimeDB connection is pumped on a dedicated background thread
//! (`run_threaded`), kept separate from the tokio runtime that serves the
//! WebSocket, so the two runtimes never share a thread.

mod gen;
mod protocol;

use anyhow::anyhow;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;

use spacetimedb_sdk::__codegen::{TableWithPrimaryKey, WithDelete, WithInsert};
use crate::gen::*;
use crate::protocol::{ClientMsg, PlayerSnapshot, ServerMsg};

/// Where SpacetimeDB lives (override with `SPACETIMEDB_URI`).
const DEFAULT_SERVER_URI: &str = "https://db.nestfeed.app";
/// Module (database) name published by `idlecore-server`.
const DEFAULT_MODULE_NAME: &str = "nestfeed";
/// Local WebSocket the Godot client connects to (override with `BRIDGE_WS_ADDR`).
const DEFAULT_WS_ADDR: &str = "127.0.0.1:9400";

fn server_uri() -> String {
    std::env::var("SPACETIMEDB_URI").unwrap_or_else(|_| DEFAULT_SERVER_URI.into())
}
fn module_name() -> String {
    std::env::var("SPACETIMEDB_MODULE").unwrap_or_else(|_| DEFAULT_MODULE_NAME.into())
}
fn ws_addr() -> String {
    std::env::var("BRIDGE_WS_ADDR").unwrap_or_else(|_| DEFAULT_WS_ADDR.into())
}

/// Shared handle to the live SpacetimeDB connection. `Mutex` guards it so the
/// tokio WS tasks can take exclusive access to send reducers.
type ConnSlot = Arc<Mutex<DbConnection>>;

/// Identity issued by the server on connect (set by the `on_connect` callback
/// on the pump thread; read by the WS tasks once established).
type IdentitySlot = Arc<Mutex<Option<String>>>;
/// Connection error message (set by the `on_connect_error` callback).
type ConnErrorSlot = Arc<Mutex<Option<String>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt().init();

    let identity = Arc::new(Mutex::new(None::<String>));
    let conn_err = Arc::new(Mutex::new(None::<String>));

    // Build the SpacetimeDB connection synchronously (on the main thread, NOT
    // inside a tokio runtime) so the SDK allocates its own background runtime.
    let (broadcast_tx, broadcast_rx) = tokio::sync::broadcast::channel::<ServerMsg>(1024);
    let conn = {
        let id = identity.clone();
        let err = conn_err.clone();
        let bcast = broadcast_tx.clone();
        let bcast_conn = bcast.clone();
        let builder = DbConnection::builder()
            .with_uri(server_uri())
            .with_database_name(module_name())
            .on_connect(move |_ctx, ident, _token| {
                tracing::info!("SpacetimeDB connected as {}", ident);
                *id.lock().unwrap() = Some(ident.to_string());
                let _ = bcast_conn.send(ServerMsg::Connected { identity: ident.to_string() });
            })
            .on_connect_error(move |_ctx, e| {
                let msg = e.to_string();
                tracing::error!("SpacetimeDB connect error: {msg}");
                *err.lock().unwrap() = Some(msg);
            });
        let conn = builder.build().map_err(|e| anyhow!("{e:?}"))?;

        // Register the authoritative `player` table callbacks; broadcast each
        // row change to every connected client.
        conn.db.player().on_insert({
            let bcast = bcast.clone();
            move |_ctx, row| {
                let snap = PlayerSnapshot::from_player(row);
                let _ = bcast.send(ServerMsg::Row {
                    table: "player".into(),
                    op: "insert".into(),
                    data: Some(snap),
                });
            }
        });
        conn.db.player().on_update({
            let bcast = bcast.clone();
            move |_ctx, _old, new| {
                let snap = PlayerSnapshot::from_player(new);
                let _ = bcast.send(ServerMsg::Row {
                    table: "player".into(),
                    op: "update".into(),
                    data: Some(snap),
                });
            }
        });
        conn.db.player().on_delete({
            let bcast = bcast.clone();
            move |_ctx, row| {
                let snap = PlayerSnapshot::from_player(row);
                let _ = bcast.send(ServerMsg::Row {
                    table: "player".into(),
                    op: "delete".into(),
                    data: Some(snap),
                });
            }
        });
        conn
    };

    tracing::info!("SpacetimeDB connection established (pumping on background thread)");
    let pump = conn.run_threaded();
    tracing::info!("WS bridge listening on {}", ws_addr());

    let conn_slot: ConnSlot = Arc::new(Mutex::new(conn));
    let identity_slot = identity;
    let conn_err_slot = conn_err;

    run_ws_server(&conn_slot, &broadcast_rx, &identity_slot, &conn_err_slot, &ws_addr()).await?;
    if let Err(e) = pump.join() {
        tracing::error!("pump thread exited with {e:?}");
    }

    Ok(())
}

async fn run_ws_server(
    conn_slot: &ConnSlot,
    bcast_rx: &tokio::sync::broadcast::Receiver<ServerMsg>,
    identity_slot: &IdentitySlot,
    conn_err_slot: &ConnErrorSlot,
    addr: &str,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("accepting on {addr}");

    loop {
        let (tcp, _addr) = listener.accept().await?;
        let peer = tcp.peer_addr().map(|a| a.to_string()).unwrap_or("<unknown>".into());
        let ws = accept_async(tcp).await?;
        tracing::info!("client {peer} connected");
        let conn_slot2 = conn_slot.clone();
        let bcast_rx2 = bcast_rx.resubscribe();
        let identity_slot2 = identity_slot.clone();
        let conn_err_slot2 = conn_err_slot.clone();
        let peer2 = peer.clone();
        let _ = tokio::spawn(async move {
            if let Err(e) = handle_client(ws, conn_slot2, bcast_rx2, identity_slot2, conn_err_slot2).await {
                tracing::debug!("client {peer2} closed: {e:?}");
            }
        });
    }
}

/// Build a reducer result reporter: converts the SDK reducer callback result
/// into a `Reducer` `ServerMsg` sent to this specific client.
fn reducer_cb(
    name: &'static str,
    tx: tokio::sync::mpsc::UnboundedSender<ServerMsg>,
) -> Box<dyn FnOnce(
    &ReducerEventContext,
    Result<Result<(), String>, spacetimedb_sdk::__codegen::InternalError>,
) + Send + 'static> {
    Box::new(move |_ctx, res| {
        let (ok, message) = match &res {
            Ok(Ok(())) => (true, format!("{name}: ok")),
            Ok(Err(e)) => (false, e.clone()),
            Err(e) => (false, format!("{name}: send error: {e}")),
        };
        let _ = tx.send(ServerMsg::Reducer {
            name: name.into(),
            ok,
            message,
        });
    })
}

async fn handle_client(
    mut ws: WebSocketStream<tokio::net::TcpStream>,
    conn_slot: ConnSlot,
    bcast_rx: tokio::sync::broadcast::Receiver<ServerMsg>,
    identity_slot: IdentitySlot,
    conn_err_slot: ConnErrorSlot,
) -> anyhow::Result<()> {
    // Per-client channel for messages that only this client cares about
    // (reducer results). Broadcast row events go through `bcast_rx`.
    let (client_tx, mut client_rx) = tokio::sync::mpsc::unbounded_channel::<ServerMsg>();

    // Wait for the SpacetimeDB connection to finish (built before this task,
    // so the callback may already have fired; otherwise wait up to 30s).
    let connect = {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(30);
        loop {
            if let Some(id) = identity_slot.lock().unwrap().clone() {
                break ServerMsg::Connected { identity: id };
            }
            if let Some(e) = conn_err_slot.lock().unwrap().clone() {
                break ServerMsg::Error { message: e };
            }
            if tokio::time::Instant::now() >= deadline {
                break ServerMsg::Error {
                    message: "SpacetimeDB connection not established in time".into(),
                };
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    };
    send_json(&mut ws, &connect).await?;

    let mut bcast_sub = bcast_rx.resubscribe();

    loop {
        tokio::select! {
            // Incoming command from the Godot client.
            next = ws.next() => {
                match next {
                    None => return Ok(()),
                    Some(item) => match item {
                        Ok(Message::Text(t)) => {
                            let body = t.as_str();
                            match serde_json::from_str::<ClientMsg>(body) {
                                Ok(cmd) => {
                                    if let Err(e) = dispatch(&conn_slot, client_tx.clone(), cmd).await {
                                        tracing::warn!("dispatch error: {e:?}");
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("bad client msg: {body}: {e}");
                                    let _ = send_json(&mut ws, &ServerMsg::Error { message: e.to_string() }).await;
                                }
                            }
                        }
                        Ok(Message::Ping(_)) => {
                            let _ = ws.send(Message::Pong(vec![])).await;
                        }
                        Ok(Message::Close(_)) => return Ok(()),
                        Ok(_) => { /* ignore other frame types */ }
                        Err(e) => {
                            tracing::debug!("ws recv error: {e:?}");
                            return Ok(());
                        }
                    },
                }
            }
            // A message addressed to this client (reducer result).
            res = client_rx.recv() => {
                match res {
                    Some(msg) => {
                        if send_json(&mut ws, &msg).await.is_err() {
                            return Ok(());
                        }
                    }
                    None => return Ok(()),
                }
            }
            // A row change broadcast to all clients.
            ev = bcast_sub.recv() => {
                match ev {
                    Ok(msg) => {
                        if send_json(&mut ws, &msg).await.is_err() {
                            return Ok(());
                        }
                    }
                    Err(_) => { /* broadcast closed; keep going */ }
                }
            }
        }
    }
}

/// Send a `ServerMsg` as a JSON text frame.
async fn send_json(ws: &mut WebSocketStream<tokio::net::TcpStream>, msg: &ServerMsg) -> anyhow::Result<()> {
    let s = serde_json::to_string(msg).map_err(|e| anyhow!("{e:?}"))?;
    ws.send(Message::Text(s.into()))
        .await
        .map_err(|e| anyhow!("{e:?}"))
}

/// Dispatch one `ClientMsg` to the SpacetimeDB connection. Locks the shared
/// connection and forwards the command as a SpacetimeDB reducer; the reducer
/// result is relayed back to the requesting client via `tx`.
async fn dispatch(
    conn_slot: &ConnSlot,
    tx: tokio::sync::mpsc::UnboundedSender<ServerMsg>,
    cmd: ClientMsg,
) -> anyhow::Result<()> {
    let result: spacetimedb_sdk::Result<()> = {
        let conn = conn_slot.lock().unwrap();
        let reducers = &conn.reducers;
        match cmd {
            ClientMsg::Connect => Ok(()),
            ClientMsg::Login { name } => {
                reducers.login_then(name, reducer_cb("login", tx.clone()))
            }
            ClientMsg::UpdateProfile { display_name, avatar, bio } => {
                reducers.update_profile_then(display_name, avatar, bio, reducer_cb("update_profile", tx.clone()))
            }
            ClientMsg::Move { dir_x, dir_y, intended_speed, dt, to_x, to_y } => {
                reducers.move_player_then(dir_x, dir_y, intended_speed, dt, to_x, to_y, reducer_cb("move_player", tx.clone()))
            }
            ClientMsg::Heartbeat => {
                reducers.heartbeat_then(reducer_cb("heartbeat", tx.clone()))
            }
            ClientMsg::Plant { hex_id, plant_type } => {
                reducers.plant_then(hex_id, plant_type, reducer_cb("plant", tx.clone()))
            }
            ClientMsg::Harvest { hex_id } => {
                reducers.harvest_then(hex_id, reducer_cb("harvest", tx.clone()))
            }
            ClientMsg::Clean { hex_id } => {
                reducers.clean_then(hex_id, reducer_cb("clean", tx.clone()))
            }
            ClientMsg::ClaimIdle => {
                reducers.claim_idle_gains_then(reducer_cb("claim_idle", tx.clone()))
            }
            ClientMsg::Teleport { q, r } => {
                let tx2 = tx.clone();
                let tq = q;
                let tr = r;
                reducers.teleport_player_then(q, r, move |_ctx, res| {
                    let (ok, message) = match &res {
                        Ok(Ok(())) => (true, format!("teleport ok -> hex ({tq},{tr})")),
                        Ok(Err(e)) => (false, e.clone()),
                        Err(e) => (false, format!("teleport send error: {e}")),
                    };
                    if ok {
                        let _ = tx2.send(ServerMsg::Teleport { q: tq, r: tr });
                    }
                    let _ = tx2.send(ServerMsg::Reducer { name: "teleport".into(), ok, message });
                })
            }
        }
    };
    result.map_err(|e| anyhow!("{e:?}"))
}
