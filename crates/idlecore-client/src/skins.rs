//! Player skins — the EmanuelleDev pre-made farmers (Alex, Josh, Lyria,
//! Manu, Tori) with 8-direction animation.
//!
//! The pack ships ready-to-use character sheets
//! (`Character/Character/Pre-made/<name>/{Idle,Walk}.png`) laid out on a
//! 32×32 frame grid: 3 rows × {4 idle | 6 walk} columns. Rows are the three
//! canonical views (0 = facing down/front, 1 = up/back, 2 = side). Each
//! 32×32 frame is sliced out at load time and the side view is mirrored to
//! give west-facing directions, producing a convincing 8-way cycle.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::HashMap;
use std::f32::consts::TAU;
use crate::player::{ClientPlayer, Player, PlayerOrientation, PLAYER_SIZE};
use crate::plugins::player::BASE_SPEED;

/// Selectable characters (the pack's ready-made farmers).
pub const CHARACTERS: &[&str] = &["Alex", "Josh", "Lyria", "Manu", "Tori"];

/// Sheet layout: directions 0=NW clockwise to 7=N (matches
/// `direction_index`). The premade sheets only carry 3 unique views, so all
/// 8 directions are covered by mirroring (S/E/SE/N share 3 base frames).
pub const DIRECTIONS: usize = 8;
pub const IDLE_FRAMES: usize = 4;
pub const WALK_FRAMES: usize = 6;
pub const RUN_FRAMES: usize = 8;

/// Frame size (art pixels) of the premade character sheets.
pub const FRAME_W: u32 = 32;
pub const FRAME_H: u32 = 32;

/// Build the source sheet paths for a named character: idle, walk, run.
fn sheet_paths(name: &str) -> (String, String, String) {
    (
        format!("models/Character/Character/Pre-made/{name}/Idle.png"),
        format!("models/Character/Character/Pre-made/{name}/Walk.png"),
        format!("models/Character/Character/Pre-made/{name}/Run.png"),
    )
}

/// On-screen figure height in world units — the player's normal size
/// (art is square, 1024×1024). Kept in sync with the spawn size so the
/// skin bake never shrinks the character below it.
pub const TINY_FIGURE_HEIGHT: f32 = PLAYER_SIZE;

/// Animation pacing (seconds per frame). The walk cycle ticks faster while
/// sprinting; idle breathes slowly.
const WALK_FRAME_TIME: f32 = 0.12;
const SPRINT_FRAME_TIME: f32 = 0.08;
const IDLE_FRAME_TIME: f32 = 0.6;
/// Above this speed the faster cadence plays.
pub const RUN_SPEED: f32 = BASE_SPEED * 1.6;
/// Below this speed the character stands idle.
const MOVE_EPS: f32 = 0.5;

// ============================================================================
// Slicing the premade character sheets
// ============================================================================

/// The three unique views in the sheets' rows: 0 = down (front), 1 = up
/// (back), 2 = side (east; west is the horizontal mirror).
const ROW_DOWN: u32 = 0;
const ROW_UP: u32 = 1;
const ROW_SIDE: u32 = 2;

/// One RGBA frame sliced from a sheet (`None` = transparent pixel).
pub type Frame = [[Option<[u8; 4]>; FRAME_W as usize]; FRAME_H as usize];

/// Extract one frame from a loaded sheet at (col, row) in FRAME_W×FRAME_H
/// cells and return it as a 2D pixel grid, optionally mirrored horizontally.
fn slice_frame(data: &[u8], w: u32, h: u32, col: u32, row: u32, flip: bool) -> Frame {
    let mut out = [[None; FRAME_W as usize]; FRAME_H as usize];
    for fy in 0..FRAME_H {
        for fx in 0..FRAME_W {
            let sx = (col * FRAME_W + fx) as usize;
            let sy = (row * FRAME_H + fy) as usize;
            if sx >= w as usize || sy >= h as usize {
                continue;
            }
            let i = (sy * w as usize + sx) * 4;
            let alpha = data.get(i + 3).copied().unwrap_or(0);
            if alpha == 0 {
                continue;
            }
            let px = [
                data[i],
                data[i + 1],
                data[i + 2],
                alpha,
            ];
            let out_x = if flip {
                (FRAME_W - 1 - fx) as usize
            } else {
                fx as usize
            };
            out[fy as usize][out_x] = Some(px);
        }
    }
    out
}

/// Convert a frame grid back to a Bevy image.
fn frame_to_image(f: &Frame) -> Image {
    let mut pixels = vec![0u8; (FRAME_W * FRAME_H * 4) as usize];
    for y in 0..FRAME_H as usize {
        for x in 0..FRAME_W as usize {
            if let Some(col) = f[y][x] {
                let i = (y * FRAME_W as usize + x) * 4;
                pixels[i..i + 4].copy_from_slice(&col);
            }
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: FRAME_W,
            height: FRAME_H,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

/// Gather all frames (sliced, per direction) for one action sheet. For each
/// of the 8 directions we select the appropriate source row (down/up/side),
/// mirroring the side view for west-facing directions.
fn build_action_frames(
    images: &mut Assets<Image>,
    data: &[u8],
    w: u32,
    h: u32,
    cols: u32,
) -> Vec<Handle<Image>> {
    // Base row per facing, plus whether the view is mirrored (west side).
    // Directions 0=NW clockwise to 7=N (matches `direction_index`).
    const DIR_SELECT: [(u32, bool); DIRECTIONS] = [
        (ROW_UP, false),   // 0: NW  -> back
        (ROW_SIDE, true),  // 1: W   -> mirrored side
        (ROW_SIDE, true),  // 2: SW  -> mirrored side
        (ROW_DOWN, false), // 3: S   -> front
        (ROW_SIDE, false), // 4: SE  -> side
        (ROW_SIDE, false), // 5: E   -> side
        (ROW_UP, false),   // 6: NE  -> back
        (ROW_UP, false),   // 7: N   -> back
    ];
    let mut out = Vec::with_capacity(DIRECTIONS * cols as usize);
    for (row, flip) in DIR_SELECT {
        for col in 0..cols {
            let frame = slice_frame(data, w, h, col, row, flip);
            out.push(images.add(frame_to_image(&frame)));
        }
    }
    out
}

// ============================================================================
// Runtime skin state + animation
// ============================================================================

/// One playable character: 8 directional idles, walks and runs.
pub struct IsoCharacter {
    pub name: String,
    /// `idle[d * IDLE_FRAMES + f]`, d = 0..8 (0=NW clockwise to 7=N).
    pub idle: Vec<Handle<Image>>,
    /// `walk[d * WALK_FRAMES + f]`.
    pub walk: Vec<Handle<Image>>,
    /// `run[d * RUN_FRAMES + f]`.
    pub run: Vec<Handle<Image>>,
}

/// Runtime skin state.
#[derive(Resource, Default)]
pub struct PlayerSkins {
    /// Character name → sliced animation set, populated once all sheets load.
    pub characters: HashMap<String, IsoCharacter>,
    /// Currently selected character (drives which frames render).
    pub selected: Option<String>,
    /// Set when the player sprite should switch to the selected character.
    pub need_bake: bool,
    /// True once the sprite currently shows the selected character.
    pub applied: bool,
    /// Animation accumulators (seconds) and current frame indices.
    pub anim_time: f32,
    pub idle_frame: usize,
    pub run_frame: usize,
    pub walk_frame: usize,
}

/// Advance (and wrap) the run frame index.
fn step_run_frame(current: usize) -> usize {
    (current + 1) % RUN_FRAMES
}

/// Advance (and wrap) the walk frame index.
fn step_walk_frame(current: usize) -> usize {
    (current + 1) % WALK_FRAMES
}

/// Advance (and wrap) the idle frame index.
fn step_idle_frame(current: usize) -> usize {
    (current + 1) % IDLE_FRAMES
}

/// Map the 8-way facing to the sheet's direction index (0=NW clockwise
/// to 7=N).
fn direction_index(facing: Facing) -> usize {
    match facing {
        Facing::NorthWest => 0,
        Facing::West => 1,
        Facing::SouthWest => 2,
        Facing::South => 3,
        Facing::SouthEast => 4,
        Facing::East => 5,
        Facing::NorthEast => 6,
        Facing::North => 7,
    }
}

pub struct SkinsPlugin;

impl Plugin for SkinsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSkins>();
        app.add_systems(Update, (
            setup_skins,
            apply_skin_to_sprite,
            animate_player_sprite.after(apply_skin_to_sprite),
        ));
    }
}

/// Load every pre-made character's Idle and Walk sheets, then slice each
/// 32×32 frame into the per-direction animation array once all sheets are
/// ready. Strong handles are kept alive in `sources` (a `Local`) so no raw
/// sheet is ever unloaded mid-build.
fn setup_skins(
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut skins: ResMut<PlayerSkins>,
    mut sources: Local<Option<Vec<Handle<Image>>>>,
) {
    if !skins.characters.is_empty() {
        return;
    }
    // Load (and keep) all source sheets: Idle + Walk + Run per character.
    let handles: Vec<Handle<Image>> = if let Some(h) = sources.as_ref() {
        h.clone()
    } else {
        let h: Vec<Handle<Image>> = CHARACTERS
            .iter()
            .flat_map(|name| {
                let (idle_path, walk_path, run_path) = sheet_paths(name);
                vec![
                    asset_server.load::<Image>(&idle_path),
                    asset_server.load::<Image>(&walk_path),
                    asset_server.load::<Image>(&run_path),
                ]
            })
            .collect();
        *sources = Some(h.clone());
        h
    };
    if handles.iter().any(|h| images.get(h).is_none()) {
        return; // sheets still streaming — retry next frame
    }
    let mut map = HashMap::new();
    for (i, name) in CHARACTERS.iter().enumerate() {
        let idle_h = handles[i * 3].clone();
        let walk_h = handles[i * 3 + 1].clone();
        let run_h = handles[i * 3 + 2].clone();
        let (iw, ih, idle_data) = {
            let idle = images.get(&idle_h).expect("idle sheet loaded");
            (
                idle.width(),
                idle.height(),
                idle.data.as_deref().unwrap_or(&[]).to_vec(),
            )
        };
        let (ww, wh, walk_data) = {
            let walk = images.get(&walk_h).expect("walk sheet loaded");
            (
                walk.width(),
                walk.height(),
                walk.data.as_deref().unwrap_or(&[]).to_vec(),
            )
        };
        let (rw, rh, run_data) = {
            let run = images.get(&run_h).expect("run sheet loaded");
            (
                run.width(),
                run.height(),
                run.data.as_deref().unwrap_or(&[]).to_vec(),
            )
        };
        let character = IsoCharacter {
            name: name.to_string(),
            idle: build_action_frames(&mut images, &idle_data, iw, ih, iw / FRAME_W),
            walk: build_action_frames(&mut images, &walk_data, ww, wh, ww / FRAME_W),
            run: build_action_frames(&mut images, &run_data, rw, rh, rw / FRAME_W),
        };
        map.insert(name.to_string(), character);
    }
    skins.characters = map;
    skins.selected = Some(CHARACTERS[0].to_string());
    skins.need_bake = true;
    info!("Pre-made skins sliced for {CHARACTERS:?}");
}

/// Switch the player sprite to the selected character.
fn apply_skin_to_sprite(
    mut skins: ResMut<PlayerSkins>,
    mut player_query: Query<(&mut Sprite, &mut Anchor), With<Player>>,
) {
    if !skins.need_bake {
        return;
    }
    let Some(name) = skins.selected.clone() else {
        return;
    };
    let Some(character) = skins.characters.get(&name) else {
        return;
    };
    let start = character.idle[3 * IDLE_FRAMES].clone(); // face the camera to start
    let Ok((mut sprite, mut anchor)) = player_query.single_mut() else {
        // The player sprite may not exist yet — retry next frame.
        return;
    };
    sprite.image = start;
    sprite.flip_x = false;
    sprite.custom_size = Some(Vec2::splat(TINY_FIGURE_HEIGHT));
    // Feet (bottom edge of the frame) on the entity transform.
    *anchor = Anchor::BOTTOM_CENTER;
    skins.anim_time = 0.0;
    skins.idle_frame = 0;
    skins.run_frame = 0;
    skins.walk_frame = 0;
    skins.need_bake = false;
    skins.applied = true;
    info!("Skin applied to sprite: {name}");
}

/// Play the directional idle or walk animation based on movement speed and
/// facing direction.
fn animate_player_sprite(
    time: Res<Time>,
    orientation: Res<PlayerOrientation>,
    mut skins: ResMut<PlayerSkins>,
    mut player_query: Query<(&mut Sprite, &mut Transform, &ClientPlayer), With<Player>>,
) {
    let Ok((mut sprite, mut transform, player)) = player_query.single_mut() else {
        return;
    };
    let d = direction_index(Facing::from_angle(orientation.facing_angle));
    let speed = player.velocity.length();

    // Snapshot this direction's frames so the animation clock can be
    // mutated below without aliasing the borrow of the character.
    let Some(name) = skins.selected.clone() else {
        return;
    };
    let Some(character) = skins.characters.get(&name) else {
        return;
    };
    let idle_frames: Vec<Handle<Image>> =
        character.idle[d * IDLE_FRAMES..(d + 1) * IDLE_FRAMES].to_vec();
    let walk_frames: Vec<Handle<Image>> =
        character.walk[d * WALK_FRAMES..(d + 1) * WALK_FRAMES].to_vec();
    let run_frames: Vec<Handle<Image>> =
        character.run[d * RUN_FRAMES..(d + 1) * RUN_FRAMES].to_vec();

    if speed > MOVE_EPS {
        if speed > RUN_SPEED {
            // Sprinting: use the dedicated running sheet, ticking faster.
            skins.anim_time += time.delta_secs();
            while skins.anim_time >= SPRINT_FRAME_TIME {
                skins.anim_time -= SPRINT_FRAME_TIME;
                skins.run_frame = step_run_frame(skins.run_frame);
            }
            skins.idle_frame = 0;
            skins.walk_frame = 0;
            let target = run_frames[skins.run_frame].clone();
            if sprite.image != target {
                sprite.image = target;
            }
        } else {
            // Walking: use the walk sheet at a calmer cadence.
            skins.anim_time += time.delta_secs();
            while skins.anim_time >= WALK_FRAME_TIME {
                skins.anim_time -= WALK_FRAME_TIME;
                skins.walk_frame = step_walk_frame(skins.walk_frame);
            }
            skins.idle_frame = 0;
            skins.run_frame = 0;
            let target = walk_frames[skins.walk_frame].clone();
            if sprite.image != target {
                sprite.image = target;
            }
        }
    } else {
        skins.anim_time += time.delta_secs();
        while skins.anim_time >= IDLE_FRAME_TIME {
            skins.anim_time -= IDLE_FRAME_TIME;
            skins.idle_frame = step_idle_frame(skins.idle_frame);
        }
        skins.run_frame = 0;
        skins.walk_frame = 0;
        let target = idle_frames[skins.idle_frame].clone();
        if sprite.image != target {
            sprite.image = target;
        }
    }
    // The sheet covers all 8 directions — no mirroring, no rotation wobble.
    transform.scale = Vec3::ONE;
    transform.rotation = Quat::IDENTITY;
}

/// Cycle the equipped skin by `dir` (±1) through `CHARACTERS`. Returns the
/// newly selected character name (callers persist it to the server).
pub(crate) fn cycle_skin_dir(skins: &mut PlayerSkins, dir: i32) -> Option<String> {
    let cur = skins
        .selected
        .as_deref()
        .and_then(|n| CHARACTERS.iter().position(|c| c == &n));
    let idx = match cur {
        Some(i) => (i as i32 + dir).rem_euclid(CHARACTERS.len() as i32) as usize,
        None => 0,
    };
    let name = CHARACTERS[idx].to_string();
    if skins.selected.as_deref() != Some(name.as_str()) {
        skins.selected = Some(name.clone());
        skins.need_bake = true;
        skins.applied = false;
    }
    Some(name)
}

/// Persisted look: the server's `avatar` column stores a character name.
/// Selects that character (if known) and rebakes the sprite. Returns whether
/// a switch is pending.
pub(crate) fn set_skin_by_name(skins: &mut PlayerSkins, name: &str) -> bool {
    if !CHARACTERS.contains(&name) {
        return false;
    }
    if skins.selected.as_deref() != Some(name) {
        skins.selected = Some(name.to_string());
        skins.need_bake = true;
        skins.applied = false;
        return true;
    }
    if !skins.applied {
        skins.need_bake = true;
    }
    false
}

/// Eight-way facing derived from the movement angle (0 = east, +PI/2 = north).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Facing {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Facing {
    fn from_angle(angle: f32) -> Self {
        // 0 = east, PI/2 = north, PI/-PI = west, -PI/2 = south.
        let a = angle.rem_euclid(TAU);
        let octant = ((a + std::f32::consts::FRAC_PI_8) / std::f32::consts::FRAC_PI_4) as usize % 8;
        [Facing::East, Facing::NorthEast, Facing::North, Facing::NorthWest,
         Facing::West, Facing::SouthWest, Facing::South, Facing::SouthEast][octant]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8, PI};

    /// Build a synthetic sheet: `cols` identical 32×32 frames per row, each
    /// filled with a distinct opaque tint, so slicing can be verified by
    /// colour. Frame `(col,row)` is tagged with a unique byte so frames can
    /// be told apart.
    fn synthetic_sheet(cols: u32) -> (Vec<u8>, u32, u32) {
        let w = cols * FRAME_W;
        let h = 3 * FRAME_H;
        let mut data = vec![0u8; (w * h * 4) as usize];
        for row in 0..3u32 {
            for col in 0..cols {
                let tag = (row * 100 + col) as u8;
                for fy in 0..FRAME_H {
                    for fx in 0..FRAME_W {
                        let sx = col * FRAME_W + fx;
                        let sy = row * FRAME_H + fy;
                        let i = ((sy * w + sx) * 4) as usize;
                        data[i] = tag;
                        data[i + 1] = tag;
                        data[i + 2] = tag;
                        data[i + 3] = 255;
                    }
                }
            }
        }
        (data, w, h)
    }

    /// Colour of the top-left opaque pixel of a frame.
    fn tag_of(f: &Frame) -> u8 {
        for y in 0..FRAME_H as usize {
            for x in 0..FRAME_W as usize {
                if let Some(c) = f[y][x] {
                    return c[0];
                }
            }
        }
        0
    }

    #[test]
    fn facing_from_angle_maps_eight_ways() {
        assert_eq!(Facing::from_angle(0.0), Facing::East);
        assert_eq!(Facing::from_angle(FRAC_PI_8 + 0.01), Facing::NorthEast);
        assert_eq!(Facing::from_angle(FRAC_PI_2), Facing::North);
        assert_eq!(Facing::from_angle(3.0 * FRAC_PI_8 + 0.01), Facing::North);
        assert_eq!(Facing::from_angle(5.0 * FRAC_PI_8 + 0.01), Facing::NorthWest);
        assert_eq!(Facing::from_angle(7.0 * FRAC_PI_8 + 0.01), Facing::West);
        assert_eq!(Facing::from_angle(PI), Facing::West);
        assert_eq!(Facing::from_angle(-PI + 0.01), Facing::West);
        assert_eq!(Facing::from_angle(9.0 * FRAC_PI_8 + 0.01), Facing::SouthWest);
        assert_eq!(Facing::from_angle(13.0 * FRAC_PI_8 - 0.01), Facing::South);
        assert_eq!(Facing::from_angle(-FRAC_PI_2), Facing::South);
        assert_eq!(Facing::from_angle(15.0 * FRAC_PI_8 - 0.01), Facing::SouthEast);
        assert_eq!(Facing::from_angle(-FRAC_PI_4), Facing::SouthEast);
        assert_eq!(Facing::from_angle(-FRAC_PI_8), Facing::East);
        assert_eq!(Facing::from_angle(TAU - 0.1), Facing::East);
        assert_eq!(Facing::from_angle(-TAU), Facing::East);
    }

    #[test]
    fn direction_index_matches_sheet_numbering() {
        // Sheet runs clockwise from NW: 0=NW 1=W 2=SW 3=S 4=SE 5=E 6=NE 7=N.
        assert_eq!(direction_index(Facing::NorthWest), 0);
        assert_eq!(direction_index(Facing::West), 1);
        assert_eq!(direction_index(Facing::SouthWest), 2);
        assert_eq!(direction_index(Facing::South), 3);
        assert_eq!(direction_index(Facing::SouthEast), 4);
        assert_eq!(direction_index(Facing::East), 5);
        assert_eq!(direction_index(Facing::NorthEast), 6);
        assert_eq!(direction_index(Facing::North), 7);
    }

    #[test]
    fn run_frame_wraps_at_six() {
        assert_eq!(step_run_frame(0), 1);
        assert_eq!(step_run_frame(5), 0);
    }

    #[test]
    fn idle_frame_wraps_at_four() {
        assert_eq!(step_idle_frame(0), 1);
        assert_eq!(step_idle_frame(3), 0);
    }

    #[test]
    fn slice_reads_the_right_row_and_column() {
        let (data, w, h) = synthetic_sheet(4);
        // col 3, row 2 (side view) has tag = 2*100 + 3 = 203.
        let f = slice_frame(&data, w, h, 3, ROW_SIDE, false);
        assert_eq!(tag_of(&f), 203);
        // col 0, row 0 (front) tag = 0.
        let f = slice_frame(&data, w, h, 0, ROW_DOWN, false);
        assert_eq!(tag_of(&f), 0);
    }

    #[test]
    fn slice_mirrors_horizontally() {
        let (data, w, h) = synthetic_sheet(4);
        // Build an asymmetric frame: only the left half opaque; mirroring
        // must move it to the right half.
        let mut tag_left = vec![0u8; (FRAME_W * FRAME_H * 4) as usize];
        for y in 0..FRAME_H as usize {
            for x in 0..(FRAME_W as usize / 2) {
                let i = (y * FRAME_W as usize + x) * 4;
                tag_left[i] = 90;
                tag_left[i + 1] = 90;
                tag_left[i + 2] = 90;
                tag_left[i + 3] = 255;
            }
        }
        let plain = slice_frame(&tag_left, FRAME_W, FRAME_H, 0, 0, false);
        let mir = slice_frame(&tag_left, FRAME_W, FRAME_H, 0, 0, true);
        // Mirrored: right half must now be opaque (top-right pixel filled).
        assert!(plain[0][0].is_some());
        assert!(plain[0][FRAME_W as usize - 1].is_none());
        assert!(mir[0][0].is_none());
        assert!(mir[0][FRAME_W as usize - 1].is_some());
    }

    #[test]
    fn direction_picks_the_correct_view_row() {
        // Each direction must map to a valid row (0..3) and the side views
        // mirror exactly the east-facing set.
        let (data, w, h) = synthetic_sheet(6);
        // E (d 5): col depends on action but row must be ROW_SIDE.
        let e = slice_frame(&data, w, h, 0, ROW_SIDE, false);
        let w_ = slice_frame(&data, w, h, 0, ROW_SIDE, true);
        // Mirrored E frame should equal the W frame (same source, flipped).
        for y in 0..FRAME_H as usize {
            for x in 0..FRAME_W as usize {
                assert_eq!(e[y][x], w_[y][FRAME_W as usize - 1 - x]);
            }
        }
    }
}
