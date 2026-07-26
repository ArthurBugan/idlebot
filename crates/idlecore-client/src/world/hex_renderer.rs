//! Renderer de Hexágonos — Bevy
//!
//! Renderiza hexágonos flat-top no mundo 3D usando cores procedurais por tipo de terreno.

use crate::map_generator::{HexData, TerrainType};
use bevy::prelude::*;

/// Componente de spawn point (centro do mapa)
#[derive(Component)]
pub struct SpawnHex;

/// Componente para mesh de hexágono
#[derive(Component)]
pub struct HexMesh {
    pub q: i32,
    pub r: i32,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: TerrainType,
}

/// Sistema que gera e renderiza todos os hexágonos do mundo
pub fn spawn_world(mut commands: Commands, query: Query<Entity, With<SpawnHex>>) {
    if !query.is_empty() {
        return;
    }

    let mut rng = rand::thread_rng();
    let hexes = crate::map_generator::generate_hex_map(&mut rng);

    for hex in &hexes {
        commands.spawn((
            Name::new(format!("hex_{}_{}", hex.q, hex.r)),
            Transform::from_xyz(hex.center_x, 0.0, hex.center_y),
            Mesh3d::default(),
            MeshMaterial3d::<StandardMaterial>::default(),
            HexMesh {
                q: hex.q,
                r: hex.r,
                center_x: hex.center_x,
                center_y: hex.center_y,
                terrain: hex.terrain,
            },
        ));
    }

    // Mark that we've spawned
    commands.spawn(SpawnHex);
}
