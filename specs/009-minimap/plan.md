# Plan 009: Minimap and Global Map

> **Implementation Plan**

## Architecture

### Minimap Data
- 2D minimap overlay in bottom-right corner
- Zoom levels: Local (5-hex radius) to Global (64-hex radius)
- Player dot, other player dots, object markers
- Click hex to select teleport destination

### Rendering Strategy
- Bevy sprite rendering for hex tiles on minimap
- Instantiated 2D sprites for player dots
- Single draw call optimization via Bevy's batching

## Files to Create/Modify

### Core (idlecore-core)
- `src/minimap.rs` — MinimapData struct, zoom levels, hex visibility calculation

### Client (idlecore-client)
- `src/minimap.rs` — Minimap rendering system, zoom controls
- `src/main.rs` — Wire minimap into Bevy app
- `src/input.rs` — Mouse wheel zoom, click-to-select

## Testing Strategy
1. Unit test: Hex visibility calculation for different zoom levels
2. Unit test: Minimap rendering at 30fps
3. Integration test: Player movement updates minimap position
4. Edge case: Teleport selection via minimap click

## Dependencies
- Depends on 002-hex-grid (hex coordinates for rendering)
- Depends on 003-player-spawn (player position)
- Depends on 008-teleport (teleport selection integration)

## Timeline
- **Estimate:** 1-2 days
- **Phase:** MVP Core Loop
