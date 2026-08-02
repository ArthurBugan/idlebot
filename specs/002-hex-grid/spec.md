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

### Terrain Types
| Terrain | Probability | Eco Rating | Color |
|---------|-------------|------------|-------|
| Grass | 50% | 50 | #7EC850 |
| Forest | 20% | 50 | #228B22 |
| Water | 8% | 20 | #4169E1 |
| City | 10% | 20 | #808080 |
| Desert | 7% | 20 | #F4A460 |
| Polluted | 5% | 10 | #4B0082 |

## Risks
- R1: Large grid performance (12k+ meshes)
- R2: Terrain continuity at edges
- R3: Memory usage for hex state

## Open Questions
- Q1: Should grid be infinite or bounded?
- Q2: How to handle hex collision with other hexes?
- Q3: LOD strategy for distant hexes?
