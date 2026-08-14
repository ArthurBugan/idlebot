//! Headless replication smoke (Spec 019 T4.3) — no Bevy needed.
//!
//! Connects to a running local SpacetimeDB, logs in a throwaway wallet
//! address, and verifies that the authoritative `player` row created by the
//! server reducer appears in our local subscription (replication), then
//! measures one teleport round trip (018 T6.2 acceptance).
//!
//! Run:  cargo run -p idlecore-client --bin e2e -- [wallet-tag]
//! Exit: 0 = PASS, 1 = FAIL, 2 = server not reachable (skip).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[path = "../net/gen/mod.rs"]
mod gen;

use spacetimedb_sdk::DbContext;
use spacetimedb_sdk::__codegen::TableLike;
use gen::player_table::PlayerTableAccess;
use gen::login;
use gen::teleport_player;

const URI: &str = "http://127.0.0.1:3000";
const DB: &str = "idlebot";
const TIMEOUT: Duration = Duration::from_secs(25);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tag = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("e2e-{}", std::process::id()));
    let wallet = format!("0x{tag}");

    let mut conn = match gen::DbConnection::builder()
        .with_uri(URI)
        .with_database_name(DB)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: cannot connect to {URI} ({e})");
            std::process::exit(2);
        }
    };
    conn.subscription_builder().subscribe_to_all_tables();

    let mut failures: Vec<String> = Vec::new();

    // --- Phase 1: connect handshake ---------------------------------------
    let connected = AtomicBool::new(false);
    println!("[1] connecting to {URI} db={DB}");
    let deadline = Instant::now() + TIMEOUT;
    while !connected.load(Ordering::Relaxed) {
        if Instant::now() > deadline {
            println!("FAIL: connect handshake did not complete");
            std::process::exit(1);
        }
        let _ = conn.frame_tick();
        std::thread::sleep(Duration::from_millis(50));
        if conn.is_active() {
            connected.store(true, Ordering::Relaxed);
        }
    }
    println!("[1] PASS: handshake complete");

    // --- Phase 2: login creates the player row server-side -----------------
    println!("[2] login({wallet}) via reducer");
    let reducer_acked = std::sync::Arc::new(AtomicBool::new(false));
    let reducer_ok = std::sync::Arc::new(AtomicBool::new(false));
    let acked = reducer_acked.clone();
    let ok = reducer_ok.clone();
    if let Err(e) = conn.reducers().login_then(wallet.clone(), move |_ctx, res| {
        acked.store(true, Ordering::Relaxed);
        ok.store(matches!(res, Ok(Ok(_))), Ordering::Relaxed);
    }) {
        println!("FAIL: login_then send error: {e}");
        std::process::exit(1);
    }
    wait_until(&conn, TIMEOUT, || reducer_acked.load(Ordering::Relaxed));
    if !reducer_ok.load(Ordering::Relaxed) {
        println!("FAIL: login reducer reported an error");
        std::process::exit(1);
    }
    println!("[2] PASS: login reducer acked");

    // --- Phase 3: replication — row created by the server reaches us -------
    println!("[3] waiting for replicated player row (gold, level)");
    let replicated = wait_until(&conn, TIMEOUT, || {
        conn.db
            .player()
            .iter()
            .any(|p| p.address == wallet && p.gold == 100 && p.level == 1)
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
                .find(|p| p.address == wallet)
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

    // --- Summary ------------------------------------------------------------
    if failures.is_empty() {
        println!("PASS: replication smoke complete (teleport RTT ~{} ms)", rtt.map(|d| d.as_millis()).unwrap_or(0));
    } else {
        for f in &failures {
            println!("FAIL: {f}");
        }
        std::process::exit(1);
    }
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