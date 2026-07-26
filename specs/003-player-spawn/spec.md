# Spec 003: Player Spawn and WASD Movement

> **Objective:** Implement player character with WASD movement system

## Problem Statement

Players need to spawn in the world and navigate using WASD keys. Movement should be smooth, directional, and affected by vehicle speed multipliers.

## Proposed Solution

- Player spawns at nearest empty hex
- WASD movement with 10 m/s base speed
- Direction based on camera angle
- Vehicle speed multipliers (2x-10x)

## Requirements

### Functional Requirements
1. FR1: Spawn player at valid location
2. FR2: WASD movement input handling
3. FR3: Movement speed calculation (base × vehicle multiplier)
4. FR4: Boundary collision (don't walk off grid)
5. FR5: Smooth movement interpolation

### Non-Functional Requirements
1. NFR1: Input latency < 100ms
2. NFR2: Network-synced movement (if multiplayer)
3. NFR3: 60fps movement smoothness

## Design

### Player Spawn
```rust
fn spawn_player(player: &mut Player, world: &World) -> Result<HexCoord> {
    // Find nearest empty grass hex
    let spawn_hex = world.find_nearest_empty_hex(&player.position)?;
    player.position = spawn_hex.to_pixel();
    player.current_hex = spawn_hex;
    Ok(spawn_hex)
}
```

### Movement System
```rust
struct MovementSystem {
    base_speed: f32, // 10.0 m/s
    vehicle_multiplier: f32,
    input: InputState,
}

impl MovementSystem {
    fn update(&mut self, dt: f32, player: &mut Player) {
        let speed = self.base_speed * self.vehicle_multiplier;
        
        let direction = self.calculate_direction();
        let movement = direction * speed * dt;
        
        let new_pos = player.position + movement;
        let new_hex = self.world.hex_at(new_pos);
        
        if new_hex.is_valid() && !self.world.is_blocked(new_hex) {
            player.position = new_pos;
            player.current_hex = new_hex;
        }
    }
    
    fn calculate_direction(&self) -> Vec2 {
        let mut dir = Vec2::ZERO;
        if self.input.w { dir.y += 1.0; }
        if self.input.s { dir.y -= 1.0; }
        if self.input.a { dir.x -= 1.0; }
        if self.input.d { dir.x += 1.0; }
        dir.normalize()
    }
}
```

## Acceptance Criteria
- [ ] Player spawns at valid hex
- [ ] WASD movement works smoothly
- [ ] Vehicle speed multipliers applied
- [ ] Boundary collision prevents walking off grid
- [ ] Movement synced in multiplayer

## Risks
- R1: Network latency in multiplayer
- R2: Stuttering on large grids
