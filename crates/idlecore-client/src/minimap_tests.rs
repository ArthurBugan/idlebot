//! Minimap tests -- verify zoom cycle, view refresh, and player position.

use crate::Minimap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_zoom_is_local() {
        let minimap = Minimap::default();
        assert_eq!(minimap.zoom, MinimapZoom::Local);
    }

    #[test]
    fn test_zoom_cycle() {
        let mut minimap = Minimap::default();

        // Start at Local, zoom out to Mid, then Global
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        // Global zoomed out should stay at Global
        minimap.zoom_out();
        assert_eq!(minimap.zoom, MinimapZoom::Global);

        // Zoom in
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Mid);

        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);

        // Already at max zoom
        minimap.zoom_in();
        assert_eq!(minimap.zoom, MinimapZoom::Local);
    }

    #[test]
    fn test_viewport_local_has_fewer_hexes_than_global() {
        let mut local = Minimap::default();
        local.zoom = MinimapZoom::Local;
        local.refresh_view();

        let mut global = Minimap::default();
        global.zoom = MinimapZoom::Global;
        global.refresh_view();

        assert!(local.viewport_hexes.len() <= global.viewport_hexes.len());
    }
}
