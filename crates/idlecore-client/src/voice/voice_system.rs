//! Voice Chat — Canal por Hexágono (WebRTC)

use bevy::prelude::*;
use std::collections::HashMap;

/// Componente de voice channel
#[derive(Component)]
pub struct VoiceChannel {
    pub hex_id: u64,
    pub players: Vec<String>,
}

/// Componente de transform do hex (pro voice system)
#[derive(Component)]
pub struct HexTransform {
    pub hex_id: u64,
    pub center_x: f32,
    pub center_y: f32,
}

/// Evento de voice channel
#[derive(Event)]
pub enum VoiceChannelEvent {
    Join(u64, String),
    Leave(u64, String),
}

/// Sistema que gerencia joins/leaves de voice channels
pub fn update_voice_channels(
    events: EventReader<VoiceChannelEvent>,
    mut commands: Commands,
    channel_query: Query<(Entity, &VoiceChannel)>,
    hex_query: Query<&HexTransform>,
) {
    for event in events.read() {
        match event {
            VoiceChannelEvent::Join(hex_id, player_addr) => {
                let channel = channel_query.iter().find(|(_, ch)| ch.hex_id == *hex_id);

                if let Some((entity, _)) = channel {
                    commands.entity(entity).insert(VoiceChannelMembers {
                        members: Vec::new(),
                    });
                } else {
                    commands.spawn((
                        VoiceChannel,
                        Name::new(format!("voice_{}", hex_id)),
                        VoiceChannelMembers {
                            members: vec![player_addr.clone()],
                        },
                    ));
                }
            }
            VoiceChannelEvent::Leave(hex_id, player_addr) => {
                if let Some((entity, _)) = channel_query.iter().find(|(_, ch)| ch.hex_id == *hex_id)
                {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

#[derive(Component)]
pub struct VoiceChannelMembers {
    pub members: Vec<String>,
}

/// WebRTC Manager — Peer-to-Peer voice chat
pub struct WebRtcManager {
    local_peer: Option<str0m::Stream>,
    remote_peers: HashMap<String, str0m::Stream>,
}

impl WebRtcManager {
    pub fn new() -> Self {
        Self {
            local_peer: None,
            remote_peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_address: String) {
        // Criar peer connection com oferta SDP
        // Enviar ICE candidates via SpacetimeDB voice_join event
    }

    pub fn remove_peer(&mut self, peer_address: &str) {
        self.remote_peers.remove(peer_address);
    }

    pub fn send_audio(&self, data: &[u8]) {
        // Enviar via DataChannel ou media track
    }

    pub fn on_remote_audio(&self, peer_address: &str, data: &[u8]) {
        // Play audio locally
    }
}
