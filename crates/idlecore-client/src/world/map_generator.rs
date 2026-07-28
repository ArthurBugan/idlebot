//! Sistema de mapa - geração procedural de hexágonos

use rand::Rng;

/// Dados de um hexágono no mapa
#[derive(Debug, Clone)]
pub struct HexData {
    pub q: i32,
    pub r: i32,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: TerrainType,
}

/// Tipo de terreno
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerrainType {
    Grass,
    Forest,
    Water,
    City,
    Desert,
    Polluted,
}

impl TerrainType {
    /// Cor do terreno (RGB)
    pub fn color(&self) -> [f32; 3] {
        match self {
            TerrainType::Grass => [0.2, 0.6, 0.2],
            TerrainType::Forest => [0.1, 0.4, 0.1],
            TerrainType::Water => [0.1, 0.3, 0.8],
            TerrainType::City => [0.5, 0.5, 0.5],
            TerrainType::Desert => [0.8, 0.7, 0.3],
            TerrainType::Polluted => [0.3, 0.1, 0.1],
        }
    }
}

/// Gerar o mapa de hexágonos
pub fn generate_hex_map<R: Rng>(rng: &mut R) -> Vec<HexData> {
    let mut hexes = Vec::new();
    let map_radius = 8i32;

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let s = -q - r;
            if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                let center_x = (q as f32) * 1.5;
                let center_y = (r as f32) * 1.3;
                let val = rng.gen_range(0.0..1.0);
                let terrain = match val {
                    0.0..0.50 => TerrainType::Grass,
                    0.50..0.70 => TerrainType::Forest,
                    0.70..0.80 => TerrainType::Water,
                    0.80..0.90 => TerrainType::City,
                    0.90..0.95 => TerrainType::Desert,
                    0.95..1.0 => TerrainType::Polluted,
                    _ => TerrainType::Grass,
                };
                hexes.push(HexData {
                    q,
                    r,
                    center_x,
                    center_y,
                    terrain,
                });
            }
        }
    }

    hexes
}
