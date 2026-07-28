# Spec 021: Server-Client Protocol

> **Objective:** Define the communication protocol between Bevy client and SpacetimeDB server

## Problem Statement

The Bevy client and SpacetimeDB server need a clear protocol for input, state updates, and events. The protocol must support real-time multiplayer, handle network issues gracefully, and be efficient for the idle game loop (low packet rate, high reliability).

## Proposed Solution

- SpacetimeDB's built-in replication for table updates (primary protocol)
- Custom message types for actions (plant, harvest, clean, move)
- Pub/Sub events for level-ups, voice channel changes, marketplace updates
- Compression for large state syncs (delta encoding)

## Requirements

### Functional Requirements
1. FR1: Client sends input messages (move, plant, harvest, clean, teleport)
2. FR2: Server validates and applies actions, returns results
3. FR3: Server pushes table updates to subscribed clients (replication)
4. FR4: Server broadcasts events (level-up, voice channel created, marketplace listing)
5. FR5: Client predicts movement locally, corrects on server reply
6. FR6: Connection heartbeat every 10 seconds
7. FR7: Protocol supports message sequencing for ordering

### Non-Functional Requirements
1. NFR1: Average packet size < 500 bytes per action
2. NFR2: Protocol overhead < 10% of payload
3. NFR3: Support for message compression (gzip, lz4)
4. NFR4: Backward compatible with protocol versioning

## Design

### Protocol Overview
```
Client                                    Server
  |                                         |
  |-- InputMessage (move, plant, etc.) ----->|
  |                                         |-- Validate & Apply
  |<--------- ActionResult ------------------|
  |                                         |
  |<--------- StateUpdate (replication) -----|
  |<--------- Event (level-up, voice) ------|
  |                                         |
  |-- Heartbeat ---------------------------->|
  |<--------- HeartbeatAck ------------------|
```

### Input Messages
```rust
enum InputMessage {
    // Movement (100ms interval)
    MoveInput {
        direction: Vec2,
        vehicle_multiplier: f32,
        timestamp: Instant,
        sequence: u32,
    },
    
    // Actions (when triggered)
    PlantAction { hex_id: u64 },
    HarvestAction { hex_id: u64 },
    CleanAction { hex_id: u64 },
    TeleportAction { target_hex_q: i32, target_hex_r: i32 },
    
    // Marketplace
    ListTemplateAction {
        title: String,
        description: String,
        github_url: String,
        price_usdt: u64,
    },
    BuyListingAction { listing_id: UUID },
    
    // Vehicle/Cosmetic
    EquipVehicleAction { vehicle_id: u32 },
    EquipCosmeticAction { cosmetic_id: u32 },
    
    // Voice
    VoiceJoinAction { channel_id: UUID },
    VoiceLeaveAction { channel_id: UUID },
    
    // Heartbeat (10s interval)
    Heartbeat { timestamp: Instant },
}

// Wire format (Borsh serialization)
impl InputMessage {
    fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(self).unwrap()
    }
}
```

### Action Results
```rust
enum ActionResult {
    MoveConfirmed {
        position: Vec2,
        hex_id: u64,
        sequence: u32,
        correction: bool,  // true if client position was wrong
    },
    PlantResult {
        success: bool,
        gold_spent: u64,
        xp_gained: u32,
        error: Option<String>,
    },
    HarvestResult {
        success: bool,
        gold_earned: u64,
        xp_gained: u32,
        error: Option<String>,
    },
    CleanResult {
        success: bool,
        gold_spent: u64,
        gold_earned: u64,
        xp_gained: u32,
        eco_points_earned: u32,
        error: Option<String>,
    },
    TeleportResult {
        success: bool,
        gold_spent: u64,
        target_position: Vec2,
        error: Option<String>,
    },
    MarketResult {
        success: bool,
        listing_id: Option<UUID>,
        error: Option<String>,
    },
    VehicleResult {
        success: bool,
        error: Option<String>,
    },
    CosmeticResult {
        success: bool,
        error: Option<String>,
    },
    HeartbeatAck { timestamp: Instant, server_time: Instant },
}
```

### Server Events (Pub/Sub)
```rust
enum ServerEvent {
    // Player state changes
    PlayerJoined { player_id: UUID, address: String },
    PlayerLeft { player_id: UUID },
    PlayerPositionUpdate { player_id: UUID, position: Vec2, hex_id: u64 },
    
    // Level up
    LevelUp { player_id: UUID, new_level: u32, total_xp: u64 },
    
    // Voice channels
    VoiceChannelCreated { channel_id: UUID, hex_id: u64 },
    VoiceChannelDestroyed { channel_id: UUID },
    VoiceParticipantJoined { channel_id: UUID, player_id: UUID },
    VoiceParticipantLeft { channel_id: UUID, player_id: UUID },
    
    // Marketplace
    ListingPublished { listing_id: UUID, seller_id: UUID, price_usdt: u64 },
    ListingSold { listing_id: UUID, buyer_id: UUID, seller_id: UUID },
    ListingExpired { listing_id: UUID },
    
    // Eco actions
    EcoPointsEarned { player_id: UUID, hex_id: u64, points: u32, action: String },
    HexEcoRatingUpdated { hex_id: u64, new_rating: i32 },
    
    // Idle gains
    IdleGainsClaimed { player_id: UUID, xp: u64, gold: u64 },
}
```

### Replication Filter Strategy
```rust
// SpacetimeDB handles replication via table subscriptions
// Client subscribes to relevant slices:

pub mod player_state_filter {
    pub struct PlayerStateFilter {
        pub my_hex_q: i32,
        pub my_hex_r: i32,
        pub view_radius: u32,  // 3 hexes
    }
    
    pub fn should_include(
        &self,
        player_hex_q: i32,
        player_hex_r: i32,
    ) -> bool {
        manhattan_distance(
            self.my_hex_q, self.my_hex_r,
            player_hex_q, player_hex_r,
        ) <= self.view_radius
    }
}

pub mod voice_channel_filter {
    pub struct VoiceChannelFilter {
        pub my_hex_q: i32,
        pub my_hex_r: i32,
        pub view_radius: u32,
    }
    
    pub fn should_include(
        &self,
        channel_hex_q: i32,
        channel_hex_r: i32,
        is_active: bool,
    ) -> bool {
        if !is_active {
            return false;
        }
        manhattan_distance(
            self.my_hex_q, self.my_hex_r,
            channel_hex_q, channel_hex_r,
        ) <= self.view_radius
    }
}
```

### Message Compression
```rust
use lz4_flex::compress_prepend_len;
use lz4_flex::decompress_prepend_len;

impl InputMessage {
    fn serialize_compressed(&self) -> Vec<u8> {
        let uncompressed = borsh::to_vec(self).unwrap();
        compress_prepend_len(&uncompressed)
    }
}

impl ActionResult {
    fn deserialize_compressed(data: &[u8]) -> Self {
        let uncompressed = decompress_prepend_len(data).unwrap();
        borsh::from_slice(&uncompressed).unwrap()
    }
}
```

### Sequence Numbering for Ordering
```rust
struct MessageSequence {
    client_sequence: u32,
    server_sequence: u32,
}

impl MessageSequence {
    fn next_client(&mut self) -> u32 {
        self.client_sequence += 1;
        self.client_sequence
    }
    
    fn check_order(&self, expected: u32, received: u32) -> bool {
        received == expected || received == expected + 1  // Allow 1 skip
    }
}
```

## Acceptance Criteria
- [ ] Client sends input messages with correct format
- [ ] Server validates and returns action results
- [ ] State updates replicate to subscribed clients
- [ ] Events broadcast correctly (level-up, voice, marketplace)
- [ ] Client predicts movement, corrects on server reply
- [ ] Heartbeat every 10 seconds with ack
- [ ] Message compression reduces size by >50%
- [ ] Protocol supports versioning for future changes

## Risks
- R1: Borsh serialization overhead for large messages
- R2: Sequence number wraparound (u32 max)
- R3: Compression compatibility across protocol versions

## Open Questions
- Q1: Should there be a priority queue for messages (movement > actions > heartbeats)?
- Q2: Is Borsh the best serialization format vs. CBOR or FlatBuffers?
- Q3: Should voice data use separate channel (already does via WebRTC)?
