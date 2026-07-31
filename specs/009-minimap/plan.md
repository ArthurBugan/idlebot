# Plan 009: Minimap and Global Map

> **Implementation Plan**

## Architecture

### Minimap Component
- 2D minimap overlay in bottom-right corner
- Zoom levels: Local (5-hex), Mid (20-hex), Global (64-hex)
- Click-to-select hex for teleport destination
- Real-time updates at 30fps

### Global Map View
- Full 64-hex radius grid view
- Toggle between local minimap and global map
- All hexes with terrain colors and player dots

## Files to Create/Modify

### Client (idlecore-client)
- `src/minimap.rs` — Minimap struct, zoom state, rendering
- Modify `src/world/hex_renderer.rs` — Add minimap hex rendering

### Core (idlecore-core)
- Modify `src/ui.rs` — Add minimap input handling (zoom, click)

## Dependencies
- Requires 002-hex-grid (hex coord system)
- Requires 003-player-spawn (player positions)
- Requires 019-database-schema (table definitions)

## Testing Strategy
1. Unit test: Zoom in/out cycles correctly
2. Unit test: Hex rendering in minimap
3. Integration test: Minimap updates when player moves
4. Edge case: Global map renders all hexes

## Timeline
- **Estimate:** 2 days
- **Phase:** Post-MVP Quality of Life
