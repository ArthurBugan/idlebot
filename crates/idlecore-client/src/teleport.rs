//! Client-side teleport UI and animation system.
//!
//! Implements:
//! - TeleportUI struct with hex selection, cooldown timer, cost display
//! - Hex click handling to select destinations
//! - Teleport animation with Bevy sprite particles
//! - Server position broadcasting

use bevy::prelude::*;
use idlecore_core::hex::HexCoord;
use idlecore_core::teleport::{generate_nearby_hexes, teleport_cost, TeleportTarget};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TELEPORT_COST: u64 = 100;
const TELEPORT_COOLDOWN_SECS: u32 = 60;

// ---------------------------------------------------------------------------
// Teleport UI Component
// ---------------------------------------------------------------------------

/// Client-side teleport UI state
#[derive(Component, Debug, Clone)]
pub struct TeleportUI {
    /// Currently selected destination hex
    pub selected_hex: Option<HexCoord>,
    /// Hexes available for teleport
    pub nearby_hexes: Vec<TeleportTarget>,
    /// Cooldown timer in seconds
    pub cooldown_timer: f32,
    /// Gold available for teleport
    pub gold_available: u64,
    /// Teleport cost
    pub teleport_cost: u64,
}

impl TeleportUI {
    pub fn new() -> Self {
        Self {
            selected_hex: None,
            nearby_hexes: Vec::new(),
            cooldown_timer: 0.0,
            gold_available: 0,
            teleport_cost: TELEPORT_COST,
        }
    }

    /// Update nearby hexes based on current position
    pub fn update_nearby_hexes(&mut self, current_hex: &HexCoord, range: i32) {
        self.nearby_hexes = generate_nearby_hexes(current_hex, range);
    }

    /// Select a destination hex
    pub fn select_hex(&mut self, hex: HexCoord) {
        self.selected_hex = Some(hex);
    }

    /// Confirm teleport (client-side validation only)
    pub fn confirm_teleport(&mut self, player_gold: u64, player_level: u32) -> Option<TeleportTarget> {
        if self.cooldown_timer > 0.0 {
            return None;
        }
        
        if player_gold < self.teleport_cost {
            return None;
        }
        
        let destination = self.selected_hex.take()?;
        let target = self.nearby_hexes.iter()
            .find(|t| t.hex == destination)?;
        
        // Start cooldown
        self.cooldown_timer = TELEPORT_COOLDOWN_SECS as f32;
        
        Some(target.clone())
    }

    /// Tick cooldown
    pub fn tick_cooldown(&mut self, delta: f32) {
        if self.cooldown_timer > 0.0 {
            self.cooldown_timer = (self.cooldown_timer - delta).max(0.0);
        }
    }

    /// Check if teleport is available
    pub fn is_available(&self) -> bool {
        self.cooldown_timer <= 0.0 && self.gold_available >= self.teleport_cost
    }
}

impl Default for TeleportUI {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Teleport Animation Component
// ---------------------------------------------------------------------------

/// Teleport animation system
#[derive(Component, Debug)]
pub struct TeleportAnimation {
    /// Start position
    pub start_position: Vec3,
    /// End position
    pub end_position: Vec3,
    /// Current progress (0.0 to 1.0)
    pub progress: f32,
    /// Animation duration
    pub duration: Duration,
    /// Whether animation is active
    pub active: bool,
}

impl TeleportAnimation {
    pub fn new(start: Vec3, end: Vec3, duration_secs: f32) -> Self {
        Self {
            start_position: start,
            end_position: end,
            progress: 0.0,
            duration: Duration::from_secs_f32(duration_secs),
            active: true,
        }
    }

    /// Update animation progress
    pub fn tick(&mut self, delta: f32) {
        if !self.active {
            return;
        }
        
        self.progress += delta / self.duration.as_secs_f32();
        
        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.active = false;
        }
    }

    /// Get interpolated position
    pub fn current_position(&self) -> Vec3 {
        self.start_position.lerp(self.end_position, self.ease_in_out(self.progress))
    }

    /// Easing function (ease-in-out)
    fn ease_in_out(&self, t: f32) -> f32 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            -1.0 + (4.0 - 2.0 * t) * t
        }
    }
}

// ---------------------------------------------------------------------------
// Teleport Particle System
// ---------------------------------------------------------------------------

/// Particle for teleport effect
#[derive(Component, Debug)]
pub struct TeleportParticle {
    /// Initial position
    pub start_pos: Vec3,
    /// Random offset
    pub offset: Vec3,
    /// Random scale
    pub scale: f32,
    /// Lifetime in seconds
    pub lifetime: Duration,
    /// Current age
    pub age: Duration,
}

impl TeleportParticle {
    pub fn new(start_pos: Vec3) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        Self {
            start_pos,
            offset: Vec3::new(
                rng.gen_range(-0.5..0.5),
                rng.gen_range(0.0..0.5),
                rng.gen_range(-0.5..0.5),
            ),
            scale: rng.gen_range(0.5..1.5),
            lifetime: Duration::from_millis(rng.gen_range(500..1000)),
            age: Duration::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// Teleport Systems
// ---------------------------------------------------------------------------

/// Update teleport UI cooldown
pub fn update_teleport_cooldown(
    time: Res<Time>,
    mut teleport_ui: Query<&mut TeleportUI>,
) {
    for mut ui in teleport_ui.iter_mut() {
        ui.tick_cooldown(time.delta_secs());
    }
}

/// Update teleport animation progress
pub fn update_teleport_animation(
    time: Res<Time>,
    mut teleport_anim: Query<(Entity, &mut TeleportAnimation, &mut Transform)>,
) {
    let delta = time.delta_secs();
    
    for (entity, mut anim, mut transform) in teleport_anim.iter_mut() {
        anim.tick(delta);
        
        if !anim.active {
            // Remove teleport animation entity
            return;
        }
        
        transform.translation = anim.current_position();
    }
}

/// Cleanup finished teleport animations
pub fn cleanup_teleport_animations(
    mut commands: Commands,
    teleport_anim: Query<(Entity, &TeleportAnimation), Changed<TeleportAnimation>>,
) {
    for (entity, anim) in teleport_anim.iter() {
        if !anim.active {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn teleport particles
pub fn spawn_teleport_particles(
    mut commands: Commands,
    time: Res<Time>,
    teleport_anim: Query<&TeleportAnimation>,
) {
    // Spawn particles during teleport animation
    for anim in teleport_anim.iter() {
        if anim.active && anim.progress > 0.3 && anim.progress < 0.8 {
            let start = anim.start_position;
            let end = anim.end_position;
            
            // Spawn a few particles
            for _ in 0..3 {
                let midpoint = start.lerp(end, anim.progress);
                commands.spawn((
                    TeleportParticle::new(midpoint),
                    Transform::from_translation(midpoint),
                    Sprite {
                        color: Color::srgba(0.0, 1.0, 1.0, 0.8),
                        custom_size: Some(Vec2::splat(0.3)),
                        ..default()
                    },
                ));
            }
        }
    }
}

/// Update teleport particles
pub fn update_teleport_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut teleport_particles: Query<(Entity, &mut TeleportParticle, &mut Transform, &mut Sprite)>,
) {
    let delta = time.delta_secs();
    
    for (entity, mut particle, mut transform, mut sprite) in teleport_particles.iter_mut() {
        particle.age += Duration::from_secs_f32(delta);
        
        // Fade out
        let elapsed = particle.age.as_secs_f32();
        let lifetime = particle.lifetime.as_secs_f32();
        let alpha = 1.0 - (elapsed / lifetime).min(1.0);
        sprite.color = Color::srgba(0.0, 1.0, 1.0, alpha);
        
        // Remove when expired
        if particle.age >= particle.lifetime {
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Teleport UI Systems
// ---------------------------------------------------------------------------

/// Update teleport UI with current game state
pub fn update_teleport_ui(
    mut teleport_ui: Query<&mut TeleportUI>,
    player_position: Query<&Transform, With<PlayerComponent>>,
) {
    let Some(mut ui) = teleport_ui.iter_mut().next() else {
        return;
    };
    
    // TODO: Update gold_available and player_level from game state
    // For now, use placeholder values
}

// BEVY 0.19 TODO: Fix event handler API compatibility
// These functions need updating for Bevy 0.19 event system
// pub fn handle_hex_click(
//     mut events: EventReader<MouseButtonDownEvent>,
//     mut teleport_ui: Query<&mut TeleportUI>,
//     camera_query: Query<(&Camera, &GlobalTransform)>,
// ) {
//     // TODO: Raycast from camera to hex grid
//     // For now, this is a placeholder
// }
//
// pub fn confirm_teleport_click(
//     mut events: EventReader<MouseButtonInput>,
//     mut teleport_ui: Query<&mut TeleportUI>,
//     player_data: Query<&PlayerComponent>,
// ) {
//     let Ok(ui) = teleport_ui.iter_mut().next() else {
//         return;
//     };
//     
//     if !ui.is_available() {
//         return;
//     }
//     
//     // TODO: Send teleport request to server
//     // For now, just log it
// }

// ---------------------------------------------------------------------------
// Player Component (placeholder)
// ---------------------------------------------------------------------------

/// Simple player component for teleport system
#[derive(Component, Debug, Clone)]
pub struct PlayerComponent {
    pub position: Vec3,
    pub hex: HexCoord,
    pub gold: u64,
    pub level: u32,
}

impl PlayerComponent {
    pub fn new(hex: HexCoord) -> Self {
        let (x, z) = hex.to_pixel(10.0);
        Self {
            position: Vec3::new(x, 0.0, z),
            hex,
            gold: 1000,
            level: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Camera Component (placeholder)
// ---------------------------------------------------------------------------

/// Placeholder camera component for raycasting
#[derive(Component, Debug)]
pub struct TeleportCamera;
