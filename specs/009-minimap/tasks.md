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
- [x] **T1.12** Zoom controls — scroll wheel + +/- (and numpad +/-) keys

## Phase 4: Client Integration
- [x] **T2.1** Occupancy events — hex_tile subscription drives plant/pollution visuals and minimap tiles
- [x] **T2.2** Update minimap when player position changes — sync_player_state system
- [x] **T2.3** Update minimap when other players enter/leave view — render_remote_players dots
- [x] **T2.4** Render minimap overlay in bottom-right corner (minimap.rs)

## Phase 5: Global Map
- [x] **T2.5** Global map toggle — M expands to full-grid view
- [x] **T2.6** 64-hex radius — explored-hex cache renders persistently, textures cached by zoom
- [x] **T2.7** Dots render at every zoom incl. Global (world_to_map_pixel path shared)

## Phase 6: Teleport Integration
- [x] **T3.1** Left-click selects hex — selection ring + selected_hex/selected_px in MinimapState
- [x] **T3.2** Teleport (hex) HUD button + stats line shows dest/cost
- [x] **T3.3** Confirmation — teleport_player reducer with cooldown/cost reporting

## Phase 7: Testing
- [x] **T4.1** Minimap renders hexes correctly (test_minimap_new, test_minimap_viewport_objects)
- [x] **T4.2** Realtime position — sync_player_state (PhysicsSet::Writeback ordering)
- [x] **T4.3** Other players visible (within range) — orange dots on minimap
- [x] **T4.4** Zoom in/out works smoothly (test_zoom_in_cycle, test_zoom_out_cycle)
- [x] **T4.5** Hex selection verified — left-click marks the axial hex (selection ring)
- [x] **T4.6** Full grid — expanded mode renders the whole explored map

## Verification
- [x] Zoom levels defined correctly (5, 20, 64 hex radii)
- [x] Minimap rendering function exists
- [x] Zoom cycle works without infinite loop
