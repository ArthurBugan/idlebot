//! Minimap rendering — 2D UI overlay showing world hex grid and player position.

use bevy::prelude::*;
use idlecore_core::world::EarthWorld;
use std::collections::HashMap;

/// Resource tracking minimap state
#[derive(Resource, Default)]
pub struct MinimapState {
    /// Player position in world coordinates
    pub player_pos: Option<(f32, f32)>,
    /// View offset (camera position)
    pub view_offset: (f32, f32),
}

/// Marker components
#[derive(Component)]
pub struct MinimapMarker;

#[derive(Component)]
pub struct MinimapContent;

#[derive(Component)]
pub struct PlayerMarker;

/// Spawn minimap UI
pub fn spawn_minimap_ui(mut commands: Commands) {
    commands.spawn((
        Name::new("minimap-ui"),
        MinimapMarker,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            width: Val::Px(250.0),
            height: Val::Px(250.0),
            ..default()
        },
    ))
    .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)))
    .with_children(|parent| {
        // Map content area (will contain hex sprites)
        parent.spawn((
            Name::new("minimap-content"),
            MinimapContent,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(220.0),
                position_type: PositionType::Relative,
                ..default()
            },
        ));
        
        // Player marker (red dot)
        parent.spawn((
            Name::new("player-marker"),
            PlayerMarker,
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                position_type: PositionType::Absolute,
                left: Val::Px(121.0),
                top: Val::Px(121.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.0, 0.0, 0.9)),
        ));
    });
}

/// Sync minimap state with game world
pub fn sync_minimap_state(
    player_query: Query<(&Transform, &super::player::ClientPlayer)>,
    mut minimap_state: ResMut<MinimapState>,
) {
    if let Some((player_transform, _player_data)) = player_query.iter().next() {
        minimap_state.player_pos = Some((
            player_transform.translation.x,
            player_transform.translation.z,
        ));
        minimap_state.view_offset = (
            player_transform.translation.x,
            player_transform.translation.z,
        );
    }
}

/// Marker for hex tile entities
#[derive(Component)]
pub struct HexTileEntity;

/// Render hex tiles on the minimap
pub fn render_hex_tiles(
    world_resource: Res<crate::plugins::world::WorldResource>,
    minimap_state: Res<MinimapState>,
    mut commands: Commands,
    content_query: Query<Entity, With<MinimapContent>>,
    hex_query: Query<Entity, With<HexTileEntity>>,
) {
    let Some(content_entity) = content_query.iter().next() else {
        return;
    };
    
    // Clear existing hex entities
    for entity in hex_query.iter() {
        if let Ok(mut entity_ref) = commands.get_entity(entity) {
            entity_ref.despawn();
        }
    }
    
    let world = &world_resource.world;
    
    // Spawn hex tiles as children of the content node
    commands.entity(content_entity).with_children(|parent| {
        // Render visible hex tiles
        for (_, tile) in &world.tiles {
            // Calculate position relative to camera
            let dx = tile.center_x - minimap_state.view_offset.0;
            let dy = tile.center_y - minimap_state.view_offset.1;
            
            // Convert world units to minimap pixels (scale down)
            let scale = 0.5;
            let screen_x = 100.0 - dx * scale; // Center at 100px
            let screen_y = 100.0 - dy * scale;
            
            // Only render if within minimap bounds
            if screen_x >= 0.0 && screen_x <= 200.0 && screen_y >= 0.0 && screen_y <= 200.0 {
                let color = tile.biome.color();
                let bg_color = Color::srgba(color.0, color.1, color.2, 1.0);
                
                parent.spawn((
                    Name::new(format!("hex-{}", tile.hex_id)),
                    HexTileEntity,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(screen_x - 3.0),
                        top: Val::Px(screen_y - 3.0),
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        ..default()
                    },
                    BackgroundColor(bg_color),
                ));
            }
        }
    });
}

/// Update minimap UI to reflect state
pub fn update_minimap_ui(
    minimap_state: Res<MinimapState>,
    mut player_query: Query<&mut Node, With<PlayerMarker>>,
) {
    // Update player marker position (always centered)
    if let Some(mut player_node) = player_query.iter_mut().next() {
        player_node.position_type = PositionType::Absolute;
        player_node.left = Val::Px(121.0);
        player_node.top = Val::Px(121.0);
    }
}
