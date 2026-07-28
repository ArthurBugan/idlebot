pub mod indicator;
pub mod ui;
pub mod update;

// Export core types for consumers of the voice module
pub use indicator::{VoiceIndicator, VoiceChannelState};
// Assuming voice_system.rs contains the higher-level domain logic:
pub use crate::voice_system::VoiceChannelEvent;
