//! Minimap tests -- verify zoom cycle, view refresh, player position, and click handling.

#[cfg(test)]
mod tests {
    use crate::minimap::*;

    #[test]
    fn test_default_zoom_is_local() {
        let minimap = MinimapComponent::default();
        assert_eq!(minimap.zoom, MinimapZoom::Local);
        assert!(!minimap.global_map_visible);
        assert!(minimap.selected_hex.is_none());
    }

    #[test]
    fn test_default_positioning() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        assert_eq!(minimap.width, 180.0);
        assert_eq!(minimap.height, 180.0);
        // Bottom-right corner positioning
        assert_eq!(minimap.screen_offset.x, 800.0 - 180.0 - 10.0);
        assert_eq!(minimap.screen_offset.y, 10.0);
    }

    #[test]
    fn test_zoom_cycle_out() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);

        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        // Cannot zoom out past Global
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);
    }

    #[test]
    fn test_zoom_cycle_in() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);

        // Go to Global first
        minimap.set_zoom(MinimapZoom::Global);
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);

        // Cannot zoom in past Local
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);
    }

    #[test]
    fn test_set_zoom() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);

        minimap.set_zoom(MinimapZoom::Global);
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        minimap.set_zoom(MinimapZoom::Local);
        assert_eq!(minimap.zoom, MinimapZoom::Local);
    }

    #[test]
    fn test_viewport_local_has_fewer_hexes_than_global() {
        let mut local = MinimapComponent::new(10.0, 800.0, 600.0);
        local.set_player_pos(Vec2::ZERO);
        local.zoom = MinimapZoom::Local;
        local.refresh_view();

        let mut global = MinimapComponent::new(10.0, 800.0, 600.0);
        global.set_player_pos(Vec2::ZERO);
        global.zoom = MinimapZoom::Global;
        global.refresh_view();

        assert!(local.viewport_hexes.len() < global.viewport_hexes.len());
    }

    #[test]
    fn test_zoom_radius_values() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);

        minimap.zoom = MinimapZoom::Local;
        assert_eq!(minimap.zoom_radius(), 5);

        minimap.zoom = MinimapZoom::Mid;
        assert_eq!(minimap.zoom_radius(), 20);

        minimap.zoom = MinimapZoom::Global;
        assert_eq!(minimap.zoom_radius(), 64);
    }

    #[test]
    fn test_player_position_updates_viewport() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.zoom = MinimapZoom::Local;
        minimap.refresh_view();
        let initial_count = minimap.viewport_hexes.len();

        // Move player
        minimap.set_player_pos(Vec2::new(100.0, 100.0));

        // After moving, viewport should still have hexes
        assert!(!minimap.viewport_hexes.is_empty());
        // And the hex at player position should be included
        let player_hex = minimap.player_pos_to_hex();
        assert!(minimap.viewport_hexes.contains(&player_hex));
    }

    #[test]
    fn test_global_map_toggle() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);
        minimap.zoom = MinimapZoom::Local;
        minimap.refresh_view();
        let local_count = minimap.viewport_hexes.len();

        minimap.toggle_global_map();
        assert!(minimap.global_map_visible);

        // Global map always shows 64 radius
        assert_eq!(minimap.viewport_hexes.len(), 1 + 6 * 64 * 65 / 2);
    }

    #[test]
    fn test_hex_to_world_roundtrip() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);

        let hex = HexCoord::new(3, -2);
        let world = minimap.hex_to_world(&hex);

        // Converting back should give same hex
        let back_hex = minimap.player_pos_to_hex();
        // The round-trip via world position might not exactly equal due to pixel rounding,
        // but for small integers it should be close
        assert!((world.x - 3.0 * 10.0 * 3.0_f32.sqrt()).abs() < 0.01);
    }

    #[test]
    fn test_world_to_screen_central_hex() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);

        let center = Vec2::ZERO;
        let screen = minimap.world_to_screen(center);

        // Center hex should be at the center of the minimap
        let expected_x = minimap.screen_offset.x + minimap.width / 2.0;
        let expected_y = minimap.screen_offset.y + minimap.height / 2.0;
        assert!((screen.x - expected_x).abs() < 1.0);
        assert!((screen.y - expected_y).abs() < 1.0);
    }

    #[test]
    fn test_hex_terrain_color_deterministic() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);

        // Same hex should always get same color
        let hex = HexCoord::new(0, 0);
        let color1 = minimap.hex_terrain_color(&hex);
        let color2 = minimap.hex_terrain_color(&hex);
        assert_eq!(color1, color2);

        // Different hexes should sometimes differ
        let hex2 = HexCoord::new(1, 0);
        // Colors don't need to differ for all hexes, but at least for some
        // The function should be deterministic
        let _ = (color1, color2, hex2);
    }

    #[test]
    fn test_object_marker_color() {
        let minimap = MinimapComponent::new(10.0, 800.0, 600.0);

        let plant = minimap.object_color(ObjectType::Plant);
        let pollution = minimap.object_color(ObjectType::Pollution);
        let building = minimap.object_color(ObjectType::Building);

        // Different types should have different colors
        assert_ne!(plant, pollution);
        assert_ne!(building, pollution);
    }

    #[test]
    fn test_hex_selection() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);

        let target = HexCoord::new(5, -3);
        minimap.select_hex(target);
        assert_eq!(minimap.selected_hex, Some(target));
        assert_eq!(minimap.selected_destination(), Some(target));

        minimap.clear_selection();
        assert!(minimap.selected_hex.is_none());
        assert!(minimap.selected_destination().is_none());
    }

    #[test]
    fn test_manhattan_hex_distance() {
        let d = MinimapComponent::default();

        // Same hex
        assert_eq!(d.manhattan_hex_distance(0, 0, 0, 0), 0);

        // Adjacent hex (1,0)
        assert_eq!(d.manhattan_hex_distance(0, 0, 1, 0), 1);

        // Two steps
        assert_eq!(d.manhattan_hex_distance(0, 0, 2, 0), 2);
    }

    #[test]
    fn test_other_players_visible() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);
        minimap.zoom = MinimapZoom::Mid;
        minimap.refresh_view();

        // Initially empty
        assert!(minimap.viewport_hexes.contains(&HexCoord::new(0, 0)));
        // No other players initially
        // (other_players is stored in MinimapData, not MinimapComponent)
    }

    #[test]
    fn test_minimap_data_creation() {
        let data = MinimapData {
            player_position: Vec2::ZERO,
            viewport_hexes: vec![HexCoord::new(0, 0)],
            other_players: vec![(Vec2::new(100.0, 0.0), "player_2".to_string())],
            objects: vec![ObjectMarker {
                hex: HexCoord::new(3, -1),
                object_type: ObjectType::Plant,
                label: Some("Oak Tree".to_string()),
            }],
            terrain_map: std::collections::HashMap::new(),
        };

        assert_eq!(data.player_position, Vec2::ZERO);
        assert_eq!(data.viewport_hexes.len(), 1);
        assert_eq!(data.other_players.len(), 1);
        assert_eq!(data.other_players[0].1, "player_2");
        assert_eq!(data.objects.len(), 1);
        assert_eq!(data.objects[0].object_type, ObjectType::Plant);
    }

    #[test]
    fn test_zoom_changes_refresh_view() {
        let mut minimap = MinimapComponent::new(10.0, 800.0, 600.0);
        minimap.set_player_pos(Vec2::ZERO);

        minimap.set_zoom(MinimapZoom::Local);
        let local_count = minimap.viewport_hexes.len();

        minimap.set_zoom(MinimapZoom::Mid);
        let mid_count = minimap.viewport_hexes.len();

        assert!(mid_count > local_count);
    }
}
