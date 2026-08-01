//! Vehicle persistence system.
//!
//! Handles saving and loading vehicle data to/from storage.

use crate::Vehicle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum speed multiplier cap to prevent excessive speed.
pub const MAX_SPEED_MULTIPLIER: f32 = 10.0;

/// Serialized vehicle data for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleSaveData {
    pub vehicle_type: Vehicle,
    pub purchased: bool,
}

/// Vehicle save data for a specific player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerVehicleData {
    pub wallet_address: String,
    pub vehicles: Vec<VehicleSaveData>,
    pub equipped_vehicle_index: Option<usize>,
}

impl PlayerVehicleData {
    /// Create new empty vehicle data for a player
    pub fn new(wallet_address: String) -> Self {
        Self {
            wallet_address,
            vehicles: Vec::new(),
            equipped_vehicle_index: None,
        }
    }

    /// Add a purchased vehicle to the inventory
    pub fn add_vehicle(&mut self, vehicle: VehicleSaveData) {
        // Check if already owned
        if !self.vehicles.iter().any(|v| v.vehicle_type == vehicle.vehicle_type) {
            self.vehicles.push(vehicle);
        }
    }

    /// Equip a vehicle by index
    pub fn equip_vehicle(&mut self, index: usize) -> bool {
        if index >= self.vehicles.len() {
            return false;
        }

        // Unequip all vehicles first
        if let Some(old_idx) = self.equipped_vehicle_index {
            if old_idx < self.vehicles.len() {
                self.vehicles[old_idx].purchased = false;
            }
        }

        // Mark the new vehicle as equipped
        self.vehicles[index].purchased = true;
        self.equipped_vehicle_index = Some(index);
        true
    }

    /// Unequip the currently equipped vehicle
    pub fn unequip_vehicle(&mut self) -> bool {
        if let Some(index) = self.equipped_vehicle_index {
            if index < self.vehicles.len() {
                self.vehicles[index].purchased = false;
                self.equipped_vehicle_index = None;
                return true;
            }
        }
        false
    }

    /// Get the currently equipped vehicle
    pub fn get_equipped_vehicle(&self) -> Option<&VehicleSaveData> {
        if let Some(index) = self.equipped_vehicle_index {
            if index < self.vehicles.len() {
                return Some(&self.vehicles[index]);
            }
        }
        None
    }

    /// Check if player owns a specific vehicle type
    pub fn owns_vehicle(&self, vehicle_type: Vehicle) -> bool {
        self.vehicles.iter().any(|v| v.vehicle_type == vehicle_type)
    }
}

/// In-memory vehicle database for persistence
pub struct VehicleDatabase {
    players: HashMap<String, PlayerVehicleData>,
}

impl VehicleDatabase {
    /// Create a new vehicle database
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }

    /// Get or create player vehicle data
    pub fn get_or_create_player(&mut self, wallet_address: String) -> &mut PlayerVehicleData {
        self.players
            .entry(wallet_address)
            .or_insert_with(|| PlayerVehicleData::new(wallet_address))
    }

    /// Get player vehicle data
    pub fn get_player(&self, wallet_address: &str) -> Option<&PlayerVehicleData> {
        self.players.get(wallet_address)
    }

    /// Save player vehicle data (in a real implementation, this would write to disk/DB)
    pub fn save_player(&mut self, wallet_address: &str) -> Result<(), String> {
        match self.players.get(wallet_address) {
            Some(_) => {
                // In a real implementation, serialize and write to storage
                Ok(())
            }
            None => Err("Player not found".to_string()),
        }
    }

    /// Load player vehicle data (in a real implementation, this would read from disk/DB)
    pub fn load_player(&mut self, wallet_address: &str) -> Result<(), String> {
        // In a real implementation, deserialize and load from storage
        Ok(())
    }

    /// Save all player data
    pub fn save_all(&mut self) -> Result<(), String> {
        for wallet_address in self.players.keys() {
            self.save_player(wallet_address)?;
        }
        Ok(())
    }
}

impl Default for VehicleDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_player_data() {
        let data = PlayerVehicleData::new("0x1234".to_string());
        assert_eq!(data.vehicles.len(), 0);
        assert_eq!(data.equipped_vehicle_index, None);
    }

    #[test]
    fn test_add_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());
        let vehicle = VehicleSaveData {
            vehicle_type: Vehicle::Bicycle,
            purchased: true,
        };

        data.add_vehicle(vehicle);
        assert_eq!(data.vehicles.len(), 1);
        assert_eq!(data.vehicles[0].vehicle_type, Vehicle::Bicycle);
    }

    #[test]
    fn test_add_duplicate_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());
        let vehicle = VehicleSaveData {
            vehicle_type: Vehicle::Bicycle,
            purchased: true,
        };

        data.add_vehicle(vehicle.clone());
        data.add_vehicle(vehicle);
        assert_eq!(data.vehicles.len(), 1, "Should not add duplicate vehicles");
    }

    #[test]
    fn test_equip_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());

        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Bicycle,
            purchased: true,
        });
        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Motorcycle,
            purchased: true,
        });

        assert!(data.equip_vehicle(1));
        assert_eq!(data.equipped_vehicle_index, Some(1));
        assert!(!data.vehicles[0].purchased);
        assert!(data.vehicles[1].purchased);
    }

    #[test]
    fn test_equip_invalid_index() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());
        assert!(!data.equip_vehicle(0));
        assert!(!data.equip_vehicle(99));
    }

    #[test]
    fn test_unequip_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());

        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Bicycle,
            purchased: true,
        });
        data.equipped_vehicle_index = Some(0);

        assert!(data.unequip_vehicle());
        assert_eq!(data.equipped_vehicle_index, None);
        assert!(!data.vehicles[0].purchased);
    }

    #[test]
    fn test_unequip_no_equipped() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());
        assert!(!data.unequip_vehicle());
    }

    #[test]
    fn test_get_equipped_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());

        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Bicycle,
            purchased: true,
        });
        data.equipped_vehicle_index = Some(0);

        let equipped = data.get_equipped_vehicle();
        assert!(equipped.is_some());
        assert_eq!(equipped.unwrap().vehicle_type, Vehicle::Bicycle);
    }

    #[test]
    fn test_get_equipped_vehicle_none() {
        let data = PlayerVehicleData::new("0x1234".to_string());
        assert!(data.get_equipped_vehicle().is_none());
    }

    #[test]
    fn test_owns_vehicle() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());

        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Scooter,
            purchased: true,
        });

        assert!(data.owns_vehicle(Vehicle::Scooter));
        assert!(!data.owns_vehicle(Vehicle::Bicycle));
    }

    #[test]
    fn test_vehicle_database() {
        let mut db = VehicleDatabase::new();

        // Create player data
        let player_data = db.get_or_create_player("0x1234".to_string());
        assert_eq!(player_data.wallet_address, "0x1234");

        // Add vehicle
        player_data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Boat,
            purchased: true,
        });

        // Load player data
        let loaded = db.get_player("0x1234").unwrap();
        assert_eq!(loaded.vehicles.len(), 1);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut data = PlayerVehicleData::new("0x1234".to_string());
        data.add_vehicle(VehicleSaveData {
            vehicle_type: Vehicle::Airplane,
            purchased: true,
        });
        data.equipped_vehicle_index = Some(0);

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: PlayerVehicleData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.wallet_address, "0x1234");
        assert_eq!(deserialized.vehicles.len(), 1);
        assert_eq!(deserialized.vehicles[0].vehicle_type, Vehicle::Airplane);
        assert_eq!(deserialized.equipped_vehicle_index, Some(0));
    }
}
