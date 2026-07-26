//! Mapa Hexagonal — Geração Procedural
//!
//! Cada hexágono inclui posição, terreno e cor procedurally gerados.

use crate::assets::procedural::{create_hex_mesh, create_tree_mesh, TerrainType};
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

/// Gerar mapa hexagonal procedural baseado em coordenadas reais da Terra
/// Escala 1:10.000 → raio de jogo ~637m
pub fn generate_hex_map(rng: &mut impl Rng) -> Vec<HexData> {
    let mut hexes = Vec::new();
    let hex_radius = 10.0f32;
    let map_radius = 64i32;

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let s = -q - r;
            if q.abs() <= map_radius && r.abs() <= map_radius && s.abs() <= map_radius {
                let center_x = hex_radius * 3.0_f32.sqrt() * (q as f32 + r as f32 / 2.0);
                let center_y = hex_radius * 1.5 * r as f32;
                let terrain = determine_terrain(q, r, rng);
                let elevation = determine_elevation(q, r, rng);

                hexes.push(HexData {
                    q,
                    r,
                    center_x,
                    center_y,
                    terrain,
                    elevation,
                });
            }
        }
    }

    tracing::info!("Generated {} hexagons", hexes.len());
    hexes
}

#[derive(Debug, Clone)]
pub struct HexData {
    pub q: i32,
    pub r: i32,
    pub center_x: f32,
    pub center_y: f32,
    pub terrain: TerrainType,
    pub elevation: f32,
}

/// Implemetação simples de cor por terreno
impl TerrainType {
    pub fn color(&self) -> Color {
        match self {
            TerrainType::Grass => Color::srgb(0.35, 0.65, 0.2),
            TerrainType::Forest => Color::srgb(0.15, 0.55, 0.25),
            TerrainType::Water => Color::srgb(0.2, 0.4, 0.7),
            TerrainType::City => Color::srgb(0.7, 0.65, 0.55),
            TerrainType::Desert => Color::srgb(0.85, 0.7, 0.3),
            TerrainType::Polluted => Color::srgb(0.15, 0.15, 0.15),
        }
    }
}

fn determine_terrain(q: i32, r: i32, rng: &mut impl Rng) -> TerrainType {
    let seed = (q as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add((r as u64).wrapping_mul(1442695040888963407));
    let val = ((seed >> 33) ^ seed) as f32 / u32::MAX as f32;

    if val < 0.50 {
        TerrainType::Grass
    } else if val < 0.70 {
        TerrainType::Forest
    } else if val < 0.78 {
        TerrainType::Water
    } else if val < 0.88 {
        TerrainType::City
    } else if val < 0.95 {
        TerrainType::Desert
    } else {
        TerrainType::Polluted
    }
}

fn determine_elevation(q: i32, r: i32, rng: &mut impl Rng) -> f32 {
    let seed = (q as u64)
        .wrapping_mul(1442695040888963407)
        .wrapping_add((r as u64).wrapping_mul(6364136223846793005));
    let val = ((seed >> 33) ^ seed) as f32 / u32::MAX as f32;
    rng.gen_range(0.0..1.0)
}
