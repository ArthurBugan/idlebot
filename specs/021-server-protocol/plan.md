# Plan 021: Server-Client Protocol

> **Implementation Plan**

## Architecture

### Protocol Design
- SpacetimeDB replication as primary protocol
- Custom message types for actions (move, plant, harvest, clean, teleport, etc.)
- Pub/Sub events for level-ups, voice, marketplace
- Borsh serialization for wire format
- Optional lz4 compression for large messages

### Message Flow
1. Client → Server: InputMessage (move 100ms, actions on trigger)
2. Server → Client: ActionResult (validated result)
3. Server → Client: StateUpdate (replication, filtered by view radius)
4. Server → Client: Event (level-up, voice, marketplace)
5. Client → Server: Heartbeat (every 10s)

### Sequence Numbering
- Client tracks sequence for ordering
- Server validates sequence (allow 1 skip)
- Prevents replay attacks

## Files to Create

### Core (idlecore-core)
- Create `src/protocol/messages.rs` — InputMessage, ActionResult, ServerEvent enums with Borsh serialization

### Server (idlecore-server)
- Modify `src/main.rs` — Wire message handlers, sequence tracking
- Create `src/protocol/handlers.rs` — Action validation and response generation

### Client (idlecore-client)
- Modify `src/input.rs` — Serialize input messages, handle responses
- Create `src/protocol/event_handler.rs` — Process server events
- Modify `src/lib.rs` — Add protocol version, heartbeat

## Dependencies
- Requires 013-wallet-auth (connection flow)
- Requires 009-minimap (position updates)
- Requires 010-economy (action validation)
- Requires 005-voice-chat (voice events)
- Requires 011-marketplace (marketplace events)

## Testing Strategy
1. Unit test: Borsh serialization/deserialization roundtrip
2. Unit test: InputMessage contains all action variants
3. Integration test: Client sends move → server corrects position
4. Integration test: Level-up event broadcasts to all clients
5. Edge case: Message compression reduces size > 50%
6. Edge case: Sequence number wraparound (u32 max)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** Phase 3 (Protocol)
- **Blocked Until:** Core protocol types (after 010-economy, 013-wallet-auth are complete)
