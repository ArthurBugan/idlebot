//! Minimap rendering — Minecraft-style persistent map with texture atlas

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;


/// Resource tracking minimap state
#[derive(Resource, Debug)]
pub struct MinimapState {
    pub player_pos: Option<(f32, f32)>,
    pub world_center: (f32, f32),
    pub recenter_requested: bool,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub needs_rebuild: bool,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            player_pos: None,
            world_center: (0.0, 0.0),
            recenter_requested: false,
            atlas_width: 1024,
            atlas_height: 1024,
            needs_rebuild: true,
        }
    }
}

/// Marker components
#[derive(Component)]
pub struct MinimapMarker;

#[derive(Component)]
pub struct MinimapContent;

#[derive(Component)]
pub struct WorldTiles;

#[derive(Component)]
pub struct PlayerMarker;

#[derive(Component)]
pub struct CoordsMarker;

#[derive(Component)]
pub struct HexTileEntity;

#[derive(Component)]
pub struct MinimapAtlasSprite;

#[derive(Component)]
pub struct WorldTileMarker;

/// Track hex mesh handles to avoid recreating every frame
#[derive(Resource, Default)]
pub struct HexMeshCache {
    pub hex_150: Option<Handle<Mesh>>,
}

impl HexMeshCache {
    pub fn get_or_create(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        if self.hex_150.is_none() {
            self.hex_150 = Some(meshes.add(create_hex_mesh(150.0)));
        }
        self.hex_150.clone().unwrap()
    }
}

/// Spawn 3D hex tiles for the world floor near the player (with despawning)
pub fn spawn_world_tiles(
    world_resource: Res<crate::plugins::world::WorldResource>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut hex_cache: ResMut<HexMeshCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tile_map: ResMut<WorldTileEntityMap>,
    mut material_cache: ResMut<MaterialCache>,
    player_query: Query<&Transform, With<crate::player::Player>>,
) {
    // Get actual player position from transform
    let player_transform = match player_query.iter().next() {
        Some(t) => t,
        None => return,
    };
    let player_pos = (player_transform.translation.x, player_transform.translation.z);

    let render_radius = 800.0;
    let render_radius_sq = render_radius * render_radius;

    // Create shared hex mesh once on first run
    let hex_mesh_handle = hex_cache.get_or_create(&mut meshes);

    // Track which tiles should be visible this frame
    let mut visible_ids = std::collections::HashSet::new();

    for tile in world_resource.world.tiles.values() {
        let dx = tile.center_x - player_pos.0;
        let dy = tile.center_y - player_pos.1;
        let dist_sq = dx * dx + dy * dy;

        if dist_sq <= render_radius_sq {
            visible_ids.insert(tile.hex_id);

            if tile_map.tile_entities.contains_key(&tile.hex_id) {
                // Tile already exists — skip (don't respawn)
                continue;
            }

            // Get terrain color and create/cached material
            let terrain_color = tile.terrain.color();
            let color_bytes = [
                (terrain_color.r * 255.0) as u8,
                (terrain_color.g * 255.0) as u8,
                (terrain_color.b * 255.0) as u8,
            ];

            let material_handle = material_cache.cache.entry(color_bytes).or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(
                        terrain_color.r,
                        terrain_color.g,
                        terrain_color.b,
                        1.0,
                    ),
                    unlit: true,
                    ..default()
                })
            }).clone();

            let entity = commands.spawn((
                Name::new(format!("hex-{}", tile.hex_id)),
                WorldTileMarker,
                Transform::from_xyz(tile.center_x, tile.elevation * 0.5, tile.center_y),
                GlobalTransform::default(),
                Mesh3d(hex_mesh_handle.clone()),
                MeshMaterial3d(material_handle),
            )).id();

            tile_map.tile_entities.insert(tile.hex_id, entity);
        }
    }

    // Despawn tiles no longer visible (outside render radius)
    let to_remove: Vec<u64> = tile_map
        .tile_entities
        .iter()
        .filter(|(&id, _)| !visible_ids.contains(&id))
        .map(|(&id, _)| id)
        .collect();

    for id in to_remove {
        if let Some(entity) = tile_map.tile_entities.remove(&id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Create a hexagonal mesh for world tiles (flat-top orientation)
fn create_hex_mesh(radius: f32) -> bevy::render::mesh::Mesh {
    use bevy::render::mesh::MeshVertexAttribute;
    use bevy::render::mesh::VertexFormat;

    let height = 0.5; // Tile height
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate 6 corner vertices for bottom face
    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::TAU / 6.0;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]); // Bottom normal
    }

    // Generate 6 corner vertices for top face
    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::TAU / 6.0;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        positions.push([x, height, z]);
        normals.push([0.0, 1.0, 0.0]); // Top normal
    }

    // Top face (fan triangulation from vertex 6)
    for i in 1..5 {
        indices.extend_from_slice(&[6u32, (6 + i) as u32, (6 + i + 1) as u32]);
    }

    // Bottom face (fan triangulation from vertex 0, CW when viewed from below)
    for i in 1..5 {
        indices.extend_from_slice(&[0u32, (0 + i + 1) as u32, (0 + i) as u32]);
    }

    let mut mesh = bevy::render::mesh::Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        default(),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new(
            "Vertex_Position",
            0,
            VertexFormat::Float32x3,
        ),
        bevy::render::mesh::VertexAttributeValues::Float32x3(positions),
    );

    mesh.insert_attribute(
        MeshVertexAttribute::new(
            "Vertex_Normal",
            1,
            VertexFormat::Float32x3,
        ),
        bevy::render::mesh::VertexAttributeValues::Float32x3(normals),
    );

    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}

/// Spawn minimap UI with texture atlas container
pub fn spawn_minimap_ui(mut commands: Commands) {
    commands.spawn((
        Name::new("minimap-ui"),
        MinimapMarker,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(10.0),
            width: Val::Px(200.0),
            height: Val::Px(200.0),
            ..default()
        },
    ))
    .insert(BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.95)))
    .insert(BorderColor::all(Color::srgba(0.4, 0.6, 0.8, 1.0)))
    .insert(Visibility::Visible)
    .with_children(|parent| {
        // Texture atlas container
        parent.spawn((
            Name::new("minimap-atlas"),
            MinimapAtlasSprite,
            Node {
                width: Val::Px(200.0),
                height: Val::Px(200.0),
                position_type: PositionType::Relative,
                overflow: Overflow::visible(),
                ..default()
            },
        ));

        // Player marker — cyan dot
        parent.spawn((
            Name::new("player-marker"),
            PlayerMarker,
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                position_type: PositionType::Absolute,
                left: Val::Px(94.0),
                top: Val::Px(94.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 1.0, 1.0, 1.0)),
            BorderColor::all(Color::WHITE),
        ));

        // Coords label — bottom-left
        parent.spawn((
            Name::new("coords-label"),
            CoordsMarker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                bottom: Val::Px(2.0),
                ..default()
            },
        ))
        .with_child((
            Text::default(),
            TextFont { font_size: FontSize::Px(9.0), ..default() },
            TextColor(Color::srgba(0.7, 0.8, 1.0, 1.0)),
        ))
        .with_child(TextSpan::new("0, 0"));

        // World tiles container — holds all hex tile entities
        parent.spawn((
            Name::new("world-tiles"),
            WorldTiles,
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
        ));
    });
}

/// Update player position in minimap state
pub fn update_player_pos_system(
    player_query: Query<&Transform, With<crate::player::Player>>,
    mut minimap_state: ResMut<MinimapState>,
) {
    let Some(transform) = player_query.iter().next() else {
        return;
    };
    minimap_state.player_pos = Some((transform.translation.x, transform.translation.z));
}

/// Discover tiles near the player using chunk indexing
pub fn discover_nearby_tiles_system(
    world_resource: Res<crate::plugins::world::WorldResource>,
    minimap_state: Res<MinimapState>,
    mut discovered_tiles: ResMut<crate::discovered_tiles::DiscoveredTiles>,
) {
    let Some((px, py)) = minimap_state.player_pos else {
        return;
    };

    use idlecore_core::hex::HexCoord;
    use idlecore_core::hex_grid::HexGrid;
    let (q, r) = HexGrid::world_to_axial(px, py, 150.0);
    let player_hex = HexCoord::new(q, r);

    let discovery_radius = discovered_tiles.discovery_radius;
    let chunk_size = world_resource.world.chunk_size;
    let chunk_radius = (discovery_radius / (idlecore_core::world::HEX_SIZE as f32)) as i32 / chunk_size + 1;
    let center_cq = player_hex.q / chunk_size;
    let center_cr = player_hex.r / chunk_size;

    for cq in center_cq - chunk_radius..=center_cq + chunk_radius {
        for cr in center_cr - chunk_radius..=center_cr + chunk_radius {
            if let Some(tile_ids) = world_resource.world.loaded_chunks.get(&(cq, cr)) {
                for &tile_hex_id in tile_ids {
                    if discovered_tiles.is_discovered(tile_hex_id) {
                        continue;
                    }
                    if let Some(tile) = world_resource.world.tiles.get(&tile_hex_id) {
                        let dx = tile.center_x - px;
                        let dy = tile.center_y - py;
                        if dx * dx + dy * dy <= discovery_radius * discovery_radius {
                            discovered_tiles.discover_tile(tile.hex_id, tile.center_x, tile.center_y);
                        }
                    }
                }
            }
        }
    }
}

/// Track hex entities created this frame for proper despawning
#[derive(Resource, Default)]
pub struct HexEntityMap {
    pub hex_entities: HashMap<u64, Entity>,
}

/// Track 3D world tile entities for proper despawning
#[derive(Resource, Default)]
pub struct WorldTileEntityMap {
    pub tile_entities: HashMap<u64, Entity>,
}

/// Cache of materials by terrain color to avoid recreating every frame
#[derive(Resource, Default)]
pub struct MaterialCache {
    pub cache: HashMap<[u8; 3], Handle<StandardMaterial>>,
}

/// Spawn/update despawn hex entities for discovered tiles (proper lifecycle management)
pub fn chunk_spawn_hex_system(
    world_resource: Option<Res<crate::plugins::world::WorldResource>>,
    minimap_state: Option<Res<MinimapState>>,
    discovered_tiles: Option<Res<crate::discovered_tiles::DiscoveredTiles>>,
    world_tiles_query: Query<Entity, With<WorldTiles>>,
    mut hex_entity_map: ResMut<HexEntityMap>,
    mut hex_node_query: Query<&mut Node, (With<HexTileEntity>, Without<WorldTiles>)>,
    mut hex_bg_query: Query<&mut BackgroundColor, (With<HexTileEntity>, Without<WorldTiles>)>,
    mut commands: Commands,
) {
    let Some(world) = world_resource else {
        return;
    };
    let Some(state) = minimap_state else {
        return;
    };
    let Some(discovered) = discovered_tiles else {
        return;
    };
    let Some(world_tiles_entity) = world_tiles_query.iter().next() else {
        return;
    };

    let world_cx = state.world_center.0;
    let world_cy = state.world_center.1;
    let scale = world.scale;
    let hex_w = 1.732 * (150.0 * scale);
    let render_radius = 400.0;

    let mut current_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut spawned = 0;

    // Spawn or update entities for currently visible discovered tiles
    for &tile_hex_id in &discovered.tiles {
        let tile = match world.world.tiles.get(&tile_hex_id) {
            Some(t) => t,
            None => continue,
        };

        let Some((px, py)) = state.player_pos else {
            continue;
        };

        let is_current = {
            let dx = tile.center_x - px;
            let dy = tile.center_y - py;
            dx * dx + dy * dy <= render_radius * render_radius
        };

        let rel_x = tile.center_x - world_cx;
        let rel_y = tile.center_y - world_cy;
        let screen_x = 100.0 + rel_x * scale;
        let screen_y = 100.0 + rel_y * scale;

        let color = tile.terrain.color();
        let bg_color = if is_current {
            Color::srgba(color.r, color.g, color.b, 1.0)
        } else {
            Color::srgba(color.r * 0.4, color.g * 0.4, color.b * 0.4, 0.7)
        };

        let tile_size = hex_w * 0.9;

        if let Some(&existing_entity) = hex_entity_map.hex_entities.get(&tile_hex_id) {
            // Update existing entity position
            if let Ok(mut node) = hex_node_query.get_mut(existing_entity) {
                node.left = Val::Px(screen_x - tile_size / 2.0);
                node.top = Val::Px(screen_y - tile_size / 2.0);
            }
            if let Ok(mut bg) = hex_bg_query.get_mut(existing_entity) {
                **bg = bg_color;
            }
            spawned += 1;
        } else {
            // Spawn new entity
            commands.entity(world_tiles_entity).with_children(|parent| {
                parent.spawn((
                    Name::new(format!("hex-{}", tile.hex_id)),
                    HexTileEntity,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(screen_x - tile_size / 2.0),
                        top: Val::Px(screen_y - tile_size / 2.0),
                        width: Val::Px(tile_size),
                        height: Val::Px(tile_size),
                        border_radius: BorderRadius::all(Val::Px(tile_size / 2.0)),
                        ..default()
                    },
                    BackgroundColor(bg_color),
                ));
            });
            spawned += 1;
        }

        current_ids.insert(tile_hex_id);
    }

    // Despawn entities no longer visible (in map but not in current_ids)
    let to_despawn: Vec<Entity> = hex_entity_map
        .hex_entities
        .iter()
        .filter(|(&id, _)| !current_ids.contains(&id))
        .map(|(_, &entity)| entity)
        .collect();

    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
    hex_entity_map.hex_entities.retain(|id, _| current_ids.contains(id));

    if spawned > 0 {
        eprintln!("[MINIMAP] Spawned {} hex tiles via ECS (total: {})", spawned, discovered.count());
    }
}

/// Build texture atlas and update the sprite
pub fn build_minimap_atlas(
    world_resource: Res<crate::plugins::world::WorldResource>,
    mut minimap_state: ResMut<MinimapState>,
    discovered_tiles: Res<crate::discovered_tiles::DiscoveredTiles>,
    mut assets: ResMut<Assets<Image>>,
    mut sprite_query: Query<&mut Sprite, With<MinimapAtlasSprite>>,
) {
    if !minimap_state.needs_rebuild {
        return;
    }

    let width = minimap_state.atlas_width;
    let height = minimap_state.atlas_height;
    let mut pixels = vec![0u8; width as usize * height as usize * 4]; // RGBA initialized to black

    let world_cx = minimap_state.world_center.0;
    let world_cy = minimap_state.world_center.1;
    let scale = world_resource.scale;

    // Transform discovered tiles to atlas coordinates and fill pixels
    for &tile_hex_id in &discovered_tiles.tiles {
        let tile = match world_resource.world.tiles.get(&tile_hex_id) {
            Some(t) => t,
            None => continue,
        };

        let rel_x = tile.center_x - world_cx;
        let rel_y = tile.center_y - world_cy;
        let atlas_x = (width as f32 / 2.0 + rel_x * scale) as i32;
        let atlas_y = (height as f32 / 2.0 + rel_y * scale) as i32;

        let color = tile.terrain.color();
        // Draw a small circle/square for each tile (simplified)
        let radius = 2;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    let px = (atlas_x + dx) as usize;
                    let py = (atlas_y + dy) as usize;
                    if px < width as usize && py < height as usize {
                        let idx = (py * width as usize + px) * 4;
                        pixels[idx] = (color.r * 255.0) as u8;
                        pixels[idx + 1] = (color.g * 255.0) as u8;
                        pixels[idx + 2] = (color.b * 255.0) as u8;
                        pixels[idx + 3] = 255; // fully opaque
                    }
                }
            }
        }
    }

    // Create the image using Bevy 0.19 API with actual pixel data
    let image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixels,
        TextureFormat::Rgba8UnormSrgb,
        Default::default(),
    );

    let handle = assets.add(image);
    if let Some(mut sprite) = sprite_query.iter_mut().next() {
        sprite.image = handle;
    }

    minimap_state.needs_rebuild = false;
}

/// Update minimap UI
pub fn update_minimap_ui(
    minimap_state: Option<Res<MinimapState>>,
    mut coords_query: Query<&mut TextSpan, With<CoordsMarker>>,
    mut player_query: Query<&mut Node, With<PlayerMarker>>,
) {
    let Some(state) = minimap_state else {
        return;
    };
    if let Some((x, y)) = state.player_pos {
        if let Some(mut coords) = coords_query.iter_mut().next() {
            **coords = format!("{:.0}, {:.0}", x, y);
        }
    }

    if let (Some((x, y)), Some(mut marker)) = (state.player_pos, player_query.iter_mut().next()) {
        let scale = 0.09;
        let marker_x = 100.0 + x * scale;
        let marker_y = 100.0 + y * scale;
        marker.left = Val::Px(marker_x - 6.0);
        marker.top = Val::Px(marker_y - 6.0);
    }
}
