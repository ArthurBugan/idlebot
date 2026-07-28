# Spec 018: Multiplayer Architecture

> **Objective:** Implement connection, state synchronization, and conflict resolution for up to 100 concurrent players

## Problem Statement

IdleBot is a multiplayer idle game where 100 players share a single hex grid world. Player positions, hex states, voice channels, and economy actions must be synchronized in real-time via SpacetimeDB, with reliable reconnect and graceful degradation on network issues.

## Proposed Solution

- Wallet-authenticated session tokens for connection
- SpacetimeDB replication tables for player state (position, hex, status)
- 100ms client-side movement prediction + server correction
- Server-authoritative hex occupancy rules
- Automatic voice channel management on disconnect
- View radius filtering (clients only receive nearby player updates)

## Requirements

### Functional Requirements
1. FR1: Connection flow — wallet signature → JWT → SpacetimeDB client connection
2. FR2: Player state sync (position, hex, vehicle, cosmetics) at 100ms intervals
3. FR3: Hex occupancy tracking — server tracks which players occupy which hex
4. FR4: Conflict resolution — two players on same hex: use proximity (within hex radius)
5. FR5: Disconnect handling — server marks player offline, waits 5s, then cleans up
6. FR6: Client visibility — only receive updates for players within 3-hex radius
6. FR7: All state changes validated server-side before applying

### Non-Functional Requirements
1. NFR1: Support 100 concurrent connections on single SpacetimeDB node
2. NFR2: State sync latency < 200ms under normal load
3. NFR3: Graceful degradation — predict movement client-side, correct on server reply
4. NFR4: Reconnect with backoff (1s → 2s → 4s, max 30s) without losing position state

## Design

### Connection Flow
```rust
// 1. Wallet auth (spec 013) — returns JWT
let token = wallet_auth.sign_and_get_token();

// 2. Connect to SpacetimeDB
let client = spacetimedb::connect(
    endpoint,       // self-hosted Pi/VPS
    token,          // JWT from wallet auth
    max_connections: 100,
)?;

// 3. Subscribe to relevant tables
client.subscribe::<PlayerState>("players");
client.subscribe::<HexTile>("hex_tiles");
client.subscribe::<VoiceChannel>("voice_channels");
client.subscribe::<MarketListing>("market_listings");
```

### Player State Replication Table
```rust
#[spacetimedb(table)]
struct PlayerState {
    pub player_id: UUID,
    pub address: String,       // wallet address
    pub hex_id: u64,           // current hex (computed from q,r)
    pub position_x: f32,
    pub position_y: f32,
    pub velocity: Vec2,        // client-predicted velocity
    pub vehicle_id: Option<u32>,
    pub cosmetics_hash: u64,
    pub status: PlayerStatus,  // online / disconnected / reconnecting
    pub view_timestamp: u64,   // last sent timestamp for client
    pub connected_at: Instant,
}

enum PlayerStatus {
    Online,
    Disconnecting,
    Reconnecting { wait_seconds: u32 },
}
```

### Server Correction Protocol
```rust
// Client-side movement prediction
struct PredictedMovement {
    local_position: Vec2,
    local_hex: HexCoord,
    server_confirmations: Vec<ServerConfirmation>,
}

struct ServerConfirmation {
    sequence: u32,
    position: Vec2,
    hex: HexCoord,
    timestamp: Instant,
}

// Every 100ms:
// 1. Client predicts next position from input
// 2. Client sends input + predicted position to server
// 3. Server validates (within grid, no conflict, speed limit)
// 4. Server replies with authoritative state
// 5. Client applies correction if server disagrees

fn handle_server_correction(
    &mut self,
    confirmation: ServerConfirmation,
) {
    if confirmation.position != self.predicted_position {
        // Server correction — snap to authoritative position
        self.predicted_position = confirmation.position;
        self.predicted_hex = confirmation.hex;
    }
    self.server_confirmations.push(confirmation);
    // Remove old confirmations (>100ms ago)
    self.server_confirmations.retain(|c| {
        c.timestamp.elapsed() < Duration::from_millis(200)
    });
}
```

### Hex Occupancy & Conflict Resolution
```rust
fn check_conflict(
    server: &mut Server,
    player_a: &PlayerState,
    player_b: &PlayerState,
) -> Option<ConflictResolution> {
    let dist = distance(player_a.position, player_b.position);
    let hex_radius = 10.0; // meters

    if dist <= hex_radius {
        // Same hex — use proximity rule:
        // Closer to hex center wins; if equal, earlier connected wins
        let dist_a = distance(player_a.position, HexCoord::center());
        let dist_b = distance(player_b.position, HexCoord::center());

        if dist_a < dist_b {
            Some(ConflictResolution::PlayerA_KeepsPosition)
        } else if dist_b < dist_a {
            Some(ConflictResolution::PlayerB_KeepsPosition)
        } else {
            Some(ConflictResolution::ConnectedFirst_Wins)
        }
    } else {
        None // No conflict
    }
}
```

### View Radius Filtering
```rust
fn get_visible_players(
    &self,
    player_id: UUID,
    my_hex: HexCoord,
    all_players: &[PlayerState],
) -> Vec<PlayerState> {
    // Only show players within 3 hex radius
    all_players
        .iter()
        .filter(|p| {
            p.player_id != player_id
            && p.status == PlayerStatus::Online
            && manhattan_distance(my_hex, p.hex) <= 3
        })
        .collect()
}
```

### Disconnect & Reconnect
```rust
fn handle_player_disconnect(server: &mut Server, player_id: UUID) {
    // 1. Mark as Disconnecting
    server.set_player_status(player_id, PlayerStatus::Disconnecting);

    // 2. Schedule cleanup in 5 seconds
    server.schedule_cleanup(player_id, Duration::from_secs(5));

    // 3. Close voice channel if in one
    if let Some(channel_id) = server.get_player_voice_channel(player_id) {
        server.remove_player_from_voice(channel_id, player_id);
        if server.get_channel_players(channel_id).is_empty() {
            server.destroy_voice_channel(channel_id);
        }
    }
}

fn handle_player_reconnect(server: &mut Server, player_id: UUID, token: &str) {
    // 1. Validate JWT
    let address = verify_jwt(token)?;

    // 2. Check if player still exists
    let player = server.get_player_by_address(address)?;
    if player.is_none() {
        return Err(ReconnectError::PlayerGone);
    }

    // 3. Restore position from last known state
    server.restore_player_state(player_id, player.last_position);

    // 4. Mark Online
    server.set_player_status(player_id, PlayerStatus::Online);

    Ok(())
}
```

### SpacetimeDB Subscriptions (Client)
```rust
// Client subscribes only to relevant slices
// SpacetimeDB filters on server-side
pub mod player_slice {
    pub struct PlayerSlice {
        pub player_id: UUID,
        pub hex_q: i32,
        pub hex_r: i32,
        pub position_x: f32,
        pub position_y: f32,
        pub vehicle_id: Option<u32>,
    }
}

// Server module: returns only players within view radius
#[spacetimedb(table)]
pub mod player_views {
    pub struct PlayerView {
        pub target_player_id: UUID,
        pub visible_player: PlayerSlice,
    }

    pub fn get_visible_players(
        db: &spacetimedb::DatabaseIndex,
        target: UUID,
        target_hex_q: i32,
        target_hex_r: i32,
    ) -> spacetimedb::response::SendSubscriberResults {
        let target_hex = db.hex_slice(target_hex_q, target_hex_r).unwrap();
        let nearby = db.player_states()
            .filter(|p| {
                p.status == PlayerStatus::Online
                && manhattan_distance_hex(p.hex_q, p.hex_r, target_hex_q, target_hex_r) <= 3
            })
            .collect::<Vec<_>>();
        // Send each nearby player as a PlayerView for the target
        nearby.into_iter().map(|p| PlayerView {
            target_player_id: target,
            visible_player: PlayerSlice {
                player_id: p.player_id,
                hex_q: p.hex_q,
                hex_r: p.hex_r,
                position_x: p.position_x,
                position_y: p.position_y,
                vehicle_id: p.vehicle_id,
            },
        }).send_results()
    }
}
```

## Acceptance Criteria
- [ ] Player connects via wallet auth → JWT → SpacetimeDB
- [ ] Position updates arrive at server within 100ms of client input
- [ ] Server correction happens on next tick if client position diverges
- [ ] Two players on same hex resolved via proximity rule
- [ ] Voice channels auto-destroy on disconnect
- [ ] Reconnect restores last known position
- [ ] Client only receives nearby player updates (≤3 hex radius)
- [ ] 100 players can connect simultaneously without performance degradation

## Risks
- R1: SpacetimeDB single-node bottleneck at 100+ players
- R2: Client prediction desync on high-latency connections
- R3: Stale state on rapid reconnect/disconnect cycles

## Open Questions
- Q1: Should there be a ping/priority system for hex occupancy?
- Q2: What's the maximum packet size for state sync?
- Q3: Should disconnected players' data persist for a configurable time?
