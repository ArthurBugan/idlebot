# Plan 005: Voice Chat System

> **Implementation Plan**

## Architecture

### Voice Channel Management
- Voice channel auto-created when 2+ players enter same hex
- Non-positional audio within hex (like a room)
- str0m WebRTC for voice transmission (future phase)
- Channel destroyed after 5 min of emptiness

### Client Integration
- Audio context initialized on startup
- Microphone permission request
- str0m WebRTC peer added/removed based on channel membership

## Files to Create/Modify

### Core (idlecore-core)
- `src/voice.rs` — VoiceChannel struct, channel creation logic, empty tracking

### Server (idlecore-server)
- `src/scheduler/voice.rs` — Scheduled cleanup (every 1 min, destroy empty channels)
- `src/voice.rs` — Voice system initialization, reduction logic

### Client (idlecore-client)
- `src/voice/voice_system.rs` — str0m WebRTC integration, audio capture/playback

## Testing Strategy
1. Unit test: VoiceChannel creation on hex occupancy
2. Unit test: Channel destruction after 5 min emptiness
3. Integration test: 2+ players in same hex creates channel
4. Edge case: Player disconnects mid-conversation

## Dependencies
- Requires 003-player-spawn (player needs hex_id tracking)
- Requires 018-multiplayer (connection management)

## Timeline
- **Estimate:** 2-3 days
- **Phase:** Post-MVP Quality of Life
