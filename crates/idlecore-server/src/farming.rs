//! Sistema de Farming -- Plantar/Colher (Stardew Valley style)
//!
//! Thin wrappers around world::interact_hex for convenience.

use std::time::{SystemTime, UNIX_EPOCH};

/// Plantar semente num hexágono
pub fn plant_seed(
    ctx: &spacetimedb::ReducerContext,
    player_address: &str,
    hex_id: u64,
    plant_type: &str,
) -> crate::world::ActionResult {
    crate::world::interact_hex(ctx, player_address, hex_id, "plant", Some(plant_type.to_string()))
        .unwrap_or(crate::world::ActionResult::Failed {
            reason: "Plant action failed".to_string(),
        })
}

/// Colher planta pronta
pub fn harvest(
    ctx: &spacetimedb::ReducerContext,
    player_address: &str,
    hex_id: u64,
) -> crate::world::ActionResult {
    crate::world::interact_hex(ctx, player_address, hex_id, "harvest", None)
        .unwrap_or(crate::world::ActionResult::Failed {
            reason: "Harvest action failed".to_string(),
        })
}

/// Atualizar crescimento de plantas (server-side timer)
pub fn update_plant_growth() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Delegate to the world module's plant growth logic
    crate::world::update_plants(&DummyCtx { _private: () });
    let _ = now; // Keep the compiler happy
}

/// Dummy context for non-DB usage.
#[derive(Debug, Clone)]
pub struct DummyCtx {
    _private: (),
}
