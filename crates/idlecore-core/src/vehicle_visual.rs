//! Vehicle visual representation system.
//!
//! Provides rendering hooks for vehicle display and visual updates.

use crate::Vehicle;
use bevy::prelude::*;

/// Visual representation of a vehicle on a player entity
#[derive(Component)]
pub struct VehicleVisual {
    pub vehicle_type: Vehicle,
}

impl VehicleVisual {
    /// Create a new vehicle visual component
    pub fn new(vehicle_type: Vehicle) -> Self {
        Self {
            vehicle_type,
        }
    }

    /// Get the display name for UI
    pub fn display_name(&self) -> &'static str {
        match self.vehicle_type {
            Vehicle::None => "None",
            Vehicle::Bicycle => "Electric Bicycle",
            Vehicle::Scooter => "Electric Scooter",
            Vehicle::Motorcycle => "Electric Motorcycle",
            Vehicle::Boat => "Electric Boat",
            Vehicle::Airplane => "Electric Airplane",
        }
    }

    /// Get the speed multiplier for this vehicle
    pub fn speed_multiplier(&self) -> f32 {
        self.vehicle_type.speed_multiplier()
    }

    /// Get the indicator color (for UI highlights)
    pub fn indicator_color(&self) -> Color {
        Color::srgb(
            match self.vehicle_type {
                Vehicle::None => 0.5,
                Vehicle::Bicycle => 0.0,
                Vehicle::Scooter => 1.0,
                Vehicle::Motorcycle => 1.0,
                Vehicle::Boat => 0.0,
                Vehicle::Airplane => 1.0,
            },
            match self.vehicle_type {
                Vehicle::None => 0.5,
                Vehicle::Bicycle => 1.0,
                Vehicle::Scooter => 1.0,
                Vehicle::Motorcycle => 0.0,
                Vehicle::Boat => 0.5,
                Vehicle::Airplane => 0.0,
            },
            match self.vehicle_type {
                Vehicle::None => 0.5,
                Vehicle::Bicycle => 0.0,
                Vehicle::Scooter => 0.0,
                Vehicle::Motorcycle => 0.0,
                Vehicle::Boat => 1.0,
                Vehicle::Airplane => 1.0,
            },
        )
    }
}

/// System to update vehicle visual when player's vehicle changes
pub fn update_vehicle_visual(
    mut query: Query<(&VehicleVisual, &Transform)>,
) {
    for (vehicle_visual, transform) in query.iter_mut() {
        log::info!(
            "Updated vehicle visual: {} at position ({}, {})",
            vehicle_visual.display_name(),
            transform.translation.x,
            transform.translation.y
        );
    }
}

/// System to spawn vehicle visual when a player equips a vehicle
pub fn spawn_vehicle_visual(
    mut commands: Commands,
    query: Query<(Entity, &Vehicle), Without<VehicleVisual>>,
) {
    for (entity, vehicle) in query.iter() {
        let visual = VehicleVisual::new(*vehicle);
        commands.entity(entity).insert(visual);
    }
}
