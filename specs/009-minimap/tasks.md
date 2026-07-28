# Tasks 009: Minimap and Global Map

> **Implementation Checklist**

## Phase 1: Minimap Data Structure
- [✓] **T1.1** Define MinimapData struct (player_position, viewport_hexes, other_players, objects)
- [ ] **T1.2** Define ObjectMarker struct (hex, object_type, label)
- [ ] **T1.3** Define ZoomLevel enum (Local(5), Mid(20), Global(64))

## Phase 2: Minimap Rendering
- [ ] **T1.4** Render hexes as colored circles/squares
- [ ] **T1.5** Display player position (blue dot)
- [ ] **T1.6** Display other players (green dots, within range)
- [ ] **T1.7** Display objects (plants, pollution, hexes)

## Phase 3: Zoom System
- [ ] **T1.8** Implement zoom_in() method
- [ ] **T1.9** Implement zoom_out() method
- [ ] **T1.10** Display zoom level indicator

## Phase 4: Interaction
- [ ] **T1.11** Click hex to select teleport destination
- [ ] **T1.12** Click hex to scroll view on main map
- [ ] **T1.13** Toggle between minimap and global map

## Phase 5: Performance
- [ ] **T1.14** Cull hexes outside viewport
- [ ] **T1.15** Use sprite batch for hex rendering

## Phase 6: Testing
- [ ] **T1.16** Test minimap renders correctly
- [ ] **T1.17** Test player position updates
- [ ] **T1.18** Test zoom in/out
- [ ] **T1.19** Test click-to-select destination
