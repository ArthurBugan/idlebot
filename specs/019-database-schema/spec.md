# Spec 019: Database Schema Design

> **Objective:** Define all SpacetimeDB tables, indexes, and relationships for IdleBot

## Problem Statement

IdleBot requires a unified database schema across all game systems: players, hex tiles, vehicles, cosmetics, marketplace, voice channels, idle gains, and scheduler state. Tables must support real-time replication via SpacetimeDB and efficient queries for the core loop.

## Proposed Solution

- Single SpacetimeDB instance (self-hosted on Pi/VPS)
- 9 tables covering all game systems
- Indexes on frequently queried fields (address, hex_id, player_id)
- Replication filters for client-side data (view radius, active channels)

## Requirements

### Functional Requirements
1. FR1: Define all 9 core tables with field types and constraints
2. FR2: Index on `address` for wallet auth lookups
3. FR3: Index on `hex_id` for hex occupancy queries
4. FR4: Index on `player_id` for player state updates
5. FR5: Replication filters limit client data to visible area
6. FR6: Scheduled functions can read/write tables atomically
7. FR7: Foreign key relationships enforced where applicable

### Non-Functional Requirements
1. NFR1: All tables support SpacetimeDB replication
2. NFR2: Query latency < 10ms for core operations
3. NFR3: Support 100+ concurrent connections

## Design

### Core Tables

#### 1. players
```rust
#[spacetimedb(table)]
pub struct Player {
    pub player_id: UUID,
    pub address: String,       // wallet address (unique)
    pub display_name: Option<String>,
    pub avatar: String,        // 'tetrahedron', 'cube', etc.
    pub bio: Option<String>,
    pub level: u32,
    pub total_xp: u64,
    pub gold: u64,
    pub eco_points: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub hex_q: i32,
    pub hex_r: i32,
    pub vehicle_id: Option<u32>,
    pub cosmetics_hash: u64,
    pub status: PlayerStatus,
    pub last_login: Instant,
    pub created_at: Instant,
    // Derived for fast queries
    pub hex_id: u64,           // = (q << 32) | r
}

pub mod players_index {
    use super::*;
    pub fn get_player_by_address(db: &spacetimedb::DatabaseIndex, address: &str) -> spacetimedb::response::GetColumn<UUID, String> {
        db.players()
            .filter(|p| p.address == address)
            .column()
            .send_result()
    }
    pub fn get_player_by_hex(db: &spacetimedb::DatabaseIndex, hex_q: i32, hex_r: i32) -> spacetimedb::response::GetColumn<UUID, i32, i32> {
        db.players()
            .filter(|p| p.hex_q == hex_q && p.hex_r == hex_r)
            .column()
            .send_result()
    }
}
```

#### 2. hex_tiles
```rust
#[spacetimedb(table)]
pub struct HexTile {
    pub hex_q: i32,
    pub hex_r: i32,
    pub hex_s: i32,            // = -(q + r)
    pub hex_id: u64,           // = (q << 32) | r (unique)
    pub terrain: TerrainType,
    pub eco_rating: u32,
    pub has_plant: bool,
    pub has_pollution: bool,
    pub plant_type: Option<PlantType>,
    pub plant_planted_at: Option<Instant>,
    pub elevation: f32,
    pub eco_rating_changes: Vec<EcoChange>,  // for audit
}

pub enum TerrainType {
    Grass,
    Forest,
    Water,
    City,
    Desert,
    Polluted,
}

pub enum PlantType {
    Wheat,
    Tree,
    RareHerb,
}
```

#### 3. vehicles
```rust
#[spacetimedb(table)]
pub struct Vehicle {
    pub vehicle_id: u32,
    pub player_id: UUID,
    pub vehicle_type: VehicleType,
    pub purchased_at: Instant,
    pub equipped: bool,
}

pub enum VehicleType {
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}
```

#### 4. cosmetics
```rust
#[spacetimedb(table)]
pub struct Cosmetic {
    pub cosmetic_id: u32,
    pub player_id: UUID,
    pub category: CosmeticCategory,
    pub cosmetic_type: CosmeticType,
    pub purchased_at: Instant,
    pub equipped: bool,
}

pub enum CosmeticCategory {
    Hat,
    Aura,
    Trail,
}

pub enum CosmeticType {
    Basic,
    Premium,
}
```

#### 5. voice_channels
```rust
#[spacetimedb(table)]
pub struct VoiceChannel {
    pub channel_id: UUID,
    pub hex_q: i32,
    pub hex_r: i32,
    pub hex_id: u64,
    pub players: Vec<UUID>,
    pub created_at: Instant,
    pub last_occupied: Instant,
    pub is_empty: bool,
    pub peer_data: Vec<u8>,      // SDP data for WebRTC
}
```

#### 6. market_listings
```rust
#[spacetimedb(table)]
pub struct MarketListing {
    pub listing_id: UUID,
    pub seller_id: UUID,
    pub title: String,
    pub description: String,
    pub github_url: String,
    pub price_usdt: u64,
    pub category: ListingCategory,
    pub published_at: Instant,
    pub expires_at: Instant,
    pub is_sold: bool,
    pub buyer_id: Option<UUID>,
}

pub enum ListingCategory {
    Agent,
    Code,
    Template,
    Snippet,
}
```

#### 7. idle_gains
```rust
#[spacetimedb(table)]
pub struct IdleGain {
    pub player_id: UUID,
    pub pending_xp: u64,
    pub pending_gold: u64,
    pub last_calculated_at: Instant,
    pub claimed_at: Option<Instant>,
}
```

#### 8. transactions
```rust
#[spacetimedb(table)]
pub struct Transaction {
    pub transaction_id: UUID,
    pub player_id: UUID,
    pub timestamp: Instant,
    pub action_type: ActionType,
    pub gold_change: i64,
    pub xp_change: i32,
    pub eco_points_change: i32,
    pub balance_after: u64,
}

pub enum ActionType {
    Plant,
    Harvest,
    Clean,
    Teleport,
    PublishListing,
    BuyListing,
    PurchaseVehicle,
    PurchaseCosmetic,
    IdleGain,
}
```

#### 9. scheduled_functions_state
```rust
#[spacetimedb(table)]
pub struct ScheduledFunctionState {
    pub function_name: String,
    pub last_run_at: Instant,
    pub next_run_at: Instant,
    pub status: FunctionStatus,
    pub error_count: u32,
}

pub enum FunctionStatus {
    Running,
    Idle,
    Error,
}
```

### Indexes
```sql
-- Automatically created by SpacetimeDB for:
-- primary keys (player_id, hex_id, etc.)
-- Foreign keys (player_id in vehicles, cosmetics, etc.)

-- Explicit indexes for performance:
CREATE INDEX idx_players_address ON players(address);
CREATE INDEX idx_hex_tiles_hex_id ON hex_tiles(hex_id);
CREATE INDEX idx_vehicles_player_id ON vehicles(player_id);
CREATE INDEX idx_cosmetics_player_id ON cosmetics(player_id);
CREATE INDEX idx_market_listings_seller_id ON market_listings(seller_id);
CREATE INDEX idx_idle_gains_player_id ON idle_gains(player_id);
```

### Replication Filters
```rust
// Only send nearby hex tiles to client
pub fn hex_tile_filter(
    db: &spacetimedb::DatabaseIndex,
    player_hex_q: i32,
    player_hex_r: i32,
) -> spacetimedb::response::GetColumn<i32, i32> {
    db.hex_tiles()
        .filter(|h| {
            manhattan_distance(h.hex_q, h.hex_r, player_hex_q, player_hex_r) <= 5
        })
        .column()
        .send_result()
}

// Only send active voice channels
pub fn voice_channel_filter(
    db: &spacetimedb::DatabaseIndex,
    player_hex_q: i32,
    player_hex_r: i32,
) -> spacetimedb::response::GetColumn<UUID> {
    db.voice_channels()
        .filter(|v| {
            v.is_empty == false
            && manhattan_distance(v.hex_q, v.hex_r, player_hex_q, player_hex_r) <= 3
        })
        .column()
        .send_result()
}
```

## Acceptance Criteria
- [ ] All 9 tables defined with correct field types
- [ ] Indexes created on address, hex_id, player_id
- [ ] Replication filters limit data to view radius
- [ ] Scheduled functions can read/write tables atomically
- [ ] Foreign keys enforced (vehicle.cosmetic → player_id)

## Risks
- R1: Large table size (hex_tiles grows with map)
- R2: Replication overhead for many small updates
- R3: Scheduled function contention

## Open Questions
- Q1: Should hex_tiles store elevation or derive from terrain?
- Q2: How often to archive transactions table?
- Q3: Should voice channels persist for reconnection?
