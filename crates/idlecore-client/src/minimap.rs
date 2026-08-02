//! Minimap rendering — 2D UI overlay showing world hex grid and player position.

use bevy::prelude::*;
use idlecore_core::world::EarthWorld;

/// Resource tracking minimap state
#[derive(Resource, Default)]
pub struct MinimapState {
    /// Player position in world coordinates
    pub player_pos: Option<(f32, f32)>,
    /// View offset (camera position)
    pub view_offset: (f32, f32),
    /// Player direction (angle in radians)
    pub player_angle: f32,
}

/// Marker components
#[derive(Component)]
pub struct MinimapMarker;

#[derive(Component)]
pub struct MinimapContent;

#[derive(Component)]
pub struct PlayerMarker;

#[derive(Component)]
pub struct CompassMarker;

#[derive(Component)]
pub struct CoordsMarker;

/// Spawn minimap UI
pub fn spawn_minimap_ui(mut commands: Commands) {
    commands.spawn((
        Name::new("minimap-ui"),
        MinimapMarker,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            width: Val::Px(280.0),
            height: Val::Px(300.0),
            ..default()
        },
    ))
    .insert(BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.9)))
    .with_children(|parent| {
        // Map content area (will contain hex sprites)
        parent.spawn((
            Name::new("minimap-content"),
            MinimapContent,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(240.0),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.15, 0.2, 1.0)),
        ));
        
        // Compass (North indicator)
        parent.spawn((
            Name::new("compass"),
            CompassMarker,
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                ..default()
            },
        )).with_child(Text::new("N"));
        
        // Coordinates display
        parent.spawn((
            Name::new("coords"),
            CoordsMarker,
            Node {
                width: Val::Px(120.0),
                height: Val::Px(16.0),
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(5.0),
                ..default()
            },
        )).with_child((
            Text::default(),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgba(0.8, 0.8, 1.0, 1.0)),
        )).with_child(TextSpan::new("0, 0"));
        
        // Player marker (directional arrow)
        parent.spawn((
            Name::new("player-marker"),
            PlayerMarker,
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                position_type: PositionType::Absolute,
                left: Val::Px(134.0),
                top: Val::Px(134.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.2, 0.2, 1.0)),
        ));
    });
}

/// Sync minimap state with game world
pub fn sync_minimap_state(
    player_query: Query<(&Transform, &super::player::ClientPlayer)>,
    mut minimap_state: ResMut<MinimapState>,
) {
    if let Some((player_transform, player_data)) = player_query.iter().next() {
        minimap_state.player_pos = Some((
            player_transform.translation.x,
            player_transform.translation.z,
        ));
        minimap_state.view_offset = (
            player_transform.translation.x,
            player_transform.translation.z,
        );
        // Calculate player direction from velocity
        if player_data.velocity.length() > 0.01 {
            minimap_state.player_angle = player_data.velocity.y.atan2(player_data.velocity.x);
        }
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
            let scale = 0.8;
            let screen_x = 120.0 - dx * scale; // Center at 120px (half of 240)
            let screen_y = 120.0 - dy * scale;
            
            // Only render if within minimap bounds (with margin)
            let margin = 10.0;
            if screen_x >= -margin && screen_x <= 240.0 + margin 
                && screen_y >= -margin && screen_y <= 240.0 + margin 
            {
                let color = tile.biome.color();
                let bg_color = Color::srgba(color.0, color.1, color.2, 0.9);
                
                // Hex tile size based on scale
                let hex_size = 8.0;
                
                parent.spawn((
                    Name::new(format!("hex-{}", tile.hex_id)),
                    HexTileEntity,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(screen_x - hex_size / 2.0),
                        top: Val::Px(screen_y - hex_size / 2.0),
                        width: Val::Px(hex_size),
                        height: Val::Px(hex_size),
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
    mut coords_query: Query<&mut TextSpan, With<CoordsMarker>>,
) {
    // Update player marker rotation based on direction
    if let Some(mut player_node) = player_query.iter_mut().next() {
        player_node.position_type = PositionType::Absolute;
        player_node.left = Val::Px(134.0);
        player_node.top = Val::Px(134.0);
    }
    
    // Update coordinates display
    if let Some((x, y)) = minimap_state.player_pos {
        if let Some(mut coords) = coords_query.iter_mut().next() {
            **coords = format!("{:.0}, {:.0}", x, y);
        }
    }
}
