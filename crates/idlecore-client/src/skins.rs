//! Player skin — a genuine Tiny* mini character with 8-direction animation.
//!
//! The base sprite is a real Tiny Dungeon character tile
//! (`models/Tiny Dungeon/Tiles/tile_0085.png`, front-facing 16×16). The
//! packs only ship that single pose, so the 8-direction × {idle, walk}
//! frames are derived from it at load time by pixel surgery: eye offsets
//! turn the face, a hair recolor builds the back view, a 1px body bob plus
//! leg shuffles animate the cycle. West-side directions are mirrors of the
//! east-side ones (S, SE, E, NE, N are derived; SW/W/NW flipped).

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use std::f32::consts::TAU;
use crate::player::{ClientPlayer, Player, PlayerOrientation};
use crate::plugins::player::BASE_SPEED;

/// Selectable characters (single Tiny Dungeon mini today).
pub const CHARACTERS: &[&str] = &["Tiny Villager"];

/// Source tile: Tiny Dungeon villager (front-facing, 16×16).
const SOURCE_TILE: &str = "models/Tiny Dungeon/Tiles/tile_0085.png";

/// Sheet layout: directions 0=NW clockwise to 7=N (matches
/// `direction_index`), 2 idle frames and 4 walk frames per direction.
pub const DIRECTIONS: usize = 8;
pub const IDLE_FRAMES: usize = 2;
pub const RUN_FRAMES: usize = 4;

/// Frame size (art pixels).
pub const FRAME_W: u32 = 16;
pub const FRAME_H: u32 = 16;

/// On-screen figure height in world units (matches the Tiny prop scale).
pub const TINY_FIGURE_HEIGHT: f32 = 3.4;

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
// Pixel surgery on the source tile
// ============================================================================

/// Tiny Dungeon palette entries we manipulate.
const EYES: [u8; 4] = [38, 43, 68, 255];
const SKIN: [u8; 4] = [247, 194, 130, 255];
const SKIN_SHADE: [u8; 4] = [225, 154, 101, 255];
const HAIR: [u8; 4] = [189, 108, 74, 255];
const HAIR_DARK: [u8; 4] = [118, 59, 54, 255];

/// One 16×16 frame under construction; `None` = transparent.
pub type Canvas = [[Option<[u8; 4]>; 16]; 16];

fn put(c: &mut Canvas, x: i32, y: i32, col: [u8; 4]) {
    if (0..16).contains(&x) && (0..16).contains(&y) {
        c[y as usize][x as usize] = Some(col);
    }
}

fn get(c: &Canvas, x: i32, y: i32) -> Option<[u8; 4]> {
    if (0..16).contains(&x) && (0..16).contains(&y) {
        c[y as usize][x as usize]
    } else {
        None
    }
}

/// Mirror horizontally (east view → west view).
fn mirrored(c: &Canvas) -> Canvas {
    let mut out = [[None; 16]; 16];
    for y in 0..16 {
        for x in 0..16 {
            out[y][x] = c[y][15 - x];
        }
    }
    out
}

fn same_color(a: Option<[u8; 4]>, b: [u8; 4]) -> bool {
    a.map(|c| c == b).unwrap_or(false)
}

/// Convert the loaded source tile into a working canvas.
fn canvas_from_image(img: &Image) -> Canvas {
    let mut c = [[None; 16]; 16];
    let data = img.data.as_deref().unwrap_or(&[]);
    for y in 0..16usize {
        for x in 0..16usize {
            let i = (y * 16 + x) * 4;
            if i + 3 < data.len() && data[i + 3] > 0 {
                c[y][x] = Some([
                    data[i],
                    data[i + 1],
                    data[i + 2],
                    data[i + 3],
                ]);
            }
        }
    }
    c
}

/// Convert a canvas to a Bevy image.
fn canvas_to_image(c: &Canvas) -> Image {
    let mut pixels = vec![0u8; (FRAME_W * FRAME_H * 4) as usize];
    for y in 0..16 {
        for x in 0..16 {
            if let Some(col) = c[y][x] {
                let i = ((y * 16 + x) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&col);
            }
        }
    }
    Image::new(
        Extent3d { width: FRAME_W, height: FRAME_H, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Shift every eye pixel by `dx` (turning the face toward a direction).
fn shift_eyes(c: &mut Canvas, dx: i32) {
    if dx == 0 {
        return;
    }
    let mut moved = Vec::new();
    for y in 0..16i32 {
        for x in 0..16i32 {
            if same_color(get(c, x, y), EYES) {
                moved.push((x, y));
            }
        }
    }
    for (x, y) in moved {
        c[y as usize][x as usize] = None;
        put(c, x + dx, y, EYES);
    }
}

/// Drop the far-side eye and nudge the near one forward (side profile).
fn side_profile(c: &mut Canvas) {
    let mut moved = Vec::new();
    for y in 0..16i32 {
        for x in 0..16i32 {
            if same_color(get(c, x, y), EYES) {
                moved.push((x, y));
            }
        }
    }
    let mut first = true;
    for (x, y) in moved {
        c[y as usize][x as usize] = None;
        if first {
            first = false;
            continue; // far eye disappears in profile
        }
        put(c, x + 1, y, EYES);
    }
}

/// Back view: the face becomes hair (head rows only — hands stay skin).
/// Eyes map to the same hair color as the skin, otherwise the two dark
/// spots survive as "eyes on the back of the head".
fn back_view(c: &mut Canvas) {
    for y in 0..10usize {
        for x in 0..16usize {
            match c[y][x] {
                Some(col) if col == SKIN || col == SKIN_SHADE || col == EYES => {
                    c[y][x] = Some(HAIR)
                }
                _ => {}
            }
        }
    }
}

/// Body bob: head+torso drop 1px, legs stay planted.
fn bobbed(c: &Canvas) -> Canvas {
    let mut out = *c;
    for y in (1..14).rev() {
        out[y] = c[y - 1];
    }
    out
}

/// Leg shuffle: pants/feet colors in the bottom rows spread (`±1`) or close
/// for the stride, inside the static outline frame.
fn leg_shuffle(c: &Canvas, left_dx: i32, right_dx: i32) -> Canvas {
    let mut out = *c;
    for y in 14..16usize {
        for x in 0..16usize {
            if let Some(col) = c[y][x] {
                if col == HAIR || col == HAIR_DARK {
                    out[y][x] = None;
                }
            }
        }
    }
    for y in 14..16usize {
        for x in 0..16usize {
            if let Some(col) = c[y][x] {
                if col == HAIR || col == HAIR_DARK {
                    let dx = if x < 8 { left_dx } else { right_dx };
                    put(&mut out, x as i32 + dx, y as i32, col);
                }
            }
        }
    }
    out
}

/// Build the full sheet from the source tile: `idle[d * IDLE_FRAMES + f]`,
/// `run[d * RUN_FRAMES + f]`, directions 0=NW clockwise to 7=N (west side
/// mirrored from east).
pub fn derive_sheet(base: &Canvas) -> (Vec<Canvas>, Vec<Canvas>) {
    // (view, eye_dx) per direction; east views get mirrored into west ones.
    // Anything up-ish (NE/N/NW) shows the back of the head — the front view
    // with shifted eyes read as "eyes on the back of the head" while running
    // diagonally up.
    const DIR_VIEW: [(u8, i32, bool); DIRECTIONS] = [
        (4, 0, true),  // 0: NW  <- back (mirrored; symmetric art)
        (2, 0, true),  // 1: W   <- mirror of E
        (1, 1, true),  // 2: SW  <- mirror of SE
        (0, 0, false), // 3: S
        (1, 1, false), // 4: SE
        (2, 0, false), // 5: E
        (4, 0, false), // 6: NE  <- back
        (4, 0, false), // 7: N
    ];
    let view_canvas = |view: u8| -> Canvas {
        let mut c = *base;
        match view {
            1 => shift_eyes(&mut c, 1),   // SE
            2 => side_profile(&mut c),    // E
            3 => shift_eyes(&mut c, 1),   // NE
            4 => back_view(&mut c),       // N
            _ => {}
        }
        c
    };
    let mut idle = Vec::with_capacity(DIRECTIONS * IDLE_FRAMES);
    let mut run = Vec::with_capacity(DIRECTIONS * RUN_FRAMES);
    for (view, _, flip) in DIR_VIEW {
        let front = view_canvas(view);
        // Idle: stand + breathe.
        idle.push(if flip { mirrored(&front) } else { front });
        idle.push(if flip { mirrored(&bobbed(&front)) } else { bobbed(&front) });
        // Walk: stand, spread, stand, together — stride frames bob.
        let spread = bobbed(&leg_shuffle(&front, -1, 1));
        let together = bobbed(&leg_shuffle(&front, 1, -1));
        for stride in [&front, &spread, &front, &together] {
            run.push(if flip { mirrored(stride) } else { *stride });
        }
    }
    (idle, run)
}

// ============================================================================
// Runtime skin state + animation
// ============================================================================

/// One playable character: 8 directional idles + 8×4 walk frames.
pub struct IsoCharacter {
    pub name: String,
    /// `idle[d * IDLE_FRAMES + f]`, d = 0..8 (0=NW clockwise to 7=N).
    pub idle: Vec<Handle<Image>>,
    /// `run[d * RUN_FRAMES + f]`.
    pub run: Vec<Handle<Image>>,
}

/// Runtime skin state.
#[derive(Resource, Default)]
pub struct PlayerSkins {
    /// Some once the generated sheet has been built.
    pub character: Option<IsoCharacter>,
    /// Set when the player sprite should switch to the current character.
    pub need_bake: bool,
    /// True once the sprite currently shows the selected character.
    pub applied: bool,
    /// Source tile handle while it streams in.
    pub source: Option<Handle<Image>>,
    /// Animation accumulators (seconds) and current frame indices.
    pub anim_time: f32,
    pub idle_frame: usize,
    pub run_frame: usize,
}

/// Advance (and wrap) the walk frame index.
fn step_run_frame(current: usize) -> usize {
    (current + 1) % RUN_FRAMES
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

/// Load the Tiny Dungeon mini, then derive the animation sheet from it once
/// the texture has streamed in.
fn setup_skins(
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut skins: ResMut<PlayerSkins>,
) {
    if skins.character.is_some() {
        return;
    }
    let handle = skins
        .source
        .get_or_insert_with(|| asset_server.load::<Image>(SOURCE_TILE));
    let Some(src) = images.get(handle) else {
        return; // tile still streaming — retry next frame
    };
    let base = canvas_from_image(src);
    let (idle_canvases, run_canvases) = derive_sheet(&base);
    let character = IsoCharacter {
        name: CHARACTERS[0].to_string(),
        idle: idle_canvases.iter().map(|c| images.add(canvas_to_image(c))).collect(),
        run: run_canvases.iter().map(|c| images.add(canvas_to_image(c))).collect(),
    };
    skins.character = Some(character);
    skins.need_bake = true;
    info!("Tiny mini skin derived from {SOURCE_TILE}");
}

/// Switch the player sprite to the selected character.
fn apply_skin_to_sprite(
    mut skins: ResMut<PlayerSkins>,
    mut player_query: Query<(&mut Sprite, &mut Anchor), With<Player>>,
) {
    if !skins.need_bake {
        return;
    }
    let Some(character) = skins.character.as_ref() else {
        return;
    };
    let name = character.name.clone();
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
    let Some(character) = skins.character.as_ref() else {
        return;
    };
    let idle_frames: Vec<Handle<Image>> =
        character.idle[d * IDLE_FRAMES..(d + 1) * IDLE_FRAMES].to_vec();
    let run_frames: Vec<Handle<Image>> =
        character.run[d * RUN_FRAMES..(d + 1) * RUN_FRAMES].to_vec();

    if speed > MOVE_EPS {
        let frame_time = if speed > RUN_SPEED {
            SPRINT_FRAME_TIME
        } else {
            WALK_FRAME_TIME
        };
        // Carry the sub-frame remainder so cadence stays exact at any fps.
        skins.anim_time += time.delta_secs();
        while skins.anim_time >= frame_time {
            skins.anim_time -= frame_time;
            skins.run_frame = step_run_frame(skins.run_frame);
        }
        skins.idle_frame = 0;
        let target = run_frames[skins.run_frame].clone();
        if sprite.image != target {
            sprite.image = target;
        }
    } else {
        skins.anim_time += time.delta_secs();
        while skins.anim_time >= IDLE_FRAME_TIME {
            skins.anim_time -= IDLE_FRAME_TIME;
            skins.idle_frame = step_idle_frame(skins.idle_frame);
        }
        skins.run_frame = 0;
        let target = idle_frames[skins.idle_frame].clone();
        if sprite.image != target {
            sprite.image = target;
        }
    }
    // The sheet covers all 8 directions — no mirroring, no rotation wobble.
    transform.scale = Vec3::ONE;
    transform.rotation = Quat::IDENTITY;
}

/// [ and ] cycle the equipped skin (single character today — kept so the
/// keys and the Avatar button stay wired).
pub(crate) fn cycle_skin_dir(skins: &mut PlayerSkins, _dir: i32) {
    if skins.character.is_some() {
        skins.need_bake = true;
        skins.applied = false;
    }
}

/// Persisted look: whatever the server stores renders as the pixel hero
/// until more characters are added.
pub(crate) fn set_skin_by_name(skins: &mut PlayerSkins, _name: &str) -> bool {
    if skins.character.is_none() {
        return false;
    }
    if !skins.applied {
        skins.need_bake = true;
    }
    true
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

    /// A tiny synthetic "source tile": skin face with two eyes on a hair
    /// block and legs at the bottom — enough structure for the surgery.
    fn fake_base() -> Canvas {
        let mut c = [[None; 16]; 16];
        for x in 4..=11 {
            put(&mut c, x, 2, HAIR);
            put(&mut c, x, 3, HAIR);
        }
        for x in 5..=10 {
            put(&mut c, x, 4, SKIN);
            put(&mut c, x, 5, SKIN);
        }
        put(&mut c, 6, 5, EYES);
        put(&mut c, 9, 5, EYES);
        for x in 5..=10 {
            put(&mut c, x, 6, SKIN_SHADE);
        }
        for x in 5..=6 {
            put(&mut c, x, 14, HAIR_DARK);
            put(&mut c, x, 15, HAIR);
        }
        for x in 9..=10 {
            put(&mut c, x, 14, HAIR_DARK);
            put(&mut c, x, 15, HAIR);
        }
        c
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
    fn run_frame_wraps_at_four() {
        assert_eq!(step_run_frame(0), 1);
        assert_eq!(step_run_frame(3), 0);
    }

    #[test]
    fn idle_frame_wraps_at_two() {
        assert_eq!(step_idle_frame(0), 1);
        assert_eq!(step_idle_frame(1), 0);
    }

    #[test]
    fn sheet_has_every_direction_and_frame() {
        let (idle, run) = derive_sheet(&fake_base());
        assert_eq!(idle.len(), DIRECTIONS * IDLE_FRAMES);
        assert_eq!(run.len(), DIRECTIONS * RUN_FRAMES);
        for c in idle.iter().chain(run.iter()) {
            let solid = c.iter().flatten().filter(|p| p.is_some()).count();
            assert!(solid > 8, "frame is (almost) empty");
        }
    }

    #[test]
    fn west_frames_mirror_east_frames() {
        let flip = |c: &Canvas| -> Canvas { mirrored(c) };
        let (idle, run) = derive_sheet(&fake_base());
        // E is direction 5, W is 1; NE=6/NW=0 and SE=4/SW=2 likewise.
        for (east, west) in [(5usize, 1usize), (6, 0), (4, 2)] {
            for f in 0..RUN_FRAMES {
                assert_eq!(
                    flip(&run[east * RUN_FRAMES + f]),
                    run[west * RUN_FRAMES + f],
                    "run d{west} f{f}"
                );
            }
            for f in 0..IDLE_FRAMES {
                assert_eq!(
                    flip(&idle[east * IDLE_FRAMES + f]),
                    idle[west * IDLE_FRAMES + f],
                    "idle d{west} f{f}"
                );
            }
        }
    }

    #[test]
    fn walk_frames_differ_from_stand() {
        let (_, run) = derive_sheet(&fake_base());
        let south = 3 * RUN_FRAMES;
        assert_ne!(run[south], run[south + 1], "stride != stand");
        assert_ne!(run[south + 1], run[south + 3], "left stride != right stride");
    }

    #[test]
    fn eyes_track_the_direction() {
        let base = fake_base();
        let (idle, _) = derive_sheet(&base);
        let eye_cols = |c: &Canvas| -> Vec<i32> {
            let mut v: Vec<i32> = (0..16i32)
                .filter(|&x| same_color(c[5][x as usize], EYES))
                .collect();
            v.sort();
            v
        };
        // S: eyes at 6/9. SE: shifted +1. E: far eye gone, near eye +1.
        assert_eq!(eye_cols(&idle[3 * IDLE_FRAMES]), vec![6, 9]);
        assert_eq!(eye_cols(&idle[4 * IDLE_FRAMES]), vec![7, 10]);
        assert_eq!(eye_cols(&idle[5 * IDLE_FRAMES]), vec![10]);
        // N/NE/NW (back views): no eyes at all — hair recolor.
        assert!(eye_cols(&idle[7 * IDLE_FRAMES]).is_empty(), "N");
        assert!(eye_cols(&idle[6 * IDLE_FRAMES]).is_empty(), "NE");
        assert!(eye_cols(&idle[0 * IDLE_FRAMES]).is_empty(), "NW");
    }

    #[test]
    fn back_view_keeps_hands_skin() {
        let mut c = fake_base();
        put(&mut c, 3, 12, SKIN); // a hand below the head rows
        back_view(&mut c);
        assert_eq!(c[12][3], Some(SKIN), "hands must not turn to hair");
        assert_eq!(c[5][5], Some(HAIR), "face skin must become hair");
        assert_eq!(c[5][9], Some(HAIR), "eyes must vanish into the hair");
    }

    #[test]
    fn bob_moves_the_head_not_the_feet() {
        let base = fake_base();
        let bob = bobbed(&base);
        assert_eq!(bob[3], base[2], "torso shifts down");
        assert_eq!(bob[14], base[14], "legs stay planted");
    }

    #[test]
    #[ignore = "visual review helper: cargo test dump_sheet -- --ignored"]
    fn dump_sheet() {
        // Prefer the real Tiny Dungeon tile; fall back to the synthetic one.
        let base = image::open("assets/models/Tiny Dungeon/Tiles/tile_0085.png")
            .ok()
            .map(|img| img.to_rgba8())
            .map(|rgba| {
                let img = Image::new(
                    Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    rgba.into_raw(),
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                canvas_from_image(&img)
            })
            .unwrap_or_else(fake_base);
        let (idle, run) = derive_sheet(&base);
        let cols = IDLE_FRAMES + RUN_FRAMES;
        let mut sheet = image::RgbaImage::new(DIRECTIONS as u32 * 17, 2 * cols as u32 * 17);
        for (row, frames) in [(0usize, &idle), (1usize, &run)] {
            for d in 0..DIRECTIONS {
                for f in 0..frames.len() / DIRECTIONS {
                    let c = &frames[d * (frames.len() / DIRECTIONS) + f];
                    for y in 0..16usize {
                        for x in 0..16usize {
                            let p = c[y][x].unwrap_or([0, 0, 0, 0]);
                            sheet.put_pixel(
                                (d as u32 * 17) + x as u32,
                                (row as u32 * cols as u32 * 17) + (f as u32 * 17) + y as u32,
                                image::Rgba(p),
                            );
                        }
                    }
                }
            }
        }
        let big = image::imageops::resize(
            &sheet, sheet.width() * 5, sheet.height() * 5, image::imageops::FilterType::Nearest,
        );
        image::DynamicImage::ImageRgba8(big)
            .save("/var/folders/nc/rcc9k90n2m17088ggj431hnm0000gn/T/opencode/mini_sheet.png")
            .unwrap();
    }
}
