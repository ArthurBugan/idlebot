use bevy::prelude::*;
use super::voice_indicator::VoiceIndicator;

/// Bevy System: Updates the local VoiceIndicator component.
/// Stub: In a full implementation, this would process server events and
/// map them to ECS state changes for FR5 compliance.
pub fn voice_indicator_updater(
    indicators: Query<&mut VoiceIndicator>,
) {
    // TODO: Process server events and update indicators
    let _ = indicators;
    println!("Voice Updater: running stub.");
}
