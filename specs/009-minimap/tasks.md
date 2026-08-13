# Tasks 009: Minimap and Global Map

> **Implementation Checklist**

## Phase 1: Minimap Data Model
- [x] **T1.1** Define ZoomLevel enum (Local, Mid, Global) with radius values
- [x] **T1.2** Define Minimap struct (zoom, offset, viewport_hexes, player_position)
- [x] **T1.3** Define ObjectMarker struct (hex, object_type, label)
- [x] **T1.4** Define MinimapData struct combining position, viewport, objects

## Phase 2: Minimap Rendering
- [x] **T1.5** Create minimap rendering function (draw hexes, player dot, objects) in minimap.rs
- [x] **T1.6** Implement hex_to_pixel conversion for minimap scale
- [x] **T1.7** Render terrain colors on minimap hexes
- [x] **T1.8** Draw player position as blue circle (5px radius) - ObjectType::Player.color()

## Phase 3: Zoom System
- [x] **T1.9** Implement zoom_in() — cycle Local→Mid→Global
- [x] **T1.10** Implement zoom_out() — cycle Global→Mid→Local
- [x] **T1.11** Handle zoom edge cases (already at max/min) - returns None
- [ ] **T1.12** Add keyboard/mouse zoom controls (scroll wheel, +/- keys) - Client side

## Phase 4: Client Integration
- [ ] **T2.1** Subscribe to hex occupancy events on client
- [x] **T2.2** Update minimap when player position changes — sync_player_state system
- [x] **T2.3** Update minimap when other players enter/leave view — render_remote_players dots
- [x] **T2.4** Render minimap overlay in bottom-right corner (minimap.rs)

## Phase 5: Global Map
- [ ] **T2.5** Implement global map toggle (full grid view)
- [ ] **T2.6** Render all 64-hex radius efficiently
- [ ] **T2.7** Add player dots and object markers to global map

## Phase 6: Teleport Integration
- [x] **T3.1** Left-click selects hex — selection ring + selected_hex/selected_px in MinimapState
- [x] **T3.2** Teleport (hex) HUD button + stats line shows dest/cost
- [x] **T3.3** Confirmation — teleport_player reducer with cooldown/cost reporting

## Phase 7: Testing
- [x] **T4.1** Minimap renders hexes correctly (test_minimap_new, test_minimap_viewport_objects)
- [ ] **T4.2** Player position updates in real-time
- [x] **T4.3** Other players visible (within range) — orange dots on minimap
- [x] **T4.4** Zoom in/out works smoothly (test_zoom_in_cycle, test_zoom_out_cycle)
- [ ] **T4.5** Click hex selects destination correctly
- [ ] **T4.6** Global map shows full grid

## Verification
- [x] Zoom levels defined correctly (5, 20, 64 hex radii)
- [x] Minimap rendering function exists
- [x] Zoom cycle works without infinite loop
