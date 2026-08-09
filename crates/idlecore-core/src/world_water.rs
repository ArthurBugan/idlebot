//! Water networks — rivers, lakes, and coastline boundaries (§11-14, §38).
//!
//! Rivers are represented as network data (spline/nodes), not painted onto
//! individual hex cells. The network determines which hexes intersect it via
//! geometric distance queries. This keeps river info from being duplicated
//! in every cell.

use crate::hex::HexCoord;
use serde::{Deserialize, Serialize};

/// A point along a river with width and flow metadata.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiverNode {
    /// World position (x, y).
    pub x: f32,
    pub y: f32,
    /// River half-width at this node.
    pub half_width: f32,
    /// Flow speed at this node (positive = downstream).
    pub flow: f32,
}

impl RiverNode {
    pub fn new(x: f32, y: f32, half_width: f32, flow: f32) -> Self {
        Self { x, y, half_width, flow }
    }
}

/// A river — a directed polyline network from source to destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct River {
    pub id: u64,
    pub name: Option<String>,
    /// Ordered nodes from source toward destination.
    pub nodes: Vec<RiverNode>,
    /// Where the river terminates.
    pub terminates_in: WaterTerminus,
}

/// What a river flows into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterTerminus {
    Ocean,
    Lake,
    Sea,
    AnotherRiver(u64),
}

impl River {
    pub fn new(id: u64, source: RiverNode) -> Self {
        Self {
            id,
            name: None,
            nodes: vec![source],
            terminates_in: WaterTerminus::Ocean,
        }
    }

    pub fn push(&mut self, node: RiverNode) {
        self.nodes.push(node);
    }

    /// Total polyline length in world units.
    pub fn length(&self) -> f32 {
        self.nodes
            .windows(2)
            .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
            .sum()
    }

    /// Minimum distance from a world position to the river polyline.
    /// Returns None if the river has < 2 nodes.
    pub fn distance_to(&self, x: f32, y: f32) -> Option<f32> {
        self.nodes
            .windows(2)
            .map(|w| segment_distance(x, y, w[0].x, w[0].y, w[1].x, w[1].y))
            .reduce(f32::min)
    }

    /// The hex coordinates this river spans (rounded to grid).
    pub fn covered_hexes(&self, hex_radius: f32, hex_pad: f32) -> Vec<HexCoord> {
        // For each node, gather the hex that contains it plus a small pad radius.
        let mut set = std::collections::HashSet::new();
        for node in &self.nodes {
            let (q, r) = crate::hex::world_pos_to_hex(node.x, node.y, hex_radius);
            set.insert(HexCoord::new(q, r));
            if hex_pad > 0.0 {
                for (dq, dr) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, 1), (1, -1)] {
                    set.insert(HexCoord::new(q + dq, r + dr));
                }
            }
        }
        set.into_iter().collect()
    }
}

/// Distance from point `p` to segment `ab`.
fn segment_distance(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// A coastline is a boundary between land and water hexes.
/// This is computed from data; kept as a list of boundary edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Coastline {
    /// Land hex coordinates bordering water.
    pub land_border_hexes: Vec<HexCoord>,
    /// Water hex coordinates bordering land.
    pub water_border_hexes: Vec<HexCoord>,
}

impl Coastline {
    /// From a water mask (per hex coord → is water), compute the borders
    /// around a center coordinate.
    pub fn compute<F>(center_q: i32, center_r: i32, radius: i32, is_water: F) -> Self
    where
        F: Fn(i32, i32) -> bool,
    {
        let neighbors = [
            (-1, 0), (1, 0), (0, -1), (0, 1), (-1, 1), (1, -1),
        ];
        let mut land_border = Vec::new();
        let mut water_border = Vec::new();

        for dq in -radius..=radius {
            for dr in -radius..=radius {
                let q = center_q + dq;
                let r = center_r + dr;
                let water = is_water(q, r);
                let has_opposite = neighbors.iter().any(|(nq, nr)| {
                    is_water(q + nq, r + nr) != water
                });
                if has_opposite {
                    if water {
                        water_border.push(HexCoord::new(q, r));
                    } else {
                        land_border.push(HexCoord::new(q, r));
                    }
                }
            }
        }

        Self { land_border_hexes: land_border, water_border_hexes: water_border }
    }
}

/// Water mask per chunk — boolean coverage + border info for rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkWaterMask {
    /// chunk coordinate (cq, cr).
    pub cq: i32,
    pub cr: i32,
    /// true if the chunk contains any water cell.
    pub has_water: bool,
    /// fraction of water cells in chunk (0..1).
    pub water_fraction: f32,
    /// coastline hexes for this chunk (shared with global coastline when present).
    pub coastline: Option<Coastline>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn river_length() {
        let mut river = River::new(1, RiverNode::new(0.0, 0.0, 5.0, 1.0));
        river.push(RiverNode::new(3.0, 4.0, 5.0, 1.0));
        assert!((river.length() - 5.0).abs() < 1e-3);
    }

    #[test]
    fn river_distance_on_segment() {
        let mut river = River::new(1, RiverNode::new(0.0, 0.0, 5.0, 1.0));
        river.push(RiverNode::new(10.0, 0.0, 5.0, 1.0));
        // Point above the midpoint should be 5 units away
        let d = river.distance_to(5.0, 5.0).unwrap();
        assert!((d - 5.0).abs() < 1e-3);
    }

    #[test]
    fn river_covered_hexes() {
        let mut river = River::new(1, RiverNode::new(0.0, 0.0, 5.0, 1.0));
        river.push(RiverNode::new(30.0, 30.0, 5.0, 1.0));
        let hexes = river.covered_hexes(10.0, 1.0);
        assert!(!hexes.is_empty());
        // Origin hex is covered
        assert!(hexes.contains(&HexCoord::new(0, 0)));
    }

    #[test]
    fn coastline_detects_transition() {
        let mut land = std::collections::HashSet::new();
        land.insert((0, 0));
        let coast = Coastline::compute(0, 0, 2, |q, r| !land.contains(&(q, r)));
        // (0,0) is land surrounded by water → on the border
        assert!(coast.land_border_hexes.contains(&HexCoord::new(0, 0)));
        assert!(coast.water_border_hexes.contains(&HexCoord::new(0, 1)));
    }
}