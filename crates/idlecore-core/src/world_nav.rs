//! Navigation — fog visibility (§29), segment-graph pathfinding (§41),
//! and streaming priority queue with movement prediction (§36).
//!
//! Long-distance navigation avoids running A* over millions of cells: local
//! hex A* (see `world_gen::find_path`) handles the near field, while a coarse
//! segment graph handles the far field.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};

// ============================================================================
// §29 — Fog of War / runtime visibility
// ============================================================================

/// Per-hex runtime visibility state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HexVisibility {
    /// Visible right now (computed each frame).
    Visible,
    /// Previously seen but currently out of vision.
    Hidden,
    /// Never seen.
    Undiscovered,
}

/// A sparse visibility map for loaded hexes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisibilityMap {
    pub states: HashMap<u64, HexVisibility>,
}

fn id(q: i32, r: i32) -> u64 {
    crate::world_gen::HexCell::id_of(q, r)
}

impl VisibilityMap {
    /// Reveal/refresh a hex as Visible.
    pub fn reveal(&mut self, q: i32, r: i32) {
        self.states.insert(id(q, r), HexVisibility::Visible);
    }

    /// Hide a hex that is no longer within vision.
    pub fn hide(&mut self, q: i32, r: i32) {
        if let Some(v) = self.states.get_mut(&id(q, r)) {
            if *v == HexVisibility::Visible {
                *v = HexVisibility::Hidden;
            }
        }
    }

    /// Current visibility of a hex (Undiscovered when unknown).
    pub fn visibility_of(&self, q: i32, r: i32) -> HexVisibility {
        self.states.get(&id(q, r)).copied().unwrap_or(HexVisibility::Undiscovered)
    }

    /// Current-vision model: reveal hexes within radius; hide previously-visible
    /// hexes outside it.
    pub fn update_from_player(&mut self, player_q: i32, player_r: i32, vision_radius: i32) {
        let mut now_visible: HashSet<u64> = HashSet::new();
        for dq in -vision_radius..=vision_radius {
            let r_min = (-vision_radius).max(-dq - vision_radius);
            let r_max = vision_radius.min(-dq + vision_radius);
            for dr in r_min..=r_max {
                let q = player_q + dq;
                let r = player_r + dr;
                let k = id(q, r);
                now_visible.insert(k);
                self.reveal(q, r);
            }
        }
        let to_hide: Vec<u64> = self
            .states
            .iter()
            .filter(|(k, v)| **v == HexVisibility::Visible && !now_visible.contains(k))
            .map(|(k, _)| *k)
            .collect();
        for k in to_hide {
            self.hide_id(k);
        }
    }

    fn hide_id(&mut self, k: u64) {
        if let Some(v) = self.states.get_mut(&k) {
            *v = HexVisibility::Hidden;
        }
    }

    /// Count of currently-visible hexes (for stats/tests).
    pub fn visible_count(&self) -> usize {
        self.states.values().filter(|v| **v == HexVisibility::Visible).count()
    }

    /// Total number of known (revealed-or-hidden) hexes.
    pub fn known_count(&self) -> usize {
        self.states.len()
    }
}

// ============================================================================
// §41 — Segment-graph regional pathfinding
// ============================================================================

/// Edge cost between two segments.
pub type SegmentCost = f32;

/// A segment connectivity graph for regional navigation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SegmentGraph {
    pub nodes: HashMap<u64, (i32, i32)>,
    pub edges: HashMap<u64, Vec<(u64, SegmentCost)>>,
}

impl SegmentGraph {
    pub fn add_segment(&mut self, id: u64, center: (i32, i32)) {
        self.nodes.insert(id, center);
        self.edges.entry(id).or_default();
    }

    /// Connect two segments (undirected) with a traversal cost.
    pub fn connect(&mut self, a: u64, b: u64, cost: SegmentCost) {
        self.edges.entry(a).or_default().push((b, cost));
        self.edges.entry(b).or_default().push((a, cost));
    }

    /// Find a path between segments using Dijkstra.
    pub fn path(&self, from: u64, to: u64) -> Option<Vec<u64>> {
        if !self.nodes.contains_key(&from) || !self.nodes.contains_key(&to) {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let mut dist: HashMap<u64, SegmentCost> = HashMap::new();
        let mut prev: HashMap<u64, u64> = HashMap::new();
        let mut closed = HashSet::new();
        let mut heap = BinaryHeap::new();
        dist.insert(from, 0.0);
        heap.push(HeapEntry { cost: 0.0, id: from });

        while let Some(HeapEntry { cost, id }) = heap.pop() {
            if closed.contains(&id) {
                continue;
            }
            closed.insert(id);
            if id == to {
                let mut path = vec![to];
                let mut cur = to;
                while let Some(&p) = prev.get(&cur) {
                    path.push(p);
                    cur = p;
                }
                path.reverse();
                return Some(path);
            }
            if let Some(neighbors) = self.edges.get(&id) {
                for (nid, edge_cost) in neighbors {
                    if closed.contains(nid) {
                        continue;
                    }
                    let new_cost = cost + edge_cost;
                    if new_cost < *dist.get(nid).unwrap_or(&f32::INFINITY) {
                        dist.insert(*nid, new_cost);
                        prev.insert(*nid, id);
                        heap.push(HeapEntry { cost: new_cost, id: *nid });
                    }
                }
            }
        }

        None
    }
}

/// Heap entry for Dijkstra (min-heap by cost).
#[derive(PartialEq)]
struct HeapEntry {
    cost: f32,
    id: u64,
}
impl Eq for HeapEntry {}
impl std::cmp::Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse so BinaryHeap acts as min-heap.
        other.cost.partial_cmp(&self.cost).unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl std::cmp::PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// §35-36 — Generation priority queue + movement prediction
// ============================================================================

/// Priority levels for streaming chunk generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GenPriority {
    PlayerChunk,
    Adjacent,
    MovementDirection,
    MinimapPreload,
    Background,
}

/// A pending chunk generation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenRequest {
    pub cq: i32,
    pub cr: i32,
    pub priority: GenPriority,
}

/// A priority queue of chunk generation requests.
#[derive(Debug, Clone, Default)]
pub struct GenQueue {
    pub queued: HashMap<(i32, i32), GenPriority>,
    pub processed: HashSet<(i32, i32)>,
}

impl GenQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a request; upgrades priority if already queued higher.
    pub fn push(&mut self, cq: i32, cr: i32, priority: GenPriority) {
        let key = (cq, cr);
        if self.processed.contains(&key) {
            return;
        }
        let entry = self.queued.entry(key).or_insert(priority);
        if priority < *entry {
            *entry = priority;
        }
    }

    /// Pop the highest-priority request (smallest `GenPriority` = most urgent).
    pub fn next(&mut self) -> Option<GenRequest> {
        let (key, priority) = self
            .queued
            .iter()
            .min_by(|a, b| a.1.cmp(b.1))
            .map(|(k, p)| (*k, *p))?;
        self.queued.remove(&key);
        self.processed.insert(key);
        Some(GenRequest { cq: key.0, cr: key.1, priority })
    }

    pub fn pending(&self) -> usize {
        self.queued.len()
    }
}

/// Movement-prediction helper: given a heading, prioritize chunks ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MovementVector {
    pub dq: i32,
    pub dr: i32,
}

impl MovementVector {
    pub fn from_positions(prev: (i32, i32), curr: (i32, i32)) -> Self {
        Self {
            dq: curr.0 - prev.0,
            dr: curr.1 - prev.1,
        }
    }

    /// Whether `target` is meaningfully ahead of `center` along the heading.
    pub fn is_ahead(&self, center: (i32, i32), target: (i32, i32)) -> bool {
        let dq = target.0 - center.0;
        let dr = target.1 - center.1;
        // Dot product of displacement with normalized heading.
        let dot = dq * self.dq + dr * self.dr;
        dot > 0 && dq.abs().max(dr.abs()) >= 2
    }
}

/// High-level streaming coordinator: builds a prioritized queue each tick.
#[derive(Debug, Clone, Default)]
pub struct StreamingManager {
    pub queue: GenQueue,
    pub prediction: MovementVector,
}

impl StreamingManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue chunk generation requests around the player chunk with priorities.
    pub fn update(&mut self, player_chunk: (i32, i32), active_radius: i32, prefetch_radius: i32) {
        for dcq in -prefetch_radius..=prefetch_radius {
            for dcr in -prefetch_radius..=prefetch_radius {
                let cq = player_chunk.0 + dcq;
                let cr = player_chunk.1 + dcr;
                let dist2 = dcq.abs() + dcr.abs();
                let in_active = dcq.abs() <= active_radius && dcr.abs() <= active_radius;
                let pri = if dcq == 0 && dcr == 0 {
                    GenPriority::PlayerChunk
                } else if in_active && dist2 <= 2 {
                    GenPriority::Adjacent
                } else if self.prediction.is_ahead(player_chunk, (cq, cr)) {
                    GenPriority::MovementDirection
                } else if in_active {
                    GenPriority::MinimapPreload
                } else {
                    GenPriority::Background
                };
                self.queue.push(cq, cr, pri);
            }
        }
    }

    /// Pop the single highest-priority request to process this tick.
    pub fn take_next(&mut self) -> Option<GenRequest> {
        self.queue.next()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_reveal_hide() {
        let mut map = VisibilityMap::default();
        map.reveal(0, 0);
        assert_eq!(map.visibility_of(0, 0), HexVisibility::Visible);
        map.hide(0, 0);
        assert_eq!(map.visibility_of(0, 0), HexVisibility::Hidden);
    }

    #[test]
    fn visibility_player_update() {
        let mut map = VisibilityMap::default();
        map.update_from_player(0, 0, 3);
        assert_eq!(map.visibility_of(3, 0), HexVisibility::Visible);
        map.update_from_player(0, 0, 1);
        assert_eq!(map.visibility_of(3, 0), HexVisibility::Hidden);
        assert_eq!(map.visibility_of(1, 0), HexVisibility::Visible);
    }

    #[test]
    fn segment_graph_finds_cheapest_path() {
        let mut g = SegmentGraph::default();
        g.add_segment(1, (0, 0));
        g.add_segment(2, (0, 1));
        g.add_segment(3, (0, 2));
        g.connect(1, 2, 1.0);
        g.connect(2, 3, 1.0);
        g.connect(1, 3, 10.0);
        assert_eq!(g.path(1, 3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn gen_queue_priority_upgrade() {
        let mut q = GenQueue::new();
        q.push(0, 0, GenPriority::MinimapPreload);
        q.push(0, 0, GenPriority::PlayerChunk); // upgrade
        q.push(5, 5, GenPriority::PlayerChunk);
        let first = q.next().unwrap();
        assert_eq!(first.priority, GenPriority::PlayerChunk);
        let second = q.next().unwrap();
        assert_eq!(second.priority, GenPriority::PlayerChunk);
        // (0,0) was upgraded away from MinimapPreload — both are PlayerChunk.
        assert!(q.pending() == 0);
    }

    #[test]
    fn movement_prediction_ahead() {
        let mv = MovementVector::from_positions((0, 0), (3, 0));
        assert!(mv.is_ahead((3, 0), (6, 0)));
        assert!(!mv.is_ahead((3, 0), (3, 0)));
        assert!(!mv.is_ahead((3, 0), (0, 0)));
    }
}