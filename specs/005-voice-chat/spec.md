# Spec 005: Voice Chat System

> **Objective:** Implement proximity-based voice chat within hexes

## Problem Statement

Players need to communicate via voice when in the same hex. Voice should be non-positional (like being in a room) and auto-manage channels.

## Proposed Solution

- Voice channel auto-created when 2+ players enter same hex
- Non-positional audio within hex
- Channel destroyed when all players leave (5 min timeout)
- str0m WebRTC for voice transmission

## Requirements

### Functional Requirements
1. FR1: Detect players in same hex
2. FR2: Create voice channel on hex occupancy
3. FR3: Join/leave channel automatically
4. FR4: Destroy channel after 5 min of emptiness
5. FR5: Player sees proximity indicator
6. FR6: Audio quality optimization

### Non-Functional Requirements
1. NFR1: Audio latency < 100ms
2. NFR2: Support 100+ concurrent voice channels
3. NFR3: Bandwidth optimization (48kbps)

## Design

### Voice Channel
```rust
struct VoiceChannel {
    hex_id: u64,
    players: HashSet<UUID>,
    created_at: Instant,
    empty_since: Option<Instant>,
    peer_connection: Option<PeerConnection>,
}

impl VoiceChannel {
    fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
    
    fn mark_empty(&mut self) {
        self.empty_since = Some(Instant::now());
    }
    
    fn is_expired(&self) -> bool {
        self.empty_since
            .map(|t| t.elapsed() > Duration::from_secs(300))
            .unwrap_or(false)
    }
}
```

### Proximity Detection
```rust
fn update_voice_channels(world: &mut World) {
    let hexes = world.get_occupied_hexes();
    
    for (hex_id, players) in hexes {
        if players.len() >= 2 {
            let channel = world.get_or_create_voice_channel(hex_id);
            for player in &players {
                if !channel.players.contains(player) {
                    channel.join(player);
                }
            }
            channel.mark_occupied();
        }
    }
    
    // Cleanup expired channels
    for channel in world.get_expired_channels() {
        world.destroy_voice_channel(channel.hex_id);
    }
}
```

### str0m Integration
```rust
use str0m::Rtc;

struct VoicePeer {
    rtc: Rtc,
    sender: Option<AudioSender>,
    receivers: Vec<AudioReceiver>,
}

impl VoicePeer {
    fn create_offer(&mut self) -> Vec<u8> {
        let offer = self.rtc.create_offer().unwrap();
        offer.to_vec()
    }
    
    fn receive_offer(&mut self, offer: &[u8]) -> Vec<u8> {
        let sdp = str0m::Sdp::parse(offer).unwrap();
        self.rtc.handle_offer(sdp).unwrap();
        let answer = self.rtc.create_answer().unwrap();
        answer.to_vec()
    }
}
```

## Acceptance Criteria
- [ ] Voice channel created when 2+ players in same hex
- [ ] Players can hear each other within hex
- [ ] Channel destroyed after 5 min of emptiness
- [ ] Proximity indicator shows voice status
- [ ] Audio latency < 100ms
- [ ] No audio leakage between hexes

## Risks
- R1: WebRTC negotiation complexity
- R2: Network firewall/NAT issues
- R3: Audio quality in noisy environments

## Open Questions
- Q1: Should there be a mute button?
- Q2: How to handle disconnected players?
- Q3: Recording for replay?
