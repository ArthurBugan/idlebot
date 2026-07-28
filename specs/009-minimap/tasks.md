# Tasks 009: Minimap and Global Map

> **Implementation Checklist**

## Phase 1: Minimap Data Structures
- [✓] **T1.1** Create MinimapData struct in idlecore-core/src/minimap.rs
- [✗] **T1.2** Implement hex visibility calculation for different zoom levels
- [✗] **T1.3** Implement player position tracking on minimap
- [✗] **T1.4** Implement other player dots on minimap
- [✗] **T1.5** Write unit tests for hex visibility calculation

## Phase 2: Rendering Strategy
- [✗] **T2.1** Create Bevy sprite rendering system for minimap
- [✗] **T2.2** Render hex tiles with terrain colors
- [✗] **T2.3** Render player dot (blue circle)
- [✗] **T2.4** Render other player dots
- [✗] **T2.5** Write unit tests for rendering

## Phase 3: Zoom Controls
- [✗] **T3.1** Implement zoom in/out functionality
- [✗] **T3.2** Implement zoom levels: Local (5-hex), Mid (20-hex), Global (64-hex)
- [✗] **T3.3** Wire mouse wheel to zoom
- [✗] **T3.4** Test zoom transitions

## Phase 4: Teleport Integration
- [✗] **T4.1** Wire minimap click to select teleport destination
- [✗] **T4.2** Implement hex selection mode (single click)
- [✗] **T4.3** Test teleport selection via minimap

## Phase 5: Testing & Polish
- [✗] **T5.1** Integration test: minimap renders correctly at all zoom levels
- [✗] **T5.2** Performance test: minimap updates at 30fps
- [✗] **T5.3** Edge case: teleport selection works at all zoom levels
- [✗] **T5.4** Edge case: multiple players visible on minimap
- [✗] **T5.5** Visual test: minimap looks correct (colors, layout)

## Verification
- [✗] All unit tests pass
- [✗] Minimap renders hexes correctly at all zoom levels
- [✗] Player position updates in real-time
- [✗] Other players visible (within range)
- [✗] Zoom in/out works smoothly
- [✗] Click hex selects destination
- [✗] Global map shows full grid
- [✗] Minimap updates at 30fps
