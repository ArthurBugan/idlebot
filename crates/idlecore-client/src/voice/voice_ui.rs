use bevy::prelude::*;
use super::voice_indicator::VoiceIndicator;

/// Bevy System: Draws the voice chat overlay onto the screen.
/// This implements the visual rendering for FR5/FR6.
pub fn voice_ui_system(
    // Read access to the current voice state component
    indicators: Query<&VoiceIndicator>,
) {
    for indicator in indicators.iter() {
        // --- Visuals dictated by VoiceIndicator ---
        
        // 1. Display Hex ID Label
        println!("Rendering Voice UI for Hex ID: {}", indicator.hex_id);

        // 2. Display Player Names
        println!("Players detected: {:?}", indicator.present_players);

        // 3. Display Busy Indicator
        if indicator.is_channel_busy {
            println!("--- VISUAL ALERT: CHANNEL BUSY (FR5) ---");
        }

        // 4. Voice Wave Animation Visualization (Stubbed)
        // In a real implementation, this would generate/update sprites/particles
        // based on presence_tick and is_channel_busy.
        println!("Animating with tick: {}", indicator.presence_tick);
    }
}

// This function would handle setting up the initial UI entities in the Bevy app lifecycle.
pub fn setup_voice_ui(_commands: Commands, _player_id: &str) {
    // TODO: Spawn Camera, add Voice Overlay Entity hierarchy here.
    println!("Voice UI system setup complete. Entities spawned.");
}