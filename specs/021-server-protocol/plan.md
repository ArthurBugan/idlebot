# Plan 021: Server-Client Protocol

> **Implementation strategy**: Define message types, implement serialization (Borsh), set up WebSocket transport, add protocol version negotiation, compression, movement prediction, and connection recovery.

## Verification Steps
1. Define InputMessage enum: MoveInput, PlantAction, HarvestAction, CleanAction, TeleportAction, Heartbeat
2. Define ActionResult enum: MoveConfirmed, PlantResult, HarvestResult, CleanResult, TeleportResult, Failed
3. Define ServerEvent enum: PlayerJoined, PlayerLeft, LevelUp, VoiceChannelCreated, VoiceChannelDestroyed, MarketplaceEvent, EcoPointsEvent
4. Implement Borsh serialization for all message types
5. Implement Borsh deserialization (round-trip test)
6. Define protocol version header (first 1 byte)
7. Define message sequence numbers (uint32, first field in each message)
8. Implement lz4 compression for messages
9. Set up WebSocket transport for custom messages (in addition to SpacetimeDB replication)
10. Implement client-side movement prediction
11. Implement server correction when client diverges
12. Implement view filtering (only send nearby players, voice channels)
13. Add heartbeat every 10 seconds on client
14. Add heartbeatAck response on server
15. Implement connection recovery on disconnect
16. Test serialization round-trip for all message types
17. Test protocol version negotiation
18. Test compression efficiency (>50% reduction)
19. Test movement prediction accuracy
20. Test server correction triggers correctly

## Implementation Order
1. Define enums (InputMessage, ActionResult, ServerEvent) in server/src/types.rs
2. Implement Borsh Serialize/Deserialize traits
3. Implement protocol version header
4. Implement sequence numbers
5. Implement compression (lz4)
6. Set up WebSocket transport
7. Implement client-side movement prediction
8. Implement server-side correction
9. Implement view filtering
10. Add heartbeat/heartbeatAck
11. Implement connection recovery
12. Test all components
