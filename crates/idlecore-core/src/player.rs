//! Player component and spawning
//!
//! Client-side player: position, velocity, current hex, gold, XP, level,
//! eco points, owned vehicle, equipped cosmetics, last login time.
//! Avatar: orange tetrahedron (placeholder for Tamagotchi).

use bevy::prelude::*;
use crate::terrain::TerrainType;
use crate::economy::PlayerEconomy;

/// Marker component at world center (0, 0, 0)
#[derive(Component)]
pub struct PlayerSpawnMarker;

/// Client-side player component (attached to the player entity)
#[derive(Component)]
pub struct Player {
    /// World position (x, y, z) in Bevy space
    pub position: Vec3,
    /// Current velocity (x, z) — updated by input system each frame
    pub velocity: Vec2,
    /// Current hex coordinates (q, r) — axial
    pub current_hex: Option<(i32, i32)>,
    /// Player's economy state
    pub economy: PlayerEconomy,
    /// Whether this is a local client player (not synced from server)
    pub is_local: bool,
}

impl Player {
    /// Create a default player at world center
    pub fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: None,
            economy: PlayerEconomy::default(),
            is_local: true,
        }
    }

    /// Create a new player with initial values
    pub fn new(gold: u64, xp: u64, eco_points: u64) -> Self {
        let mut econ = PlayerEconomy::default();
        econ.gold = gold;
        econ.xp = xp;
        econ.eco_points = eco_points;
        econ.eco_points = eco_points;

        Self {
            position: Vec3::ZERO,
            velocity: Vec2::ZERO,
            current_hex: Some((0, 0)),
            economy: econ,
            is_local: true,
        }
    }

    /// Get the player's level
    pub fn level(&self) -> u32 {
        self.economy.level
    }

    /// Get the player's XP
    pub fn xp(&self) -> u64 {
        self.economy.xp
    }

    /// Get the player's gold
    pub fn gold(&self) -> u64 {
        self.economy.gold
    }

    /// Get the player's eco points
    pub fn eco_points(&self) -> u64 {
        self.economy.eco_points
    }

    /// Get the vehicle the player owns
    pub fn owned_vehicle(&self) -> Option<&Vehicle> {
        if self.economy.vehicle.is_empty() {
            None
        } else {
            Some(&self.economy.vehicle)
        }
    }

    /// Set the vehicle the player owns
    pub fn set_vehicle(&mut self, vehicle: Vehicle) {
        self.economy.vehicle = vehicle.name.to_string();
    }

    /// Get the hex ID from current hex coordinates
    pub fn current_hex_id(&self) -> u64 {
        if let Some((q, r)) = self.current_hex {
            ((q as u64) << 32) | (r as u64)
        } else {
            0
        }
    }

    /// Convert world position to hex coordinates (q, r)
    pub fn world_to_hex(w: f32, z: f32, hex_radius: f32) -> (i32, i32) {
        let sq3 = std::f32::consts::SQRT_3;
        let r_approx = (z / (1.5 * hex_radius)) as i32;
        let q_approx = ((w / (sq3 * hex_radius)) - (r_approx as f32) / 2.0) as i32;

        let fq = q_approx as f64;
        let fr = r_approx as f64;
        let fs = -(fq + fr);

        // Round to nearest of the three directions
        let dq = (fq - fr).abs();
        let dr = (fq - fs).abs();
        let ds = (fr - fs).abs();

        if dq > dr && dq > ds {
            // On q edge: move along q-r direction
            let row = if fs >= 0.0 {
                r_approx
            } else {
                -r_approx
            };
            (row, r_approx)
        } else if dr > ds {
            (q_approx, r_approx)
        } else {
            // On s edge
            let offset = if fs >= 0.0 { 0 } else { -2 * r_approx };
            (q_approx, r_approx + offset / 2)
        }
    }
}

/// Vehicle types with speed multipliers and purchase costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vehicle {
    None,
    Bicycle,
    Scooter,
    Motorcycle,
    Boat,
    Airplane,
}

impl Vehicle {
    /// Speed multiplier: how much it multiplies the base movement speed.
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Vehicle::None => 1.0,
            Vehicle::Bicycle => 2.0,
            Vehicle::Scooter => 3.0,
            Vehicle::Motorcycle => 5.0,
            Vehicle::Boat => 4.0,
            Vehicle::Airplane => 10.0,
        }
    }

    /// Gold cost to purchase this vehicle (from PROPOSAL section 2.6).
    pub fn purchase_cost(&self) -> u64 {
        match self {
            Vehicle::None => 0,
            Vehicle::Bicycle => 500,
            Vehicle::Scooter => 1_000,
            Vehicle::Motorcycle => 2_500,
            Vehicle::Boat => 2_000,
            Vehicle::Airplane => 10_000,
        }
    }

    /// Display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Vehicle::None => "None",
            Vehicle::Bicycle => "Electric Bicycle",
            Vehicle::Scooter => "Electric Scooter",
            Vehicle::Motorcycle => "Electric Motorcycle",
            Vehicle::Boat => "Electric Boat",
            Vehicle::Airplane => "Electric Airplane",
        }
    }

    /// All vehicles sorted by cost (ascending)
    pub fn all_vehicles() -> &'static [Vehicle] {
        &[
            Vehicle::None,
            Vehicle::Bicycle,
            Vehicle::Scooter,
            Vehicle::Boat,
            Vehicle::Motorcycle,
            Vehicle::Airplane,
        ]
    }
}

/// Hex terrain color for rendering.
pub fn terrain_color(terrain: &TerrainType) -> Color {
    match terrain {
        TerrainType::Grass => Color::srgb(0.35, 0.65, 0.2),
        TerrainType::Forest => Color::srgb(0.15, 0.55, 0.25),
        TerrainType::Water => Color::srgb(0.2, 0.4, 0.7),
        TerrainType::City => Color::srgb(0.7, 0.65, 0.55),
        TerrainType::Desert => Color::srgb(0.85, 0.7, 0.3),
        TerrainType::Polluted => Color::srgb(0.15, 0.15, 0.15),
    }
}

/// Orange tetrahedron mesh for the player avatar (placeholder Tamagotchi)
pub fn player_tetrahedron_mesh() -> Mesh {
    let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);

    // Tetrahedron: 4 vertices, 4 triangular faces
    let vertices: [Vec3; 4] = [
        Vec3::new(0.0, 0.4, 0.0),     // apex
        Vec3::new(-0.433, -0.2, 0.2),  // bottom-left
        Vec3::new(0.433, -0.2, 0.2),   // bottom-right
        Vec3::new(0.0, -0.2, -0.4),    // back
    ];

    // 4 faces (indices of the 4 vertices)
    let face_indices: [[u32; 3]; 4] = [
        [0, 1, 4], // front
        [0, 1, 2], // left
        [0, 2, 3], // right
        [0, 3, 4], // back
    ];

    let positions: Vec<[f32; 3]> = vertices.map(|v| v.into());
    let indices: Vec<u32> = face_indices.flat_map(|f| f).collect();

    // Orange color for all faces
    let orange = Color::srgb(1.0, 0.78, 0.2);

    mesh.set_attributes(bevy::render::mesh::MeshVertex::mesh_vertex([
        positions,
        vec![[0.0, 0.0, 1.0]; positions.len()], // normals
        vec![orange; positions.len()],          // diffuse color
    ]));

    mesh.set_indices(Some(bevy::render::render_resource::IndexBuffer::new(
        bevy::render::render_resource::BufferSize::Size(indices.len() as u64 * 4),
        bevy::render::render_resource::IndexFormat::Uint32,
        indices,
    )));

    mesh
}

/// Spawn a player entity with orange tetrahedron at the world center.
pub fn spawn_player(mut commands: Commands) -> Entity {
    let player_entity = commands.spawn((
        Name::new("player"),
        Player::default(),
        player_tetrahedron_mesh(),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Spawn a marker above the player
    commands.spawn((
        Name::new("player_marker"),
        PlayerSpawnMarker,
        Transform::from_xyz(0.0, 1.5, 0.0),
    ));

    println!("Player spawned at (0, 0, 0) with orange tetrahedron avatar");
    player_entity
}
