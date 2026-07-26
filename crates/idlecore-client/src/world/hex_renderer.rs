//! Renderer de Hexágonos — Bevy
//!
//! Renderiza hexágonos flat-top no mundo 3D usando cores procedurais por tipo de terreno.

use crate::world::map_generator::{self, HexData, TerrainType};
use bevy::prelude::*;

/// Componente de spawn point (centro do mapa)
#[derive(Component)]
pub struct SpawnPoint;

/// Sistema que gera e renderiza todos os hexágonos do mundo
pub fn spawn_world(mut commands: Commands, query: Query<Entity, With<SpawnWorld>>) {
    if !query.is_empty() {
        return;
    }

    let mut rng = rand::thread_rng();
    let hexes = map_generator::generate_hex_map(&mut rng);

    for hex in &hexes {
        let terrain_color = hex.terrain.color();

        commands.spawn((
            crate::assets::procedural::HexMesh {
                q: hex.q,
                r: hex.r,
                center_x: hex.center_x,
                center_y: hex.center_y,
                terrain: hex.terrain,
            },
            Name::new(format!("hex_{}_{}", hex.q, hex.r)),
            terrain_color,
            Transform::from_xyz(hex.center_x, hex.elevation * 0.3, 0.0),
        ));
    }

    // Spawn point no centro
    commands.spawn((
        SpawnPoint,
        Name::new("spawn_point"),
        Transform::from_xyz(0.0, 0.0, 0.5),
        Visibility::default(),
    ));

    tracing::info!("World spawned with {} hexes", hexes.len());
}

/// Criar mesh de hexágono flat-top
fn create_hex_mesh(x: f32, y: f32, radius: f32, color: Color, elevation: f32) -> Mesh {
    let mut vertices = Vec::new();
    for i in 0..6 {
        let angle = std::f32::consts::FRAC_PI_3 * i as f32;
        let vx = x + radius * angle.cos();
        let vy = y + radius * angle.sin();
        let vz = elevation * 0.5;
        vertices.push([vx, vy, vz]);
    }

    let mut indices = Vec::new();
    for i in 1..5 {
        indices.push([0u32, i as u32, (i + 1) as u32]);
    }

    Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList)
}

/// Sistema de minimap
pub fn spawn_minimap(mut commands: Commands) {
    commands.spawn((Camera2d::default(), Name::new("minimap_camera")));
}
