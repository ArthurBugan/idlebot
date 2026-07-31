# Tasks 021: Server-Client Protocol

> **Implementation Checklist**

## Phase 1: Data Model
- [ ] **T1.1** Define InputMessage enum (MoveInput, PlantAction, HarvestAction, CleanAction, TeleportAction, ListTemplateAction, BuyListingAction, EquipVehicleAction, EquipCosmeticAction, VoiceJoinAction, VoiceLeaveAction, Heartbeat)
- [ ] **T1.2** Define ActionResult enum (MoveConfirmed, PlantResult, HarvestResult, CleanResult, TeleportResult, MarketResult, VehicleResult, CosmeticResult, HeartbeatAck)
- [ ] **T1.3** Define ServerEvent enum (PlayerJoined, PlayerLeft, PlayerPositionUpdate, LevelUp, VoiceChannelCreated, VoiceChannelDestroyed, VoiceParticipantJoined, VoiceParticipantLeft, ListingPublished, ListingSold, ListingExpired, EcoPointsEarned, HexEcoRatingUpdated, IdleGainsClaimed)
- [ ] **T1.4** Implement Borsh serialization for all message types

## Phase 2: Sequence Numbering
- [ ] **T2.1** Define MessageSequence struct (client_sequence, server_sequence)
- [ ] **T2.2** Implement next_client() — increment and return
- [ ] **T2.3** Implement check_order() — allow 1 skip (received == expected || received == expected + 1)

## Phase 3: Server Handlers
- [ ] **T3.1** Implement handle_input_message() — route to action handlers
- [ ] **T3.2** Implement validate_input() — check sequence, permissions
- [ ] **T3.3** Implement validate_move() — within grid, no conflict, speed limit
- [ ] **T3.4** Implement validate_action() — check economy (gold, USDT)
- [ ] **T3.5** Implement handle_heartbeat() — check connection alive, return ack

## Phase 4: Client-Side Serialization
- [ ] **T4.1** Implement serialize_input_message() in client
- [ ] **T4.2** Implement deserialize_action_result() in client
- [ ] **T4.3** Implement deserialize_server_event() in client
- [ ] **T4.4** Handle protocol version mismatch

## Phase 5: Message Compression
- [ ] **T5.1** Implement serialize_compressed() using lz4
- [ ] **T5.2** Implement decompress_prepend_len() for deserialization
- [ ] **T5.3** Measure compression ratio (target > 50%)

## Phase 6: Replication Filter
- [ ] **T6.1** Implement player_state_filter() — manhattan_distance ≤ 3
- [ ] **T6.2** Implement hex_tile_filter() — manhattan_distance ≤ 5
- [ ] **T6.3** Implement voice_channel_filter() — active only, distance ≤ 3

## Phase 7: Testing
- [ ] **T7.1** Client sends input messages with correct format
- [ ] **T7.2** Server validates and returns action results
- [ ] **T7.3** State updates replicate to subscribed clients
- [ ] **T7.4** Events broadcast correctly
- [ ] **T7.5** Client predicts movement, corrects on server reply
- [ ] **T7.6** Heartbeat every 10 seconds with ack
- [ ] **T7.7** Message compression reduces size by > 50%
- [ ] **T7.8** Protocol supports versioning

## Verification
- [✓] All message types defined with Borsh serialization
- [✓] Sequence numbering prevents replay attacks
- [✓] Heartbeat mechanism keeps connections alive
