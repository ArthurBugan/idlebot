//! Tiny* pixel-art helpers — chroma-keying for tiles with baked backdrops.
//!
//! Kenney's Tiny* character/prop tiles ship with an opaque dark backdrop
//! blob (RGB 63,38,49) around the sprite plus pre-cut transparent corners.
//! [`key_background_flood`] flood-fills from the tile borders through both,
//! so it can never eat interior sprite pixels.

use bevy::prelude::*;

/// Kenney Tiny* baked backdrop color.
pub(crate) const BACKDROP: (u8, u8, u8) = (63, 38, 49);

/// Handles awaiting background key-out once their assets load.
#[derive(Resource, Default)]
pub struct TinyKeyQueue(pub Vec<Handle<Image>>);

/// Key out every queued tile whose asset has loaded; retries the rest.
pub fn process_key_queue(mut images: ResMut<Assets<Image>>, mut queue: ResMut<TinyKeyQueue>) {
    let mut remaining = Vec::new();
    for handle in queue.0.drain(..) {
        let loaded = match images.get_mut(&handle) {
            Some(mut image) => {
                let (w, h) = (image.width() as usize, image.height() as usize);
                match image.data.as_mut() {
                    Some(data) => {
                        key_background_flood(data, w, h);
                        true
                    }
                    None => false,
                }
            }
            None => false,
        };
        if !loaded {
            remaining.push(handle);
        }
    }
    queue.0 = remaining;
}

/// Flood-fill transparency from the tile borders: clears pre-cut transparent
/// pixels and the baked backdrop ring, never interior sprite pixels.
pub(crate) fn key_background_flood(data: &mut [u8], w: usize, h: usize) {
    const TOL: f32 = 30.0;
    let idx = |x: usize, y: usize| (y * w + x) * 4;
    let mut visited = vec![false; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let p = y * w + x;
        if visited[p] {
            continue;
        }
        visited[p] = true;
        let i = idx(x, y);
        let transparent = data[i + 3] == 0;
        let backdrop = (data[i] as f32 - BACKDROP.0 as f32).abs() <= TOL
            && (data[i + 1] as f32 - BACKDROP.1 as f32).abs() <= TOL
            && (data[i + 2] as f32 - BACKDROP.2 as f32).abs() <= TOL;
        if !transparent && !backdrop {
            continue;
        }
        data[i + 3] = 0;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
}
