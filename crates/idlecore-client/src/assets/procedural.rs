/// Assets Procedurais para IdleBot
/// Gera meshes 3D programaticamente - sem dependência de arquivos externos
/// Licença: MIT
use bevy::prelude::*;
use bevy::render::mesh::Mesh;

/// Gera um hexágono (flat-top) para o terreno
pub fn create_hex_mesh(radius: f32, height: f32) -> Mesh {
    let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);

    // Gerar vertices do hexágono
    let mut positions = Vec::new();
    for i in 0..6 {
        let angle = std::f32::consts::FRAC_PI_3 * i as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        positions.push([x, y, 0.0]); // Topo
        positions.push([x, y, -height]); // Base
    }

    // Gerar indices para triangulação
    let mut indices = Vec::new();
    for i in 0..6 {
        let next = (i + 1) % 6;
        // Lados
        indices.push([i as u32, next as u32, (i + 6) as u32]);
        indices.push([(i + 6) as u32, (next + 6) as u32, (i + 6) as u32]);
        // Topo e base
        indices.push([i as u32, next as u32, (i + 6) as u32]);
    }

    mesh.set_indices(Some(bevy::render::render_resource::IndexBuffer::new(
        bevy::render::render_resource::BufferSize::Size(indices.len() as u64 * 4),
        bevy::render::render_resource::IndexFormat::Uint32,
        indices,
    )));

    mesh.set_attribute(
        Mesh::ATTRIBUTE_POSITION,
        bevy::render::render_resource::VertexAttribute::new(
            "Position",
            bevy::render::render_resource::VertexFormat::Float32x3,
            0,
        ),
        bevy::render::render_resource::VertexAttributeValues::Float32x3(positions),
    );

    // Gerar normais (plainas, apontando para cima)
    let mut normals = vec![[0.0, 0.0, 1.0].repeat(positions.len())];
    mesh.set_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        bevy::render::render_resource::VertexAttribute::new(
            "Normal",
            bevy::render::render_resource::VertexFormat::Float32x3,
            0,
        ),
        bevy::render::render_resource::VertexAttributeValues::Float32x3(normals),
    );

    mesh
}

/// Gera uma árvore low-poly simples
pub fn create_tree_mesh() -> Mesh {
    let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);

    // Tronco (cilindro simples)
    let trunk_radius = 0.3;
    let trunk_height = 2.0;
    let trunk_segments = 8;

    let mut trunk_positions = Vec::new();
    for i in 0..trunk_segments {
        let angle = std::f32::consts::TAU * i as f32 / trunk_segments as f32;
        let x = trunk_radius * angle.cos();
        let y = trunk_radius * angle.sin();
        trunk_positions.push([x, y, 0.0]);
        trunk_positions.push([x, y, trunk_height]);
    }

    let mut trunk_indices = Vec::new();
    for i in 0..trunk_segments {
        let next = (i + 1) % trunk_segments;
        trunk_indices.push([i as u32, next as u32, (i + trunk_segments) as u32]);
        trunk_indices.push([
            (i + trunk_segments) as u32,
            (next + trunk_segments) as u32,
            (i + trunk_segments) as u32,
        ]);
    }

    // Copa (cone)
    let canopy_radius = 1.5;
    let canopy_height = 3.0;
    let canopy_segments = 8;

    let mut canopy_positions = vec![[0.0, 0.0, canopy_height + trunk_height]]; // Topo do cone
    for i in 0..canopy_segments {
        let angle = std::f32::consts::TAU * i as f32 / canopy_segments as f32;
        let x = canopy_radius * angle.cos();
        let y = canopy_radius * angle.sin();
        canopy_positions.push([x, y, canopy_height]);
    }

    let mut canopy_indices = Vec::new();
    for i in 1..=canopy_segments {
        let next = if i == canopy_segments { 1 } else { i + 1 };
        canopy_indices.push([0u32, i, next]); // Lados do cone
    }

    // Combinar tronco + copa
    let mut all_positions = trunk_positions;
    all_positions.extend(canopy_positions);

    let mut all_indices = trunk_indices;
    all_indices.extend(
        canopy_indices
            .iter()
            .map(|&i| i + trunk_positions.len() as u32),
    );

    mesh.set_indices(Some(bevy::render::render_resource::IndexBuffer::new(
        bevy::render::render_resource::BufferSize::Size(all_indices.len() as u64 * 4),
        bevy::render::render_resource::IndexFormat::Uint32,
        all_indices,
    )));

    mesh.set_attribute(
        Mesh::ATTRIBUTE_POSITION,
        bevy::render::render_resource::VertexAttribute::new(
            "Position",
            bevy::render::render_resource::VertexFormat::Float32x3,
            0,
        ),
        bevy::render::render_resource::VertexAttributeValues::Float32x3(all_positions),
    );

    mesh
}

/// Gera uma planta (cultivo)
pub fn create_plant_mesh(plant_type: &str) -> Mesh {
    let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);

    let height = match plant_type {
        "wheat" => 0.8,
        "tomato" => 1.0,
        "tree" => 3.0,
        "sunflower" => 1.2,
        "rare_herb" => 0.6,
        _ => 1.0,
    };

    // Caule
    let stem_positions = vec![[0.0, 0.0, 0.0], [0.0, 0.0, height]];

    let stem_indices = vec![[0, 1, 2]];

    // Folhas (para plantas maiores)
    let leaf_positions = if height > 1.0 {
        vec![
            [0.5, 0.0, height * 0.5],
            [-0.5, 0.0, height * 0.5],
            [0.0, 0.5, height * 0.7],
            [0.0, -0.5, height * 0.7],
        ]
    } else {
        vec![]
    };

    let mut all_positions = stem_positions;
    all_positions.extend(leaf_positions);

    let mut all_indices = stem_indices;
    all_indices.extend(leaf_indices());

    mesh.set_indices(Some(bevy::render::render_resource::IndexBuffer::new(
        bevy::render::render_resource::BufferSize::Size(all_indices.len() as u64 * 4),
        bevy::render::render_resource::IndexFormat::Uint32,
        all_indices,
    )));

    mesh.set_attribute(
        Mesh::ATTRIBUTE_POSITION,
        bevy::render::render_resource::VertexAttribute::new(
            "Position",
            bevy::render::render_resource::VertexFormat::Float32x3,
            0,
        ),
        bevy::render::render_resource::VertexAttributeValues::Float32x3(all_positions),
    );

    mesh
}

/// Gera índices para folhas
fn leaf_indices() -> Vec<u32> {
    vec![[3, 4, 5], [3, 5, 6]]
}

/// Sistema que spawn automaticamente os meshes no mundo (versão simplificada)
pub fn spawn_procedural_assets(mut commands: Commands) {
    // Apenas gera um ponto de spawn visível
    commands.spawn((
        Name::new("procedural_spawn"),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
    ));

    tracing::info!("Procedural assets spawned!");
}
