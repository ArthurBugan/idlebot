# Spec 002: Hex Grid Generation and Rendering

> **Objective:** Create a 3D hexagonal grid world with terrain types and procedural generation

## Problem Statement

Players need a navigable 3D world rendered as a hex grid. The grid must support terrain types, objects (plants, pollution markers), and efficient rendering for performance.

## Proposed Solution

- Axial coordinate system (q, r, s) for hex addressing
- Procedural generation with seed-based terrain distribution
- Flat-top hex geometry with terrain-based coloring
- Efficient culling and LOD for large grids

## Requirements

### Functional Requirements
1. FR1: Generate hex grid with axial coordinates
2. FR2: Assign terrain types based on probability
3. FR3: Render hexes as 3D flat-top tiles
4. FR4: Display plants and pollution markers on hexes
5. FR5: Support grid queries (get hex at position, get neighbors)

### Non-Functional Requirements
1. NFR1: Render 12,480+ hexes at 60fps
2. NFR2: Memory-efficient (instanced rendering)
3. NFR3: Deterministic generation (same seed = same world)
4. NFR4: Support dynamic terrain updates

## Design

### Hex Coordinate System
```rust
struct HexCoord {
    q: i32,
    r: i32,
    s: i32, // q + r + s = 0
}

impl HexCoord {
    fn to_pixel(&self) -> (f32, f32) {
        let x = self.hex_radius * f32::sqrt(3.0) * (self.q as f32 + self.r as f32 / 2.0);
        let y = self.hex_radius * 1.5 * self.r as f32;
        (x, y)
    }
    
    fn to_id(&self) -> u64 {
        ((self.q as u64) << 32) | (self.r as u64)
    }
}
```

### Terrain Types
| Terrain | Probability | Eco Rating | Color |
|---------|-------------|------------|-------|
| Grass | 50% | 50 | #7EC850 |
| Forest | 20% | 50 | #228B22 |
| Water | 8% | 20 | #4169E1 |
| City | 10% | 20 | #808080 |
| Desert | 7% | 20 | #F4A460 |
| Polluted | 5% | 10 | #4B0082 |

### Generation Algorithm
```rust
fn generate_hex_grid(seed: u64, radius: i32) -> HashMap<u64, HexTile> {
    let mut grid = HashMap::new();
    let mut rng = SeedRng::from_seed(seed);
    
    for q in -radius..=radius {
        for r in -radius..=(radius - q.abs()) {
            let s = -(q + r);
            if (q as i64).abs() <= radius as i64 && (r as i64).abs() <= radius as i64 {
                let hex_id = (q as u64) << 32 | (r as u64);
                let terrain = rng.terrain_distribution();
                grid.insert(hex_id, HexTile {
                    coord: HexCoord { q, r, s },
                    terrain,
                    elevation: rng.f32_range(0.0, 1.0),
                    ..Default::default()
                });
            }
        }
    }
    
    grid
}
```

## Acceptance Criteria
- [ ] Grid generates with correct hex count (~12,480)
- [ ] Terrain distribution matches probabilities
- [ ] Hexes render as flat-top 3D tiles
- [ ] Plants and pollution visible on hexes
- [ ] Grid queries return correct results
- [ ] Performance meets 60fps target

## Risks
- R1: Large grid performance (12k+ meshes)
- R2: Terrain continuity at edges
- R3: Memory usage for hex state

## Open Questions
- Q1: Should grid be infinite or bounded?
- Q2: How to handle hex collision with other hexes?
- Q3: LOD strategy for distant hexes?
