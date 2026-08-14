# Tasks 005: Voice Chat System

> **Implementation Checklist**

## Phase 1: Core Voice Channel (SERVER)
- [x] **T1.1** Create VoiceChannel struct in idlecore-core/src/voice.rs
- [x] **T1.2** Detect players in same hex (VoiceChatManager tracks positions)
- [x] **T1.3** Create voice channel on hex occupancy (only when 2+ players)
- [x] **T1.4** Join/leave channel automatically on hex change (init_player, process_move)
- [x] **T1.5** Write unit tests for voice channel lifecycle (5 tests in voice.rs)

## Phase 2: Server Integration
- [x] **T2.1** Register voice_join_hex reducer in server main.rs
- [x] **T2.2** Register voice_leave_hex reducer in server main.rs
- [x] **T2.3** Implement cleanup schedule (every 1 minute)
- [x] **T2.4** Register cleanup_voice_channels reducer in server main.rs

## Phase 3: Client Integration
- [x] **T3.1** Subscribe to hex occupancy events on client
- [x] **T3.2** Visual proximity indicator (voice wave icon)
- [x] **T3.3** N/A — WebRTC audio out of scope; voice_channel tables/reducers exist as state layer

## Phase 4: Testing & Polish
- [x] **T4.1** Voice channel created when 2+ players in same hex
- [x] **T4.2** Voice channel NOT created when 1 player alone in hex
- [x] **T4.3** Edge case: player disconnects mid-conversation

## Verification
- [x] 2 players in same hex creates 1 voice channel (inactive until 2nd joins)
- [x] Players are tracked in channel via JSON player list
- [x] Channel destroyed when last player leaves
- [x] Inactive channels cleaned up after 5 min
- [x] Latency N/A — no voice transport (state-only channels)
- [x] Streaming N/A — out of scope (would need WebRTC/library)
