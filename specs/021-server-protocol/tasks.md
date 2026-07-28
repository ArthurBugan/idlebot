# Tasks 021: Server-Client Protocol

> **Implementation Checklist**

## Phase 1: Protocol Design
- [✓] **T1.1** Define InputMessage enum (MoveInput, PlantAction, HarvestAction, CleanAction, TeleportAction, Heartbeat) — **COMPLETE** (reducers in main.rs)
- [✓] **T1.2** Define ActionResult enum (MoveConfirmed, PlantResult, HarvestResult, CleanResult, TeleportResult) — **COMPLETE** (world.rs return types)
- [✓] **T1.3** Define ServerEvent enum (PlayerJoined, PlayerLeft, LevelUp, VoiceChannelCreated/Destroyed, Marketplace events, Eco events) — **COMPLETE** (events in voice.rs/market.rs)
- [ ] **T1.4** Define message wire format (Borsh serialization) — **NOT IMPLEMENTED**
- [ ] **T1.5** Define protocol version header — **NOT IMPLEMENTED**
- [ ] **T1.6** Define message sequence numbers for ordering — **NOT IMPLEMENTED**

## Phase 2: Serialization
- [ ] **T1.7** Serialize InputMessage to bytes (Borsh) — **NOT IMPLEMENTED**
- [ ] **T1.8** Deserialize ActionResult from bytes — **NOT IMPLEMENTED**
- [ ] **T1.9** Serialize ServerEvent to bytes — **NOT IMPLEMENTED**
- [ ] **T1.10** Deserialize ServerEvent from bytes — **NOT IMPLEMENTED**
- [ ] **T1.11** Message compression (lz4) — **NOT IMPLEMENTED**

## Phase 3: Transport
- [✓] **T1.12** SpacetimeDB handles replication (primary transport) — **COMPLETE** (SpacetimeDB built-in)
- [ ] **T1.13** Custom WebSockets for input messages — **NOT IMPLEMENTED**
- [ ] **T1.14** WebSockets for server events — **NOT IMPLEMENTED**
- [ ] **T1.15** Heartbeat every 10 seconds — **NOT IMPLEMENTED**
- [ ] **T1.16** HeartbeatAck response — **NOT IMPLEMENTED**
- [ ] **T1.17** Connection recovery on disconnect — **NOT IMPLEMENTED**

## Phase 4: Movement Prediction
- [ ] **T1.18** Client-side movement prediction — **NOT IMPLEMENTED**
- [ ] **T1.19** Server correction on divergence — **NOT IMPLEMENTED**
- [ ] **T1.20** Client receives correction packet — **NOT IMPLEMENTED**

## Phase 5: View Filtering
- [ ] **T1.21** Replication filter: only send nearby players (≤3 hexes) — **NOT IMPLEMENTED**
- [ ] **T1.22** Replication filter: only send active voice channels — **NOT IMPLEMENTED**

## Phase 6: Protocol Tests
- [ ] **T1.23** Test: InputMessage serialization round-trip
- [ ] **T1.24** Test: ActionResult deserialization round-trip
- [ ] **T1.25** Test: ServerEvent serialization round-trip
- [ ] **T1.26** Test: Sequence numbers increment correctly
- [ ] **T1.27** Test: Compression reduces size by >50%
