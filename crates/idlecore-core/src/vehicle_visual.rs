//! Vehicle visual representation system.
//!
//! Provides rendering hooks for vehicle display and visual updates.
//! Bevy-specific rendering is in idlecore-client.

use crate::Vehicle;

/// Visual representation of a vehicle on a player entity.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleVisual {
    pub vehicle_type: Vehicle,
}

impl VehicleVisual {
    /// Create a new vehicle visual component.
    pub fn new(vehicle_type: Vehicle) -> Self {
        Self {
            vehicle_type,
        }
    }

    /// Get the display name for UI.
    pub fn display_name(&self) -> &'static str {
        match self.vehicle_type {
            Vehicle::None => "None",
            Vehicle::Bicycle => "Car",
            Vehicle::Scooter => "Electric Scooter",
            Vehicle::Motorcycle => "Electric Motorcycle",
            Vehicle::Boat => "Electric Boat",
            Vehicle::Airplane => "Electric Airplane",
        }
    }

    /// Get the speed multiplier for this vehicle.
    pub fn speed_multiplier(&self) -> f32 {
        self.vehicle_type.speed_multiplier()
    }

    /// Get the indicator color (for UI highlights) as (r, g, b).
    pub fn indicator_color(&self) -> (f32, f32, f32) {
        match self.vehicle_type {
            Vehicle::None => (0.5, 0.5, 0.5),
            Vehicle::Bicycle => (0.0, 1.0, 0.0),
            Vehicle::Scooter => (1.0, 1.0, 0.0),
            Vehicle::Motorcycle => (1.0, 0.0, 0.0),
            Vehicle::Boat => (0.0, 0.5, 1.0),
            Vehicle::Airplane => (1.0, 0.0, 1.0),
        }
    }
}

/// Check if a vehicle visual is valid (owned and equipped).
pub fn is_valid_visual(vehicle_type: Vehicle) -> bool {
    vehicle_type != Vehicle::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vehicle_visual_display_names() {
        assert_eq!(VehicleVisual::new(Vehicle::None).display_name(), "None");
        assert_eq!(VehicleVisual::new(Vehicle::Bicycle).display_name(), "Car");
        assert_eq!(VehicleVisual::new(Vehicle::Airplane).display_name(), "Electric Airplane");
    }

    #[test]
    fn test_vehicle_visual_speed_multiplier() {
        assert_eq!(VehicleVisual::new(Vehicle::None).speed_multiplier(), 1.0);
        assert_eq!(VehicleVisual::new(Vehicle::Bicycle).speed_multiplier(), 2.0);
        assert_eq!(VehicleVisual::new(Vehicle::Airplane).speed_multiplier(), 10.0);
    }

    #[test]
    fn test_is_valid_visual() {
        assert!(!is_valid_visual(Vehicle::None));
        assert!(is_valid_visual(Vehicle::Bicycle));
        assert!(is_valid_visual(Vehicle::Airplane));
    }
}
