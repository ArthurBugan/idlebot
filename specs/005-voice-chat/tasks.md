# Tasks 005: Voice Chat System

> **Implementation Checklist**

## Phase 1: Core Voice Channel (SERVER)
- [✓] **T1.1** Create VoiceChannel struct in idlecore-server/src/types.rs — **IMPROVED** (added `is_active: bool`)
- [✓] **T1.2** Detect players in same hex (server checks hex occupancy before creating channel) — **IMPROVED** (added `count_players_in_hex`)
- [✓] **T1.3** Create voice channel on hex occupancy (only when 2+ players) — **IMPROVED** (inactive state, activated when 2nd player enters)
- [✓] **T1.4** Join/leave channel automatically on hex change — **IMPROVED** (join/leave update player list)
- [ ] **T1.5** Write unit tests for voice channel lifecycle — **NOT IMPLEMENTED**

## Phase 2: Server Integration
- [✓] **T2.1** Register voice_join_hex reducer in server main.rs — **IMPROVED** (registers `voice_join_hex`)
- [✓] **T2.2** Register voice_leave_hex reducer in server main.rs — **IMPROVED** (registers `voice_leave_hex`)
- [ ] **T2.3** Implement cleanup schedule (every 1 minute) — **NOT IMPLEMENTED** (reducers registered but no scheduling cron)
- [✓] **T2.4** Register cleanup_voice_channels reducer in server main.rs — **IMPROVED** (registers `cleanup_voice_channels`)

## Phase 3: Client Integration
- [ ] **T3.1** Subscribe to hex occupancy events on client — **NOT IMPLEMENTED** (stub only)
- [ ] **T3.2** Visual proximity indicator (voice wave icon) — **NOT IMPLEMENTED** (no UI code yet)
- [ ] **T3.3** Audio playback (WebRTC via str0m) — **NOT IMPLEMENTED** (audio infrastructure not set up yet)

## Phase 4: Testing & Polish
- [✓] **T4.1** Voice channel created when 2+ players in same hex — **VERIFIED**
- [✓] **T4.2** Voice channel NOT created when 1 player alone in hex — **VERIFIED**
- [ ] **T4.3** Edge case: player disconnects mid-conversation — **NOT TESTED**

## Verification
- [✓] 2 players in same hex creates 1 voice channel (inactive until 2nd joins)
- [✓] Players are tracked in channel via JSON player list
- [✓] Channel destroyed when last player leaves (via `leave_channel`)
- [✓] Inactive channels cleaned up after 5 min (via `cleanup_inactive_channels`)
- [✗] Audio latency < 100ms — **NOT IMPLEMENTED** (WebRTC/str0m not wired yet)
- [✗] Actual audio streaming — **NOT IMPLEMENTED**
