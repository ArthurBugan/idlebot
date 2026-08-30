//! Async sprite cropping for the EmanuelleDev asset pack.
//!
//! The new art ships as premade transparent sprites and larger atlas sheets,
//! so unlike the old Kenney Tiny* packs there is **nothing to chroma-key**.
//! Instead, individual sprites are cropped out of their source sheet once it
//! has streamed in and registered as standalone [`Image`] handles.
//!
//! A request names a source sheet and a pixel [`CropRect`]. Each frame the
//! slicer loads whatever sheets are still pending, crops any that are ready,
//! and stashes the resulting handles in [`SlicedAtlas`] under a caller-chosen
//! key. Consumers (`PropTextures`, `DecoTextures`, `SolidFloorTextures`,
//! `WaterTextures`) wait until the keys they need exist, then build.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Pixel region of a sheet to crop out as a sprite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl CropRect {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
}

/// One pending slice request: a sheet path + rect + the atlas key assigned
/// to the result.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    key: String,
    path: &'static str,
    rect: CropRect,
    handle: Handle<Image>,
}

/// Cropped sprite handles, keyed by the request's key.
#[derive(Resource, Default)]
pub struct SlicedAtlas {
    pub items: std::collections::HashMap<String, Handle<Image>>,
}

/// Accumulates slice requests; drained into the loader each frame.
#[derive(Resource, Default)]
pub struct SliceRequests(pub Vec<Pending>);

/// Convenience: build a slice request for a sheet+rect.
pub fn request(
    requests: &mut SliceRequests,
    asset_server: &AssetServer,
    key: &str,
    path: &'static str,
    rect: CropRect,
) {
    let handle = asset_server.load::<Image>(path);
    requests.0.push(Pending { key: key.to_string(), path, rect, handle });
}

/// Pump the pending requests: load sheets, crop ready ones, move results to
/// the atlas (retrying anything still streaming).
pub fn pump_slices(
    mut images: ResMut<Assets<Image>>,
    mut requests: ResMut<SliceRequests>,
    mut atlas: ResMut<SlicedAtlas>,
) {
    let mut still_pending: Vec<Pending> = Vec::new();
    for req in requests.0.drain(..) {
        let Some(src) = images.get(&req.handle) else {
            still_pending.push(req);
            continue;
        };
        let (sw, sh) = (src.width(), src.height());
        let x = req.rect.x.min(sw);
        let y = req.rect.y.min(sh);
        let w = req.rect.w.min(sw - x);
        let h = req.rect.h.min(sh - y);
        let data = src.data.as_deref().unwrap_or(&[]);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for py in 0..h {
            for px in 0..w {
                let sx = (x + px) as usize;
                let sy = (y + py) as usize;
                let si = (sy * sw as usize + sx) * 4;
                let di = (py * w + px) as usize * 4;
                pixels[di..di + 4]
                    .copy_from_slice(&data.get(si..si + 4).unwrap_or(&[0, 0, 0, 0]));
            }
        }
        let crop = Image::new(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        atlas.items.insert(req.key, images.add(crop));
    }
    requests.0 = still_pending;
}
