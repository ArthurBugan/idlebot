//! Headless replication + reducer integration smoke (Spec 019 T4.3) — no Bevy
//! needed.
//!
//! Connects to a running local SpacetimeDB and verifies, against throwaway
//! wallets:
//!   [1-4] wallet A: login replication, teleport round trip (018 T6.2).
//!   [5-8] wallet B: plant (10 G), profile avatar, move_player position.
//!   [9]   reconnect: a NEW connection with wallet B's SAVED TOKEN sees the
//!         persisted avatar, position and planted hex (server persistence).
//!
//! Each wallet logs in from its own connection/identity, matching the real
//! client flow (the server resolves `address_of` from the caller identity).
//!
//! Run:  cargo run -p idlecore-client --bin e2e -- [wallet-tag]
//! Exit: 0 = PASS, 1 = FAIL, 2 = server not reachable (skip).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "../net/gen/mod.rs"]
mod gen;

use spacetimedb_sdk::DbContext;
use spacetimedb_sdk::__codegen::TableLike;
use gen::player_table::PlayerTableAccess;
use gen::hex_tile_table::HexTileTableAccess;
use gen::login;
use gen::move_player;
use gen::plant;
use gen::teleport_player;
use gen::update_profile;

const URI: &str = "http://127.0.0.1:3000";
const DB: &str = "idlebot";
const TIMEOUT: Duration = Duration::from_secs(25);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tag = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("e2e-{}", std::process::id()));
    let wallet_a = format!("0x{tag}");
    let wallet_b = format!("0x{tag}-b");

    let mut failures: Vec<String> = Vec::new();

    // --- Wallet A connection (own identity) --------------------------------
    let conn = connect();
    conn.subscription_builder().subscribe_to_all_tables();

    // --- Phase 1: connect handshake ---------------------------------------
    println!("[1] connecting to {URI} db={DB}");
    if !await_active(&conn) {
        println!("FAIL: connect handshake did not complete");
        std::process::exit(1);
    }
    println!("[1] PASS: handshake complete");

    // --- Phase 2: login creates the player row server-side -----------------
    login_phase(&conn, &wallet_a, "2", &mut failures);

    // --- Phase 3: replication — row created by the server reaches us -------
    println!("[3] waiting for replicated player row (gold, level)");
    let replicated = wait_until(&conn, TIMEOUT, || {
        conn.db
            .player()
            .iter()
            .any(|p| p.address == wallet_a && p.gold == 100 && p.level == 1)
    });
    if !replicated {
        failures.push("replication: player row never appeared".to_string());
    } else {
        println!("[3] PASS: player row replicated (gold=100, level=1)");
    }

    // --- Phase 4: teleport round trip replicates position ------------------
    println!("[4] teleport(1,0) round trip");
    let sent = Instant::now();
    let mut rtt: Option<Duration> = None;
    if let Err(e) = conn.reducers().teleport_player_then(1, 0, move |_ctx, _res| {}) {
        failures.push(format!("teleport send failed: {e}"));
    } else {
        let moved = wait_until_fine(&conn, TIMEOUT, || {
            conn.db
                .player()
                .iter()
                .find(|p| p.address == wallet_a)
                .map(|p| p.hex_q == 1 && p.hex_r == 0)
                .unwrap_or(false)
        });
        rtt = Some(Instant::now() - sent);
        if !moved {
            failures.push("teleport position never replicated".to_string());
        } else {
            println!("[4] PASS: teleport replicated");
            let ms = rtt.unwrap().as_secs_f64() * 1000.0;
            if ms <= 100.0 {
                println!("[4] acceptance: teleport RTT {ms:.0} ms <= 100 ms (018 T6.2)");
            } else {
                failures.push(format!("018 T6.2: teleport RTT {ms:.0} ms > 100 ms budget"));
            }
        }
    }

    // --- Wallet B connection (own identity, token captured) ----------------
    println!("[5] opening wallet B connection");
    let token_b = Arc::new(Mutex::new(None::<String>));
    let conn_b = {
        let token_b = token_b.clone();
        match gen::DbConnection::builder()
            .with_uri(URI)
            .with_database_name(DB)
            .on_connect({
                move |_ctx, _identity, t| {
                    *token_b.lock().unwrap() = Some(t.to_string());
                }
            })
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                println!("FAIL: cannot open wallet B connection ({e})");
                std::process::exit(1);
            }
        }
    };
    conn_b.subscription_builder().subscribe_to_all_tables();
    if !await_active(&conn_b) {
        failures.push("wallet B handshake did not complete".to_string());
    }
    let (planted_hex, avatar, (x, y)) = economy_phases(&conn_b, &wallet_b, &mut failures);

    // --- Phase 9: reconnect with wallet B's SAVED TOKEN --------------------
    println!("[9] reconnecting with wallet B's saved token");
    let token_b = token_b.lock().unwrap().clone();
    if let Some(token) = token_b {
        let conn2 = match gen::DbConnection::builder()
            .with_uri(URI)
            .with_database_name(DB)
            .with_token(Some(token))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("reconnect: cannot connect ({e})"));
                std::process::exit(1);
            }
        };
        conn2.subscription_builder().subscribe_to_all_tables();
        let ok = connect_and_login(&conn2, &wallet_b);
        if !ok {
            failures.push("reconnect: login on second connection failed".to_string());
        } else {
            let avatar_ok = wait_until(&conn2, TIMEOUT, || {
                conn2
                    .db
                    .player()
                    .iter()
                    .any(|p| p.address == wallet_b && p.avatar == avatar)
            });
            if avatar_ok {
                println!("[9] PASS: avatar persisted across reconnect");
            } else {
                failures.push(format!("reconnect: avatar '{avatar}' not persisted"));
            }

            let pos_ok = wait_until(&conn2, TIMEOUT, || {
                conn2
                    .db
                    .player()
                    .iter()
                    .any(|p| {
                        p.address == wallet_b
                            && (p.position_x - x).abs() < 0.01
                            && (p.position_y - y).abs() < 0.01
                    })
            });
            if pos_ok {
                println!("[9] PASS: position ({x:.1},{y:.1}) persisted across reconnect");
            } else {
                failures.push(format!("reconnect: position ({x:.1},{y:.1}) not persisted"));
            }

            if let Some(hex_id) = planted_hex {
                let plant_ok = wait_until(&conn2, TIMEOUT, || {
                    conn2
                        .db
                        .hex_tile()
                        .iter()
                        .any(|h| h.hex_id == hex_id && h.plant.is_some())
                });
                if plant_ok {
                    println!("[9] PASS: planted hex {hex_id} persisted across reconnect");
                } else {
                    failures.push("reconnect: planted hex not persisted".to_string());
                }
            }
        }
    } else {
        failures.push("reconnect: no token captured from wallet B connection".to_string());
    }

    // --- Summary ------------------------------------------------------------
    if failures.is_empty() {
        println!("PASS: integration smoke complete (teleport RTT ~{} ms)", rtt.map(|d| d.as_millis()).unwrap_or(0));
    } else {
        for f in &failures {
            println!("FAIL: {f}");
        }
        std::process::exit(1);
    }
}

/// Build a connection; exit(2) when the server is unreachable.
fn connect() -> gen::DbConnection {
    match gen::DbConnection::builder()
        .with_uri(URI)
        .with_database_name(DB)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: cannot connect to {URI} ({e})");
            std::process::exit(2);
        }
    }
}

/// Drive frames until the connection is active or the timeout elapses.
fn await_active(conn: &gen::DbConnection) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        let _ = conn.frame_tick();
        std::thread::sleep(Duration::from_millis(50));
        if conn.is_active() {
            return true;
        }
    }
    false
}

/// Phases 6-8: wallet B moves, plants and sets an avatar. Returns
/// (planted_hex_id, avatar_name, final_position).
fn economy_phases(
    conn: &gen::DbConnection,
    wallet: &str,
    failures: &mut Vec<String>,
) -> (Option<u64>, String, (f32, f32)) {
    // --- Phase 5: login wallet B ------------------------------------------
    login_phase(conn, wallet, "5", failures);
    let replicated = wait_until(conn, TIMEOUT, || {
        conn.db
            .player()
            .iter()
            .any(|p| p.address == wallet && p.gold == 100 && p.level == 1)
    });
    if !replicated {
        failures.push("replication: wallet B player row never appeared".to_string());
        return (None, String::new(), (0.0, 0.0));
    }
    println!("[5] PASS: wallet B row replicated (gold=100)");

    // --- Phase 6: move_player moves the row --------------------------------
    println!("[6] move_player(1,0) at speed 5");
    let prev = conn
        .db
        .player()
        .iter()
        .find(|p| p.address == wallet)
        .map(|p| (p.position_x, p.position_y))
        .unwrap_or((0.0, 0.0));
    if let Err(e) = conn.reducers().move_player_then(1.0, 0.0, 5.0, 1.0, move |_ctx, _res| {}) {
        failures.push(format!("move_player send failed: {e}"));
    }
    let moved = wait_until(conn, TIMEOUT, || {
        conn.db
            .player()
            .iter()
            .find(|p| p.address == wallet)
            .map(|p| {
                let dx = p.position_x - prev.0;
                let dy = p.position_y - prev.1;
                dx * dx + dy * dy > 0.25 // more than half a unit
            })
            .unwrap_or(false)
    });
    let final_pos = conn
        .db
        .player()
        .iter()
        .find(|p| p.address == wallet)
        .map(|p| (p.position_x, p.position_y))
        .unwrap_or(prev);
    if moved {
        println!(
            "[6] PASS: position ({:.1},{:.1}) -> ({:.1},{:.1})",
            prev.0, prev.1, final_pos.0, final_pos.1
        );
    } else {
        failures.push("move_player position never replicated".to_string());
    }

    // --- Phase 7: plant Wheat on the player's current hex -------------------
    // The hex must be free (runs leave plants behind) AND far from spawn
    // (>= 40 hexes out) so test plants never litter the player's area.
    // Server hexes are ~17.3 world units wide but moves are capped at 10 u/s,
    // so two moves per attempt guarantee crossing a boundary.
    let start = std::time::Instant::now();
    println!("[7] plant Wheat on current hex (10 G)");
    let mut planted: Option<u64> = None;
    // Walk east first; at the world edge (x ~1216 = hex ~70) switch north,
    // which past runs never littered.
    let mut north = false;
    for attempt in 1..=120 {
        let hex_id = conn
            .db
            .player()
            .iter()
            .find(|p| p.address == wallet)
            .map(|p| p.hex_id)
            .unwrap_or(0);
        let pos = conn
            .db
            .player()
            .iter()
            .find(|p| p.address == wallet)
            .map(|p| (p.position_x, p.position_y))
            .unwrap_or((0.0, 0.0));
        let far_enough = conn
            .db
            .player()
            .iter()
            .find(|p| p.address == wallet)
            .map(|p| p.hex_q >= 40 || p.hex_r >= 40)
            .unwrap_or(false);
        let free = conn
            .db
            .hex_tile()
            .iter()
            .find(|h| h.hex_id == hex_id)
            .map(|h| {
                !h.is_polluted
                    && (h.terrain == "Grass" || h.terrain == "Forest")
                    && h.plant.is_none()
            })
            .unwrap_or(false);
        if !free || !far_enough {
            if pos.0 >= 1200.0 {
                north = true;
            }
            let (dx, dy) = if north { (0.0, 1.0) } else { (1.0, 0.0) };
            println!(
                "[7 t={:.0}s] hex {hex_id} not plantable (occupied/polluted/terrain/too close); walking further",
                start.elapsed().as_secs_f32()
            );
            if conn
                .reducers()
                .move_player_then(dx, dy, 10.0, 1.0, move |_ctx, _res| {})
                .is_err()
                || conn
                    .reducers()
                    .move_player_then(dx, dy, 10.0, 1.0, move |_ctx, _res| {})
                    .is_err()
            {
                failures.push("plant retry move send failed".to_string());
                break;
            }
            let _ = wait_until(conn, TIMEOUT, || {
                conn.db
                    .player()
                    .iter()
                    .any(|p| p.address == wallet && p.hex_id != hex_id)
            });
            continue;
        }
        println!("[7] attempt {attempt}: planting at hex {hex_id}");
        let outcome_cell = Arc::new(Mutex::new(None::<String>));
        let oc = outcome_cell.clone();
        if let Err(e) = conn.reducers().plant_then(hex_id, "Wheat".to_string(), move |_ctx, res| {
            *oc.lock().unwrap() = Some(match res {
                Ok(Ok(())) => "ok".to_string(),
                Ok(Err(e)) => format!("rejected: {e}"),
                Err(e) => format!("sdk error: {e}"),
            });
        }) {
            failures.push(format!("plant send failed: {e}"));
            break;
        }
        let acked = wait_until(conn, TIMEOUT, || outcome_cell.lock().unwrap().is_some());
        if !acked {
            failures.push("plant reducer never acked".to_string());
            break;
        }
        let outcome = outcome_cell.lock().unwrap().clone().unwrap();
        if outcome != "ok" {
            failures.push(format!("plant reducer: {outcome}"));
            break;
        }
        let hex_ok = wait_until(conn, TIMEOUT, || {
            conn.db
                .hex_tile()
                .iter()
                .any(|h| h.hex_id == hex_id && h.plant.is_some())
        });
        let gold_ok = wait_until(conn, TIMEOUT, || {
            conn.db
                .player()
                .iter()
                .any(|p| p.address == wallet && p.gold == 90)
        });
        if hex_ok && gold_ok {
            planted = Some(hex_id);
            println!("[7] PASS: hex {hex_id} planted, gold 100 -> 90");
        } else {
            failures.push(format!(
                "plant replicated hex={hex_ok} gold={gold_ok} (want both)"
            ));
        }
        break;
    }
    if planted.is_none() && failures.iter().all(|f| !f.starts_with("plant")) {
        failures.push("no free far hex found for planting after 40 moves".to_string());
    }

    // --- Phase 8: profile avatar -------------------------------------------
    let avatar = "zombieA".to_string();
    println!("[8] update_profile avatar={avatar}");
    if let Err(e) = conn.reducers().update_profile_then(
        None,
        Some(avatar.clone()),
        None,
        move |_ctx, _res| {},
    ) {
        failures.push(format!("update_profile send failed: {e}"));
    }
    let av_ok = wait_until(conn, TIMEOUT, || {
        conn.db
            .player()
            .iter()
            .any(|p| p.address == wallet && p.avatar == avatar)
    });
    if av_ok {
        println!("[8] PASS: avatar replicated as '{avatar}'");
    } else {
        failures.push("avatar change never replicated".to_string());
    }

    let final_pos = conn
        .db
        .player()
        .iter()
        .find(|p| p.address == wallet)
        .map(|p| (p.position_x, p.position_y))
        .unwrap_or(prev);
    (planted, avatar, final_pos)
}

/// Phase 2/5: login reducer acks with Ok.
fn login_phase(conn: &gen::DbConnection, wallet: &str, label: &str, failures: &mut Vec<String>) {
    println!("[{label}] login({wallet}) via reducer");
    let reducer_acked = std::sync::Arc::new(AtomicBool::new(false));
    let reducer_ok = std::sync::Arc::new(AtomicBool::new(false));
    let acked = reducer_acked.clone();
    let ok = reducer_ok.clone();
    if let Err(e) = conn.reducers().login_then(wallet.to_string(), move |_ctx, res| {
        acked.store(true, Ordering::Relaxed);
        ok.store(matches!(res, Ok(Ok(_))), Ordering::Relaxed);
    }) {
        println!("FAIL: login_then send error: {e}");
        std::process::exit(1);
    }
    wait_until(conn, TIMEOUT, || reducer_acked.load(Ordering::Relaxed));
    if !reducer_ok.load(Ordering::Relaxed) {
        failures.push(format!("[{label}] login reducer reported an error"));
        return;
    }
    println!("[{label}] PASS: login reducer acked");
}

/// Drives the connection until `cond` is true or the timeout elapses.
fn wait_until(conn: &gen::DbConnection, timeout: Duration, cond: impl Fn() -> bool) -> bool {
    wait_until_fine(conn, timeout, cond)
}

/// Same as [`wait_until`] but with 10 ms polling for finer latency samples.
fn wait_until_fine(conn: &gen::DbConnection, timeout: Duration, cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let _ = conn.frame_tick();
        std::thread::sleep(Duration::from_millis(10));
        if cond() {
            return true;
        }
    }
    false
}

/// Connect, await the handshake, then login. Returns success.
fn connect_and_login(conn: &gen::DbConnection, wallet: &str) -> bool {
    if !await_active(conn) {
        return false;
    }
    let acked = Arc::new(AtomicBool::new(false));
    let ok = Arc::new(AtomicBool::new(false));
    let a = acked.clone();
    let o = ok.clone();
    if conn
        .reducers()
        .login_then(wallet.to_string(), move |_ctx, res| {
            a.store(true, Ordering::Relaxed);
            o.store(matches!(res, Ok(Ok(_))), Ordering::Relaxed);
        })
        .is_err()
    {
        return false;
    }
    wait_until(conn, TIMEOUT, || acked.load(Ordering::Relaxed)) && ok.load(Ordering::Relaxed)
}