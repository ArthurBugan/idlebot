# Spec 016: Assets (Placeholder → Low-Poly → Polish)

> **Objective:** Implement asset pipeline from procedural placeholders to low-poly assets to polished VFX

## Problem Statement

Need a phased asset strategy: procedural placeholders for MVP, low-poly assets for Phase 2, and polished VFX for Phase 3. Assets must be performant and theme-consistent (eco-friendly, electric vehicles).

## Requirements

### Functional Requirements
1. FR1: Procedural placeholders for MVP (Phase 1)
2. FR2: Low-poly asset import (Phase 2)
3. FR3: Vehicle models (5 types)
4. FR4: Cosmetic models (hats, auras, trails)
5. FR5: Plant models (Wheat, Tree, RareHerb)
6. FR6: Terrain materials (6 types)
7. FR7: Animation system (Phase 3)
8. FR8: Particle effects (VFX)

### Non-Functional Requirements
1. NFR1: Low-poly targets: < 500 triangles per model
2. NFR2: Texture size: 256x256 or 512x512 max
3. NFR3: Animation: 30fps minimum
4. NFR4: Asset loading: < 100ms per asset

## Design

### Phase 1: Procedural Placeholders
```rust
// Use Bevy's primitive meshes for MVP
use bevy::prelude::*;

fn spawn_hex_materials(mut commands: Commands) {
    let colors = [
        Color::new_linear(0.49, 0.78, 0.31), // Grass (#7EC850)
        Color::new_linear(0.13, 0.55, 0.13), // Forest (#228B22)
        Color::new_linear(0.25, 0.41, 0.88), // Water (#4169E1)
        Color::new_linear(0.50, 0.50, 0.50), // City (#808080)
        Color::new_linear(0.96, 0.64, 0.38), // Desert (#F4A460)
        Color::new_linear(0.29, 0.00, 0.51), // Polluted (#4B0082)
    ];
    
    for (i, &color) in colors.iter().enumerate() {
        commands.spawn(Material3dComponent {
            material: StandardMaterial {
                base_color: color,
                ..default()
            },
        });
    }
}

// Player as tetrahedron placeholder
fn spawn_player_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        create_tetrahedron_vertices(),
    )
}

fn create_tetrahedron_vertices() -> Vec<Vertex> {
    // 4 vertices, 4 faces
    vec![
        Vertex { position: [0.0, 1.0, 0.0], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [-0.8, -0.5, 0.0], normal: [-0.8, -0.5, 0.0] },
        Vertex { position: [0.8, -0.5, 0.0], normal: [0.8, -0.5, 0.0] },
        Vertex { position: [0.0, -0.5, 0.8], normal: [0.0, -0.5, 0.8] },
    ]
}
```

### Phase 2: Low-Poly Assets
```rust
// Asset loading from glTF files
use bevy::gltf::Gltf;

fn load_vehicle_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let vehicle_paths = [
        "vehicles/bicycle.glb",
        "vehicles/scooter.glb",
        "vehicles/motorcycle.glb",
        "vehicles/boat.glb",
        "vehicles/airplane.glb",
    ];
    
    for path in vehicle_paths {
        let handle = asset_server.load(path);
        commands.spawn((
            Mesh3d(handle),
            Transform::default(),
            Visibility::default(),
        ));
    }
}

// Low-poly constraints
// - < 500 triangles per model
// - 256x256 or 512x512 textures
// - PBR materials (metallic, roughness)
```

### Phase 3: Animations and VFX
```rust
use bevy::animation::{AnimationClip, AnimationPlayer};

// Vehicle animations
fn play_vehicle_animation(
    query: Query<(&AnimationPlayer, &VehicleType)>,
) {
    for (player, vehicle_type) in query.iter() {
        let clip_name = match vehicle_type {
            VehicleType::Bicycle => "pedal",
            VehicleType::Scooter => "idle",
            VehicleType::Motorcycle => "ride",
            VehicleType::Boat => "float",
            VehicleType::Airplane => "fly",
        };
        
        player.play(AnimationClip::new(clip_name)).auto_rebound();
    }
}

// Particle effects
use bevy::pbr::{DeferredLight, PointLight};
use bevy::prelude::*;

fn spawn_aura_vfx(
    mut commands: Commands,
    player_query: Query<&PlayerId>,
) {
    for player_id in player_query.iter() {
        commands.spawn((
            PointLight {
                color: Color::SPECTRUM_ORANGE,
                intensity: 1000.0,
                range: 5.0,
                ..default()
            },
            Transform::from_translation(Vec3::Y * 0.5),
        ));
    }
}
```

### Asset Pipeline
```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.19" }
image = "0.25"
png = "0.17"
```

```bash
# Asset preparation script
./scripts/prepare_assets.sh

# 1. Resize textures
mogrify -resize 512x512 assets/textures/*.png

# 2. Optimize meshes
blender --background --python scripts/optimize_meshes.py

# 3. Convert to glTF
for f in assets/models/*.obj; do
    blender --background --python scripts/convert_to_gltf.py "$f"
done
```

## Acceptance Criteria

### Phase 1 (MVP)
- [ ] Hexes render with terrain colors
- [ ] Player visible as tetrahedron
- [ ] Plants visible (simple cones)
- [ ] Pollution visible (dark markers)

### Phase 2
- [ ] 5 vehicle models imported
- [ ] Cosmetics models imported
- [ ] Plant models (3 types)
- [ ] Terrain materials (6 types)
- [ ] All assets < 500 triangles

### Phase 3
- [ ] Vehicle animations (5)
- [ ] Cosmetic animations (optional)
- [ ] Plant growth animations
- [ ] Aura particle effects
- [ ] Trail particle effects

## Risks
- R1: Asset file size (optimize)
- R2: Animation rigging complexity
- R3: Style consistency across assets

## Open Questions
- Q1: Asset license (CC0, purchased, custom)?
- Q2: Dynamic LOD for distant objects?
- Q3: Shader customization for terrain?
