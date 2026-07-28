# Spec 009: Minimap and Global Map

> **Objective:** Implement 2D minimap overlay and full global map view

## Problem Statement

Players need to see their position relative to the world. A minimap provides local context, while the global map enables long-range navigation and teleportation.

## Proposed Solution

- 2D minimap overlay in bottom-right corner
- Zoom levels: local (5-hex radius) to global (full 64-hex radius)
- Player dot, other player dots, object markers
- Click-to-select for teleport destination

## Requirements

### Functional Requirements
1. FR1: Render minimap as 2D overlay
2. FR2: Display player position on minimap
3. FR3: Display other players (within range)
4. FR4: Display objects (plants, pollution, hexes)
5. FR5: Zoom in/out functionality
6. FR6: Click hex to select teleport destination
7. FR7: Global map toggle (full grid view)

### Non-Functional Requirements
1. NFR1: Minimap updates at 30fps
2. NFR2: Global map renders all hexes efficiently
3. NFR3: Minimap doesn't obscure gameplay UI

## Design

### Minimap Data
```rust
struct MinimapData {
    player_position: (f32, f32),
    viewport_hexes: Vec<HexCoord>,
    other_players: Vec<(HexCoord, UUID)>,
    objects: Vec<ObjectMarker>,
}

struct ObjectMarker {
    hex: HexCoord,
    object_type: ObjectType,
    label: Option<String>,
}
```

### Minimap Rendering
```rust
fn render_minimap(&mut self, cam: &Camera, ctx: &mut RenderContext) {
    let hex_data = self.world.get_visible_hexes(cam.view_radius);
    
    for hex in hex_data {
        let pixel = hex.to_pixel();
        let screen_x = pixel.0 * self.minimap_scale;
        let screen_y = pixel.1 * self.minimap_scale;
        
        ctx.draw_hex(screen_x, screen_y, hex.terrain.color());
    }
    
    // Draw player dot
    ctx.draw_circle(
        self.player_position.x,
        self.player_position.y,
        5.0,
        Color::BLUE,
    );
}
```

### Zoom System
```rust
enum ZoomLevel {
    Local(5),    // 5 hex radius
    Mid(20),     // 20 hex radius
    Global(64),  // Full grid
}

struct Minimap {
    zoom: ZoomLevel,
    offset: Vec2,
}

impl Minimap {
    fn zoom_in(&mut self) {
        self.zoom = match self.zoom {
            ZoomLevel::Global(r) => ZoomLevel::Mid(r / 2),
            ZoomLevel::Mid(r) => ZoomLevel::Local(r / 2),
            ZoomLevel::Local(_) => {} // Already at max
        };
    }
    
    fn zoom_out(&mut self) {
        self.zoom = match self.zoom {
            ZoomLevel::Local(r) => ZoomLevel::Mid(r * 2),
            ZoomLevel::Mid(r) => ZoomLevel::Global(r * 2),
            ZoomLevel::Global(_) => {} // Already at min
        };
    }
}
```

## Acceptance Criteria
- [ ] Minimap renders hexes correctly
- [ ] Player position updates in real-time
- [ ] Other players visible (within range)
- [ ] Zoom in/out works smoothly
- [ ] Click hex selects destination
- [ ] Global map shows full grid

## Risks
- R1: Performance with many objects on minimap
- R2: Clarity at high zoom levels

## Open Questions
- Q1: Should minimap show other players' names?
- Q2: Color coding for hex types on minimap?
