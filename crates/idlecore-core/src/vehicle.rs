//! Vehicle system — purchase, equip, and use vehicles.
//!
//! VehicleType enum delegates to crate::Vehicle for display_name(), speed_multiplier(),
//! and purchase_cost() to avoid duplication.

use crate::Vehicle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleType {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

impl VehicleType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => Vehicle::None.display_name(),
            Self::Bicycle => Vehicle::Bicycle.display_name(),
            Self::Scooter => Vehicle::Scooter.display_name(),
            Self::Motorcycle => Vehicle::Motorcycle.display_name(),
            Self::Boat => Vehicle::Boat.display_name(),
            Self::Airplane => Vehicle::Airplane.display_name(),
        }
    }

    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Self::None => Vehicle::None.speed_multiplier(),
            Self::Bicycle => Vehicle::Bicycle.speed_multiplier(),
            Self::Scooter => Vehicle::Scooter.speed_multiplier(),
            Self::Motorcycle => Vehicle::Motorcycle.speed_multiplier(),
            Self::Boat => Vehicle::Boat.speed_multiplier(),
            Self::Airplane => Vehicle::Airplane.speed_multiplier(),
        }
    }

    pub fn purchase_cost(&self) -> u64 {
        match self {
            Self::None => Vehicle::None.purchase_cost(),
            Self::Bicycle => Vehicle::Bicycle.purchase_cost(),
            Self::Scooter => Vehicle::Scooter.purchase_cost(),
            Self::Motorcycle => Vehicle::Motorcycle.purchase_cost(),
            Self::Boat => Vehicle::Boat.purchase_cost(),
            Self::Airplane => Vehicle::Airplane.purchase_cost(),
        }
    }
}

impl From<VehicleType> for Vehicle {
    fn from(vt: VehicleType) -> Self {
        match vt {
            VehicleType::None => Vehicle::None,
            VehicleType::Bicycle => Vehicle::Bicycle,
            VehicleType::Scooter => Vehicle::Scooter,
            VehicleType::Motorcycle => Vehicle::Motorcycle,
            VehicleType::Boat => Vehicle::Boat,
            VehicleType::Airplane => Vehicle::Airplane,
        }
    }
}

impl From<Vehicle> for VehicleType {
    fn from(v: Vehicle) -> Self {
        match v {
            Vehicle::None => VehicleType::None,
            Vehicle::Bicycle => VehicleType::Bicycle,
            Vehicle::Scooter => VehicleType::Scooter,
            Vehicle::Motorcycle => VehicleType::Motorcycle,
            Vehicle::Boat => VehicleType::Boat,
            Vehicle::Airplane => VehicleType::Airplane,
        }
    }
}

pub fn available_vehicles() -> Vec<Vehicle> {
    Vehicle::all_vehicles().to_vec()
}
