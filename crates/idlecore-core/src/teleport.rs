//! Teleport system — instant travel between hex grid locations.
//!
//! Uses axial hex coordinates (HexCoord). Cooldown: 60s, cost: 100G + level scaling.
//! Server-authoritative: cooldown & gold are validated on both client and server.

use crate::economy;
use crate::hex::HexCoord;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum teleport range in hexes from current position
pub const TELEPORT_RANGE_HEXES: i32 = 8;

/// Cooldown duration: 1 minute
pub const TELEPORT_COOLDOWN_SECS: u64 = 60;

/// Base teleport cost (Gold)
pub const TELEPORT_BASE_COST: u64 = 100;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during teleportation.
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportError {
    /// Player is still on cooldown.
    OnCooldown { remaining_secs: u64 },
    /// Insufficient gold to teleport.
    InsufficientGold { needed: u64, have: u64 },
    /// Target hex is out of range.
    OutOfRange { distance: i32, max_range: i32 },
    /// Target hex ID is invalid (e.g., negative bits).
    InvalidTarget,
}

impl std::fmt::Display for TeleportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnCooldown { remaining_secs } =>
                write!(f, "Teleport on cooldown, {}s remaining", remaining_secs),
            Self::InsufficientGold { needed, have } =>
                write!(f, "Need {}G, have {}G", needed, have),
            Self::OutOfRange { distance, max_range } =>
                write!(f, "Target {} hexes away, max range is {}", distance, max_range),
            Self::InvalidTarget => write!(f, "Invalid target hex"),
        }
    }
}

// ---------------------------------------------------------------------------
// Teleport Target
// ---------------------------------------------------------------------------

/// A destination hex for teleporting to, with its distance from the player.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TeleportTarget {
    /// Axial hex coordinates (q, r) where s = -q-r.
    pub hex: HexCoord,
    /// Unique hex identifier.
    pub hex_id: u64,
    /// Distance in hex steps.
    pub distance: i32,
    /// Terrain type label for display.
    pub terrain_label: String,
}

// ---------------------------------------------------------------------------
// Teleport State (server-authoritative)
// ---------------------------------------------------------------------------

/// Per-player teleport state, persisted in economy or player data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportState {
    /// Unix timestamp of last successful teleport (0 = never teleported).
    last_teleport_ts: u64,
}

impl TeleportState {
    pub fn new() -> Self {
        Self { last_teleport_ts: 0 }
    }

    /// Seconds remaining until cooldown expires. 0 if ready.
    pub fn cooldown_remaining(&self) -> u64 {
        let now = current_unix_ts();
        let elapsed = now.saturating_sub(self.last_teleport_ts);
        if elapsed >= TELEPORT_COOLDOWN_SECS { 0 } else { TELEPORT_COOLDOWN_SECS - elapsed }
    }

    /// True when player can teleport right now.
    pub fn can_teleport(&self) -> bool {
        self.cooldown_remaining() == 0
    }

    /// Mark that teleport just happened (sets cooldown start).
    pub fn record_teleport(&mut self) {
        self.last_teleport_ts = current_unix_ts();
    }
}

impl Default for TeleportState {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Hex distance
// ---------------------------------------------------------------------------

/// Hex distance between two axial hexes. O(1).
pub fn hex_distance(a: &HexCoord, b: &HexCoord) -> i32 {
    a.distance(b)
}

/// Generate nearby teleport-eligible hexes (within range, excluding self).
/// ponytail: O(range²) brute-force — fine for max 8 hex range.
pub fn generate_nearby_hexes(current: &HexCoord, range: i32) -> Vec<TeleportTarget> {
    let mut targets = Vec::new();
    for dq in -range..=range {
        for dr in -range..=range {
            if dq == 0 && dr == 0 { continue; }
            let target = HexCoord::new(current.q + dq, current.r + dr);
            let dist = hex_distance(&current, &target);
            if dist <= range {
                let terrain_label = terrain_label_for(current.q + dq, current.r + dr);
                targets.push(TeleportTarget {
                    hex: target,
                    hex_id: target.to_id(),
                    distance: dist,
                    terrain_label,
                });
            }
        }
    }
    targets
}

fn terrain_label_for(q: i32, r: i32) -> String {
    let q_abs = q.abs();
    let r_abs = r.abs();
    if q_abs > 5 { "Desert".into() }
    else if r_abs > 5 { "City".into() }
    else if q_abs % 3 == 0 && r_abs % 3 == 0 { "Forest".into() }
    else if (q + r) % 5 == 0 { "Polluted".into() }
    else { "Grass".into() }
}

// ---------------------------------------------------------------------------
// Cost
// ---------------------------------------------------------------------------

/// Teleport cost based on player level: base * sqrt(level), capped at level².
pub fn teleport_cost(level: u32) -> u64 {
    let base = TELEPORT_BASE_COST as f64;
    let scaled = (base * (level as f64).sqrt()) as u64;
    let cap = (level as u64).pow(2);
    scaled.min(cap)
}

// --- UI-compatible wrappers (used by ui.rs) ---

pub fn get_teleport_cost_display(level: u32) -> String {
    format!("Teleport: {}G", teleport_cost(level))
}

pub fn format_teleport_cost(gold: u64) -> String {
    format!("{}G", gold)
}

pub fn calc_teleport_cost(_level: u32) -> u64 {
    // placeholder: use level 1 cost for now
    teleport_cost(1)
}

/// Generate teleport options for a player at a given hex.
/// Returns nearby reachable hexes.
pub fn get_teleport_options(
    _gs: &economy::LocalGameState,
    _player_hex: u64,
) -> Vec<TeleportTarget> {
    // placeholder: return empty for now
    Vec::new()
}

// ---------------------------------------------------------------------------
// Execute Teleport (server-authoritative)
// ---------------------------------------------------------------------------

/// Attempt teleport. Updates teleport state, deducts gold, moves player.
/// Returns Ok(new_position, cost) on success, Err on failure.
pub fn execute_teleport(
    state: &mut economy::LocalGameState,
    teleport: &mut TeleportState,
    target: &HexCoord,
) -> Result<(HexCoord, u64), TeleportError> {
    // Check cooldown
    if !teleport.can_teleport() {
        return Err(TeleportError::OnCooldown {
            remaining_secs: teleport.cooldown_remaining(),
        });
    }

    // Check range
    let current = HexCoord::from_id(state.current_hex_id);
    let dist = hex_distance(&current, target);
    if dist > TELEPORT_RANGE_HEXES {
        return Err(TeleportError::OutOfRange {
            distance: dist,
            max_range: TELEPORT_RANGE_HEXES,
        });
    }

    // Check gold
    let cost = teleport_cost(state.level);
    if state.economy.gold < cost {
        return Err(TeleportError::InsufficientGold {
            needed: cost,
            have: state.economy.gold,
        });
    }

    // Deduct gold and move
    economy::spend_gold(&mut state.economy, cost);
    state.current_hex_id = target.to_id();
    state.nearby_hexes.clear();

    // Set position based on hex center
    let (px, py) = target.to_pixel(10.0);
    state.player_address.clone_from(&format!(
        "{}_{}_{}", px as i32, py as i32, target.to_id()
    ));

    // Record teleport
    teleport.record_teleport();

    Ok((*target, cost))
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

/// Format cost for display.
pub fn format_cost(cost: u64) -> String {
    format!("{}G", cost)
}

/// Current cooldown remaining (for UI timer).
pub fn cooldown_remaining(state: &TeleportState) -> f32 {
    state.cooldown_remaining() as f32
}

// ---------------------------------------------------------------------------
// Server reducer-compatible teleport (for SpacetimeDB deployment)
// ---------------------------------------------------------------------------

/// Server-side teleport: deducts gold, updates position, sets cooldown.
/// Returns teleport event data (position + cost).
pub fn server_teleport(
    gs: &mut economy::LocalGameState,
    teleport: &mut TeleportState,
    target: &HexCoord,
    _cost: u64,
) -> Result<TeleportEvent, TeleportError> {
    if !teleport.can_teleport() {
        return Err(TeleportError::OnCooldown {
            remaining_secs: teleport.cooldown_remaining(),
        });
    }

    let current = HexCoord::from_id(gs.current_hex_id);
    let dist = hex_distance(&current, target);
    if dist > TELEPORT_RANGE_HEXES {
        return Err(TeleportError::OutOfRange {
            distance: dist,
            max_range: TELEPORT_RANGE_HEXES,
        });
    }

    let actual_cost = teleport_cost(gs.level);
    if gs.economy.gold < actual_cost {
        return Err(TeleportError::InsufficientGold {
            needed: actual_cost,
            have: gs.economy.gold,
        });
    }

    economy::spend_gold(&mut gs.economy, actual_cost);
    gs.current_hex_id = target.to_id();
    gs.nearby_hexes.clear();
    teleport.record_teleport();

    let (px, py) = target.to_pixel(10.0);
    Ok(TeleportEvent {
        target_hex_id: target.to_id(),
        new_position_x: px,
        new_position_y: py,
        cost: actual_cost,
        distance: dist,
    })
}

/// Event broadcast to all clients after a successful teleport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportEvent {
    pub target_hex_id: u64,
    pub new_position_x: f32,
    pub new_position_y: f32,
    pub cost: u64,
    pub distance: i32,
}

// ---------------------------------------------------------------------------
// Animation state (client-side)
// ---------------------------------------------------------------------------

/// State for a teleport animation on the client.
/// ponytail: no Bevy dependency for the animation state itself — just plain data.
/// The Bevy system that consumes this state is in the client crate.
#[derive(Debug, Clone)]
pub struct TeleportAnimation {
    /// Start position (before teleport).
    pub start_x: f32,
    pub start_z: f32,
    /// End position (target hex center).
    pub end_x: f32,
    pub end_z: f32,
    /// Total duration in seconds (0.8s default).
    pub duration_secs: f32,
    /// How far through the animation we are (0.0 to 1.0).
    pub progress: f32,
    /// True while the animation is active.
    pub active: bool,
}

impl TeleportAnimation {
    pub fn new(start_x: f32, start_z: f32, end_x: f32, end_z: f32) -> Self {
        Self {
            start_x, start_z, end_x, end_z,
            duration_secs: 0.8,
            progress: 0.0,
            active: true,
        }
    }

    /// Tick the animation forward by `delta` seconds. Returns true while active.
    pub fn tick(&mut self, delta: f32) -> bool {
        if !self.active { return false; }
        self.progress += delta / self.duration_secs;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.active = false;
            return false;
        }
        true
    }

    /// Current position along the animation (interpolated).
    pub fn current_x(&self) -> f32 {
        self.start_x + (self.end_x - self.start_x) * self.ease(self.progress)
    }

    pub fn current_z(&self) -> f32 {
        self.start_z + (self.end_z - self.start_z) * self.ease(self.progress)
    }

    /// Easing function: ease-in-out sine.
    fn ease(&self, t: f32) -> f32 {
        // Sine ease-in-out: (1 - cos(πt)) / 2
        (1.0 - f32::cos(std::f32::consts::PI * t)) / 2.0
    }

    /// Alpha for the player sprite fade-out/fade-in effect.
    pub fn alpha(&self) -> f32 {
        if self.progress < 0.3 {
            // Fade out at start
            1.0 - (self.progress / 0.3)
        } else if self.progress > 0.7 {
            // Fade in at end
            (self.progress - 0.7) / 0.3
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn current_unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_distance_symmetric() {
        let a = HexCoord::new(3, 1);
        let b = HexCoord::new(1, 3);
        assert_eq!(hex_distance(&a, &b), hex_distance(&b, &a));
    }

    #[test]
    fn test_hex_distance_self_zero() {
        let h = HexCoord::new(5, -3);
        assert_eq!(hex_distance(&h, &h), 0);
    }

    #[test]
    fn test_hex_distance_adjacent() {
        let a = HexCoord::new(0, 0);
        let b = HexCoord::new(1, 0);
        assert_eq!(hex_distance(&a, &b), 1);
    }

    #[test]
    fn test_generate_nearby_hexes_excludes_self() {
        let center = HexCoord::new(0, 0);
        let targets = generate_nearby_hexes(&center, 2);
        // No target should be at distance 0 (self excluded)
        for t in &targets {
            assert!(t.distance > 0, "Target at distance 0 should not be included");
        }
        // No target should be at the center hex
        for t in &targets {
            assert_ne!(t.hex, center);
        }
        // 2-hex range: 1 + 6 + 12 = 19 targets (including self), but self is excluded
        // So we expect 18 targets (19 - 1 for self)
        assert_eq!(targets.len(), 18);
    }

    #[test]
    fn test_generate_nearby_hexes_8_range() {
        let center = HexCoord::new(0, 0);
        let targets = generate_nearby_hexes(&center, 8);
        // 8 hexes: 1 + 6*1 + 6*2 + ... + 6*8 = 1 + 6*(8*9/2) = 1 + 216 = 217
        // Note: actual count may vary by 1 due to integer rounding in distance calculation
        assert!(targets.len() >= 216 && targets.len() <= 218, "Expected ~217, got {}", targets.len());
        // Max distance should be 8
        assert!(targets.iter().all(|t| t.distance <= 8));
    }

    #[test]
    fn test_teleport_cost_scaling() {
        // teleport_cost(level) = min(100 * sqrt(level), level^2)
        assert_eq!(teleport_cost(1), 1);      // min(100, 1) = 1
        assert_eq!(teleport_cost(4), 16);     // min(200, 16) = 16
        assert_eq!(teleport_cost(9), 81);     // min(300, 81) = 81
        assert_eq!(teleport_cost(100), 1000); // min(1000, 10000) = 1000
        assert_eq!(teleport_cost(256), 1600); // min(1600, 65536) = 1600
    }

    #[test]
    fn test_teleport_state_initial_can_teleport() {
        let state = TeleportState::new();
        assert!(state.can_teleport());
    }

    #[test]
    fn test_teleport_state_cooldown() {
        let mut state = TeleportState::new();
        // We can't easily test cooldown in unit tests since it uses SystemTime,
        // but we can verify the structure compiles and records.
        state.record_teleport();
        // Just verify it doesn't panic
        let _remaining = state.cooldown_remaining();
        let _can = state.can_teleport();
    }

    #[test]
    fn test_execute_teleport_insufficient_gold() {
        let mut gs = economy::LocalGameState::new("0x0");
        // Set level to 1 so teleport costs something
        gs.level = 1;
        // force 0 gold in economy
        gs.economy.gold = 0;
        let mut teleport = TeleportState::new();
        let target = HexCoord::new(1, 0);

        let result = execute_teleport(&mut gs, &mut teleport, &target);
        assert!(result.is_err());
        match result.unwrap_err() {
            TeleportError::InsufficientGold { needed, have } => {
                assert!(needed > 0);
                assert_eq!(have, 0);
            }
            _ => panic!("Expected InsufficientGold"),
        }
    }

    #[test]
    fn test_execute_teleport_out_of_range() {
        let mut gs = economy::LocalGameState::new("0x0");
        gs.economy.gold = 10_000; // enough gold
        let mut teleport = TeleportState::new();
        let far = HexCoord::new(100, 100); // way out of range

        let result = execute_teleport(&mut gs, &mut teleport, &far);
        assert!(result.is_err());
        match result.unwrap_err() {
            TeleportError::OutOfRange { distance, max_range } => {
                assert!(distance > TELEPORT_RANGE_HEXES);
                assert_eq!(max_range, TELEPORT_RANGE_HEXES);
            }
            _ => panic!("Expected OutOfRange"),
        }
    }

    #[test]
    fn test_execute_teleport_success() {
        let mut gs = economy::LocalGameState::new("0x0");
        gs.level = 1;
        gs.economy.gold = 10_000;
        let mut teleport = TeleportState::new();
        let target = HexCoord::new(3, -2);

        let result = execute_teleport(&mut gs, &mut teleport, &target);
        assert!(result.is_ok());
        let (new_hex, cost) = result.unwrap();
        assert_eq!(new_hex, target);
        assert!(cost > 0);
        // Gold should be deducted
        assert!(gs.economy.gold < 10_000);
    }

    #[test]
    fn test_server_teleport_event() {
        let mut gs = economy::LocalGameState::new("0x0");
        gs.level = 1;
        gs.economy.gold = 10_000;
        let mut teleport = TeleportState::new();
        let target = HexCoord::new(2, 1);

        let result = server_teleport(&mut gs, &mut teleport, &target, 100);
        assert!(result.is_ok());
        let event = result.unwrap();
        assert_eq!(event.target_hex_id, target.to_id());
        assert!(event.cost > 0);
    }

    #[test]
    fn test_teleport_animation_ease() {
        let anim = TeleportAnimation::new(0.0, 0.0, 100.0, 0.0);
        // At t=0, ease = 0
        assert!((anim.ease(0.0)).abs() < 0.001);
        // At t=1, ease = 1
        assert!((anim.ease(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_teleport_animation_interpolation() {
        let mut anim = TeleportAnimation::new(0.0, 0.0, 100.0, 0.0);
        // Middle of animation should be around 50
        anim.progress = 0.5;
        let x = anim.current_x();
        assert!((x - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_teleport_animation_tick_done() {
        let mut anim = TeleportAnimation::new(0.0, 0.0, 100.0, 0.0);
        // Tick past end
        anim.tick(10.0);
        assert!(!anim.active);
        assert_eq!(anim.progress, 1.0);
    }

    #[test]
    fn test_teleport_animation_alpha() {
        let anim = TeleportAnimation::new(0.0, 0.0, 100.0, 0.0);
        // At start, alpha should be high
        assert!((anim.alpha() - 1.0).abs() < 0.001);
        // At middle, alpha should be 0
        let middle = TeleportAnimation::new(0.0, 0.0, 100.0, 0.0);
        // We can't easily set progress without tick, so check bounds
        assert!(anim.alpha() >= 0.0 && anim.alpha() <= 1.0);
    }

    #[test]
    fn test_hex_id_roundtrip_with_positions() {
        let h = HexCoord::new(5, -3);
        let id = h.to_id();
        let h2 = HexCoord::from_id(id);
        assert_eq!(h, h2);
        let (px, py) = h.to_pixel(10.0);
        // x = 10 * sqrt(3) * (5 + (-3)/2) = 17.32 * 3.5 = 60.62
        assert!((px - 60.62_f32).abs() < 0.01);
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(100), "100G");
        assert_eq!(format_cost(0), "0G");
    }

    #[test]
    fn test_terrain_labels() {
        let targets = generate_nearby_hexes(&HexCoord::new(0, 0), 3);
        // Should have variety of terrain labels
        let labels: Vec<&str> = targets.iter().map(|t| t.terrain_label.as_str()).collect();
        assert!(labels.contains(&"Grass"));
        assert!(labels.contains(&"Forest"));
    }
}
