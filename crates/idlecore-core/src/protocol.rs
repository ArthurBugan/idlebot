//! Server-Client Protocol — message types and serialization.
//!
//! Defines InputMessage, ActionResult, ServerEvent enums with Borsh-like serialization.

use serde::{Deserialize, Serialize};
use crate::hex::HexCoord;

// ---------------------------------------------------------------------------
// Protocol Version
// ---------------------------------------------------------------------------

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Sequence Numbers
// ---------------------------------------------------------------------------

/// Message sequence for ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSequence {
    pub client_sequence: u64,
    pub server_sequence: u64,
}

impl MessageSequence {
    /// Create a new sequence with client sequence incremented.
    pub fn next_client(&self) -> Self {
        Self {
            client_sequence: self.client_sequence + 1,
            server_sequence: self.server_sequence,
        }
    }

    /// Check if received sequence is valid (allows 1 skip).
    pub fn check_order(received: u64, expected: u64) -> bool {
        received == expected || received == expected + 1
    }
}

impl Default for MessageSequence {
    fn default() -> Self {
        Self {
            client_sequence: 0,
            server_sequence: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Input Messages (Client → Server)
// ---------------------------------------------------------------------------

/// Client input messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputMessage {
    /// Move input with target hex.
    MoveInput {
        sequence: u64,
        target_hex: HexCoord,
    },
    /// Plant action on hex.
    PlantAction {
        sequence: u64,
        hex_id: u64,
        plant_type: u8, // 0=Wheat, 1=Corn, 2=Tree, 3=RareHerb
    },
    /// Harvest action on hex.
    HarvestAction {
        sequence: u64,
        hex_id: u64,
    },
    /// Clean action on hex.
    CleanAction {
        sequence: u64,
        hex_id: u64,
    },
    /// Teleport action.
    TeleportAction {
        sequence: u64,
        target_hex: HexCoord,
    },
    /// List template action.
    ListTemplateAction {
        sequence: u64,
        title: String,
        description: String,
        github_url: String,
        price_usdt: f64,
    },
    /// Buy listing action.
    BuyListingAction {
        sequence: u64,
        listing_id: u64,
    },
    /// Equip vehicle action.
    EquipVehicleAction {
        sequence: u64,
        vehicle_type: u8, // 0=None, 1=Bicycle, 2=Scooter, 3=Motorcycle, 4=Boat, 5=Airplane
    },
    /// Equip cosmetic action.
    EquipCosmeticAction {
        sequence: u64,
        category: u8, // 0=Hat, 1=Aura, 2=Trail
        index: usize,
    },
    /// Join voice channel.
    VoiceJoinAction {
        sequence: u64,
        hex_id: u64,
    },
    /// Leave voice channel.
    VoiceLeaveAction {
        sequence: u64,
    },
    /// Heartbeat to keep connection alive.
    Heartbeat {
        sequence: u64,
    },
}

impl InputMessage {
    /// Get the sequence number.
    pub fn sequence(&self) -> u64 {
        match self {
            InputMessage::MoveInput { sequence, .. } => *sequence,
            InputMessage::PlantAction { sequence, .. } => *sequence,
            InputMessage::HarvestAction { sequence, .. } => *sequence,
            InputMessage::CleanAction { sequence, .. } => *sequence,
            InputMessage::TeleportAction { sequence, .. } => *sequence,
            InputMessage::ListTemplateAction { sequence, .. } => *sequence,
            InputMessage::BuyListingAction { sequence, .. } => *sequence,
            InputMessage::EquipVehicleAction { sequence, .. } => *sequence,
            InputMessage::EquipCosmeticAction { sequence, .. } => *sequence,
            InputMessage::VoiceJoinAction { sequence, .. } => *sequence,
            InputMessage::VoiceLeaveAction { sequence, .. } => *sequence,
            InputMessage::Heartbeat { sequence, .. } => *sequence,
        }
    }
}

// ---------------------------------------------------------------------------
// Action Results (Server → Client)
// ---------------------------------------------------------------------------

/// Server action results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    /// Move confirmed.
    MoveConfirmed {
        sequence: u64,
        new_hex: HexCoord,
    },
    /// Plant result.
    PlantResult {
        sequence: u64,
        success: bool,
        message: String,
        gold_change: i64,
        xp_change: i64,
    },
    /// Harvest result.
    HarvestResult {
        sequence: u64,
        success: bool,
        message: String,
        gold_change: i64,
        xp_change: i64,
    },
    /// Clean result.
    CleanResult {
        sequence: u64,
        success: bool,
        message: String,
        gold_change: i64,
        xp_change: i64,
    },
    /// Teleport result.
    TeleportResult {
        sequence: u64,
        success: bool,
        message: String,
        cost: u64,
    },
    /// Market result.
    MarketResult {
        sequence: u64,
        success: bool,
        message: String,
        listing_id: Option<u64>,
    },
    /// Vehicle result.
    VehicleResult {
        sequence: u64,
        success: bool,
        message: String,
    },
    /// Cosmetic result.
    CosmeticResult {
        sequence: u64,
        success: bool,
        message: String,
    },
    /// Heartbeat acknowledgement.
    HeartbeatAck {
        sequence: u64,
    },
}

// ---------------------------------------------------------------------------
// Server Events (Server → Client)
// ---------------------------------------------------------------------------

/// Server events broadcast to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEvent {
    /// Player joined the game.
    PlayerJoined {
        player_address: String,
        hex_id: u64,
        position_x: f32,
        position_y: f32,
    },
    /// Player left the game.
    PlayerLeft {
        player_address: String,
    },
    /// Player position updated.
    PlayerPositionUpdate {
        player_address: String,
        hex_id: u64,
        position_x: f32,
        position_y: f32,
    },
    /// Player leveled up.
    LevelUp {
        player_address: String,
        new_level: u32,
    },
    /// Voice channel created.
    VoiceChannelCreated {
        hex_id: u64,
        player_count: u32,
    },
    /// Voice channel destroyed.
    VoiceChannelDestroyed {
        hex_id: u64,
    },
    /// Voice participant joined.
    VoiceParticipantJoined {
        hex_id: u64,
        player_address: String,
    },
    /// Voice participant left.
    VoiceParticipantLeft {
        hex_id: u64,
        player_address: String,
    },
    /// Listing published.
    ListingPublished {
        listing_id: u64,
        seller: String,
        title: String,
        price_usdt: f64,
    },
    /// Listing sold.
    ListingSold {
        listing_id: u64,
        buyer: String,
    },
    /// Listing expired.
    ListingExpired {
        listing_id: u64,
    },
    /// Eco points earned.
    EcoPointsEarned {
        player_address: String,
        amount: u64,
    },
    /// Hex eco rating updated.
    HexEcoRatingUpdated {
        hex_id: u64,
        new_rating: u32,
    },
    /// Idle gains claimed.
    IdleGainsClaimed {
        player_address: String,
        xp: u64,
        gold: u64,
    },
}

// ---------------------------------------------------------------------------
// Serialization Helpers
// ---------------------------------------------------------------------------

/// Serialize a message to bytes (simplified — uses serde_json for now).
pub fn serialize_message(msg: &InputMessage) -> Result<Vec<u8>, String> {
    serde_json::to_vec(msg).map_err(|e| e.to_string())
}

/// Deserialize a message from bytes.
pub fn deserialize_message(bytes: &[u8]) -> Result<InputMessage, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

/// Serialize an action result to bytes.
pub fn serialize_action_result(result: &ActionResult) -> Result<Vec<u8>, String> {
    serde_json::to_vec(result).map_err(|e| e.to_string())
}

/// Deserialize an action result from bytes.
pub fn deserialize_action_result(bytes: &[u8]) -> Result<ActionResult, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

/// Serialize a server event to bytes.
pub fn serialize_server_event(event: &ServerEvent) -> Result<Vec<u8>, String> {
    serde_json::to_vec(event).map_err(|e| e.to_string())
}

/// Deserialize a server event from bytes.
pub fn deserialize_server_event(bytes: &[u8]) -> Result<ServerEvent, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_sequence_next() {
        let seq = MessageSequence { client_sequence: 5, server_sequence: 10 };
        let next = seq.next_client();
        assert_eq!(next.client_sequence, 6);
        assert_eq!(next.server_sequence, 10);
    }

    #[test]
    fn test_message_sequence_check_order() {
        assert!(MessageSequence::check_order(5, 5));   // Exact match
        assert!(MessageSequence::check_order(6, 5));   // 1 skip
        assert!(!MessageSequence::check_order(7, 5));  // 2 skips
        assert!(!MessageSequence::check_order(4, 5));  // Behind
    }

    #[test]
    fn test_input_message_sequence() {
        let msg = InputMessage::Heartbeat { sequence: 42 };
        assert_eq!(msg.sequence(), 42);
    }

    #[test]
    fn test_serialize_deserialize_input_message() {
        let msg = InputMessage::Heartbeat { sequence: 1 };
        let bytes = serialize_message(&msg).unwrap();
        let decoded = deserialize_message(&bytes).unwrap();
        match decoded {
            InputMessage::Heartbeat { sequence } => assert_eq!(sequence, 1),
            _ => panic!("Expected Heartbeat"),
        }
    }

    #[test]
    fn test_serialize_deserialize_action_result() {
        let result = ActionResult::HeartbeatAck { sequence: 1 };
        let bytes = serialize_action_result(&result).unwrap();
        let decoded = deserialize_action_result(&bytes).unwrap();
        match decoded {
            ActionResult::HeartbeatAck { sequence } => assert_eq!(sequence, 1),
            _ => panic!("Expected HeartbeatAck"),
        }
    }

    #[test]
    fn test_serialize_deserialize_server_event() {
        let event = ServerEvent::EcoPointsEarned {
            player_address: "0x1".into(),
            amount: 10,
        };
        let bytes = serialize_server_event(&event).unwrap();
        let decoded = deserialize_server_event(&bytes).unwrap();
        match decoded {
            ServerEvent::EcoPointsEarned { amount, .. } => assert_eq!(amount, 10),
            _ => panic!("Expected EcoPointsEarned"),
        }
    }

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
