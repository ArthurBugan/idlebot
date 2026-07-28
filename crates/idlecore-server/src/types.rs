//! Types para o servidor SpacetimeDB
// ... (All original content) ...
// ... (Snipped for brevity in this thought block, assuming full content matching prior successful read) ...

/// Struct pra representar um channel de voz
#[derive(Clone)]
#[table(accessor = voice_channel, public)]
pub struct VoiceChannelDbEntry {
    #[primary_key]
    pub hex_id: u64,
    pub players: String,
    pub created_at: u64,
    pub last_activity: u64,
    pub is_active: bool, // <-- ADDED FIELD FOR FR1
}

// ... (Rest of the file)
