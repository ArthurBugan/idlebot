//! Player skins — the EmanuelleDev pre-made farmers (Alex, Josh, Lyria,
//! Manu, Tori) with 8-direction animation.
//!
//! The pack ships ready-to-use character sheets
//! (`Character/Character/Pre-made/<name>/{Idle,Walk}.png`) laid out on a
//! 32×32 frame grid: 3 rows × {4 idle | 6 walk} columns. Rows are the three
//! canonical views (0 = facing down/front, 1 = up/back, 2 = side). Each
//! 32×32 frame is sliced out at load time and the side view is mirrored to
//! give west-facing directions, producing a convincing 8-way cycle.
//! Spec 030: the same pre-made folder ships attack sheets — `Sword.png`
//! (10 columns/frame view) and `Bow and Arrow.png` (7) — used for the
//! one-shot strike animation whenever the player attacks.

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
/// Attack sheets (Spec 030–033): per frame-view column counts.
pub const SWORD_FRAMES: usize = 10;
pub const BOW_FRAMES: usize = 7;
pub const AXE_FRAMES: usize = 6;
pub const SICKLE_FRAMES: usize = 6;
pub const PICKAXE_FRAMES: usize = 6;
pub const HOE_FRAMES: usize = 6;
pub const SHOVEL_FRAMES: usize = 5;
/// REV (playtest): the Fists punch reuses `Throwing items.png` — a bare-arm
/// whiff reads as a jab (5 columns per direction row).
pub const FISTS_FRAMES: usize = 5;
/// One-shot attack clip cadence (10 sword frames ≈ 0.70 s, 7 bow ≈ 0.49 s).
pub const ATTACK_FRAME_TIME: f32 = 0.07;

/// Frame size (art pixels) of the premade character sheets.
pub const FRAME_W: u32 = 32;
pub const FRAME_H: u32 = 32;

/// Build the source sheet paths for a named character: idle, walk, run and
/// the attack sheets (+ the Fists punch sheet, REV playtest), then the two
/// Horse mount sheets (`Horse/Horse idle|run.png`).
fn sheet_paths(name: &str) -> [String; 13] {
    let base = format!("models/Character/Character/Pre-made/{name}/");
    [
        format!("{base}Idle.png"),
        format!("{base}Walk.png"),
        format!("{base}Run.png"),
        format!("{base}Sword.png"),
        format!("{base}Bow and Arrow.png"),
        format!("{base}Axe.png"),
        format!("{base}Sickle.png"),
        format!("{base}Pickaxe.png"),
        format!("{base}Hoe.png"),
        format!("{base}Shovel.png"),
        format!("{base}Throwing items.png"),
        format!("{base}Horse/Horse idle.png"),
        format!("{base}Horse/Horse run.png"),
    ]
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
/// Horse mount sheets (48×48 art): gallop cadence and idle breath.
const HORSE_RUN_FRAME_TIME: f32 = 0.09;
const HORSE_IDLE_FRAME_TIME: f32 = 0.4;
/// Horse sheet layout: 32×48 frames (one pixel-per-unit row taller than the
/// character sheets). Idle packs all three views in one row (2 frames per
/// view: front / back / side); run is 6 columns × 3 view rows (down / up /
/// side like the character sheets).
pub const HORSE_FRAME_W: u32 = 32;
pub const HORSE_FRAME_H: u32 = 48;
pub const HORSE_IDLE_FRAMES: usize = 2;
pub const HORSE_RUN_FRAMES: usize = 6;
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

/// Base row per facing, plus whether the view is mirrored (west side).
/// Directions 0=NW clockwise to 7=N (matches `direction_index`).
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

/// One RGBA frame sliced from a sheet (`None` = transparent pixel).
pub type Frame = [[Option<[u8; 4]>; FRAME_W as usize]; FRAME_H as usize];

/// Variable-size RGBA frame for the Horse sheets (48×48).
type SizedFrame = Vec<Vec<Option<[u8; 4]>>>;

/// Extract one frame from a loaded sheet at (col, row) in fw×fh cells and
/// return it as a 2D pixel grid, optionally mirrored horizontally.
fn slice_frame_at(
    data: &[u8],
    w: u32,
    h: u32,
    fw: u32,
    fh: u32,
    col: u32,
    row: u32,
    flip: bool,
) -> SizedFrame {
    let mut out = vec![vec![None; fw as usize]; fh as usize];
    for fy in 0..fh {
        for fx in 0..fw {
            let sx = (col * fw + fx) as usize;
            let sy = (row * fh + fy) as usize;
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
                (fw - 1 - fx) as usize
            } else {
                fx as usize
            };
            out[fy as usize][out_x] = Some(px);
        }
    }
    out
}

/// Convert a variable-size frame grid back to a Bevy image.
fn frame_to_image_at(f: &SizedFrame, fw: u32, fh: u32) -> Image {
    let mut pixels = vec![0u8; (fw * fh * 4) as usize];
    for y in 0..fh as usize {
        for x in 0..fw as usize {
            if let Some(col) = f[y][x] {
                let i = (y * fw as usize + x) * 4;
                pixels[i..i + 4].copy_from_slice(&col);
            }
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: fw,
            height: fh,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

/// Extract one frame from a loaded sheet at (col, row) in FRAME_W×FRAME_H
/// cells and return it as a 2D pixel grid, optionally mirrored horizontally.
fn slice_frame(data: &[u8], w: u32, h: u32, col: u32, row: u32, flip: bool) -> Frame {
    slice_frame_at(data, w, h, FRAME_W, FRAME_H, col, row, flip)
        .iter()
        .map(|row| row.clone().try_into().expect("frame width stays 32"))
        .collect::<Vec<_>>()
        .try_into()
        .expect("frame height stays 32")
}

/// Convert a frame grid back to a Bevy image.
fn frame_to_image(f: &Frame) -> Image {
    let f: SizedFrame = f.iter().map(|row| row.to_vec()).collect();
    frame_to_image_at(&f, FRAME_W, FRAME_H)
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
    build_action_frames_at(images, data, w, h, FRAME_W, FRAME_H, cols)
}

/// Variable frame-size builder (the Horse sheets run 48×48).
fn build_action_frames_at(
    images: &mut Assets<Image>,
    data: &[u8],
    w: u32,
    h: u32,
    fw: u32,
    fh: u32,
    cols: u32,
) -> Vec<Handle<Image>> {
    let mut out = Vec::with_capacity(DIRECTIONS * cols as usize);
    for (row, flip) in DIR_SELECT {
        for col in 0..cols {
            let frame = slice_frame_at(data, w, h, fw, fh, col, row, flip);
            out.push(images.add(frame_to_image_at(&frame, fw, fh)));
        }
    }
    out
}

/// `Horse idle.png`: one 32×48 row packing all three views — columns 0–1
/// front, 2–3 back, 4–5 side (east-facing; west mirrors) — 2 frames per
/// view, sliced into the same 8-direction layout as the character sheets.
fn build_horse_idle_frames(
    images: &mut Assets<Image>,
    data: &[u8],
    w: u32,
    h: u32,
) -> Vec<Handle<Image>> {
    // View row → the idle sheet's two-column group that carries that view.
    let group = |row: u32| match row {
        ROW_DOWN => 0u32,  // front  = cols 0–1
        ROW_UP => 2u32,    // back   = cols 2–3
        _ => 4u32,         // side   = cols 4–5
    };
    let mut out = Vec::with_capacity(DIRECTIONS * HORSE_IDLE_FRAMES);
    for (row, flip) in DIR_SELECT {
        let base = group(row);
        for f in 0..HORSE_IDLE_FRAMES as u32 {
            let frame = slice_frame_at(
                data,
                w,
                h,
                HORSE_FRAME_W,
                HORSE_FRAME_H,
                base + f,
                0,
                flip,
            );
            out.push(images.add(frame_to_image_at(&frame, HORSE_FRAME_W, HORSE_FRAME_H)));
        }
    }
    out
}

/// `Horse run.png`: 6 columns × 3 view rows (down / up / side), sliced in
/// the same 8-direction layout as the character sheets.
fn build_horse_run_frames(
    images: &mut Assets<Image>,
    data: &[u8],
    w: u32,
    h: u32,
) -> Vec<Handle<Image>> {
    build_action_frames_at(
        images,
        data,
        w,
        h,
        HORSE_FRAME_W,
        HORSE_FRAME_H,
        HORSE_RUN_FRAMES as u32,
    )
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
    /// Sword strike clip: `sword[d * SWORD_FRAMES + f]` (Spec 030).
    pub sword: Vec<Handle<Image>>,
    /// Bow draw+loose clip: `bow[d * BOW_FRAMES + f]` (Spec 030).
    pub bow: Vec<Handle<Image>>,
    /// Axe chop clip: `axe[d * AXE_FRAMES + f]` (Spec 031).
    pub axe: Vec<Handle<Image>>,
    /// Sickle hook clip: `sickle[d * SICKLE_FRAMES + f]` (Spec 031).
    pub sickle: Vec<Handle<Image>>,
    /// Pickaxe stab clip: `pickaxe[d * PICKAXE_FRAMES + f]` (Spec 033).
    pub pickaxe: Vec<Handle<Image>>,
    /// Hoe sweep clip: `hoe[d * HOE_FRAMES + f]` (Spec 033).
    pub hoe: Vec<Handle<Image>>,
    /// Shovel lob clip: `shovel[d * SHOVEL_FRAMES + f]` (Spec 033).
    pub shovel: Vec<Handle<Image>>,
    /// REV (playtest): Fists jab clip: `fists[d * FISTS_FRAMES + f]` — the
    /// `Throwing items` sheets, arm-forward whiff.
    pub fists: Vec<Handle<Image>>,
    /// Horse mount idle (front view, 4 frames × 8 directions).
    pub horse_idle: Vec<Handle<Image>>,
    /// Horse mount gallop (`horse_run[d * HORSE_RUN_FRAMES + f]`).
    pub horse_run: Vec<Handle<Image>>,
}

/// One-shot attack clip selector (Spec 031/033) — which sheet a strike plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackAnim {
    Sword,
    Bow,
    Axe,
    Sickle,
    Pickaxe,
    Hoe,
    Shovel,
    /// REV (playtest): bare-fist jab (`Throwing items` sheet).
    Fists,
}

impl AttackAnim {
    /// Sheet choice for an equipped tier (Spec 033): the core tier carries
    /// its animation; REV: Fists play their own punch, not the sword slash.
    pub fn of_weapon(core_anim: idlecore_core::player_combat::WeaponAnim) -> Self {
        use idlecore_core::player_combat::WeaponAnim as W;
        match core_anim {
            W::Sword => AttackAnim::Sword,
            W::Bow => AttackAnim::Bow,
            W::Axe => AttackAnim::Axe,
            W::Sickle => AttackAnim::Sickle,
            W::Pickaxe => AttackAnim::Pickaxe,
            W::Hoe => AttackAnim::Hoe,
            W::Shovel => AttackAnim::Shovel,
            W::Fists => AttackAnim::Fists,
        }
    }

    fn frames(self) -> usize {
        match self {
            AttackAnim::Sword => SWORD_FRAMES,
            AttackAnim::Bow => BOW_FRAMES,
            AttackAnim::Axe => AXE_FRAMES,
            AttackAnim::Sickle => SICKLE_FRAMES,
            AttackAnim::Pickaxe => PICKAXE_FRAMES,
            AttackAnim::Hoe => HOE_FRAMES,
            AttackAnim::Shovel => SHOVEL_FRAMES,
            AttackAnim::Fists => FISTS_FRAMES,
        }
    }
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
    /// Spec 030: one-shot attack clip — `Some(anim)` while playing, with
    /// its elapsed seconds. Any frame advancing past the last column ends
    /// the clip and falls back to idle/walk.
    pub attack: Option<AttackAnim>,
    pub attack_time: f32,
    /// Spec 034 C3 (HLD lunge): set alongside a fresh `attack` start, then
    /// consumed by `player_movement` as a single velocity impulse toward the
    /// strike — the exp-ease recovers it inside the clip, so nothing has to
    /// reverse it by hand.
    pub lunge_pending: bool,
    /// Horse mount state: when true the sprite plays the 48×48 ride sheets.
    /// Movement/attack timing keeps running so mount/unmount transitions
    /// reset the gallop clock exactly once.
    pub mount: bool,
    pub mount_time: f32,
    pub mount_frame: usize,
    pub mount_run_frame: usize,
}

/// On-screen size of the mounted 32×48 ride frame, world units. The same
/// pixel scale as the 32×32 character sheet describes the wider ride: 1 wu
/// per art pixel, so the frame is the character width × 1.5× the height.
pub fn mount_sprite_size() -> Vec2 {
    Vec2::new(TINY_FIGURE_HEIGHT, TINY_FIGURE_HEIGHT * HORSE_FRAME_H as f32 / FRAME_H as f32)
}

/// Attack-freeze rule: while a swing clip plays the player does not walk —
/// movement resumes when the animation ends.
pub fn movement_locked(skins: &PlayerSkins) -> bool {
    skins.attack.is_some()
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
    // Load (and keep) all source sheets: Idle + Walk + Run + the four
    // attack sheets per character.
    let handles: Vec<Handle<Image>> = if let Some(h) = sources.as_ref() {
        h.clone()
    } else {
        let h: Vec<Handle<Image>> = CHARACTERS
            .iter()
            .flat_map(|name| {
                sheet_paths(name).map(|p| asset_server.load::<Image>(&p))
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
        let h: &Vec<Handle<Image>> = &handles;
        let base = i * 13;
        let sheet = |idx: usize| -> (u32, u32, Vec<u8>) {
            let img = images.get(&h[base + idx]).expect(
                "attack sheet loaded",
            );
            (
                img.width(),
                img.height(),
                img.data.as_deref().unwrap_or(&[]).to_vec(),
            )
        };
        let (iw, ih, idle_data) = sheet(0);
        let (ww, wh, walk_data) = sheet(1);
        let (rw, rh, run_data) = sheet(2);
        let (sw, sh, sword_data) = sheet(3);
        let (bw, bh, bow_data) = sheet(4);
        let (xw, xh, axe_data) = sheet(5);
        let (kw, kh, sickle_data) = sheet(6);
        let (pw, ph, pickaxe_data) = sheet(7);
        let (ow, oh, hoe_data) = sheet(8);
        let (vw, vh, shovel_data) = sheet(9);
        let (fw, fh, fists_data) = sheet(10);
        let (hiw, hih, horse_idle_data) = sheet(11);
        let (hrw, hrh, horse_run_data) = sheet(12);
        let character = IsoCharacter {
            name: name.to_string(),
            idle: build_action_frames(&mut images, &idle_data, iw, ih, iw / FRAME_W),
            walk: build_action_frames(&mut images, &walk_data, ww, wh, ww / FRAME_W),
            run: build_action_frames(&mut images, &run_data, rw, rh, rw / FRAME_W),
            sword: build_action_frames(&mut images, &sword_data, sw, sh, sw / FRAME_W),
            bow: build_action_frames(&mut images, &bow_data, bw, bh, bw / FRAME_W),
            axe: build_action_frames(&mut images, &axe_data, xw, xh, xw / FRAME_W),
            sickle: build_action_frames(&mut images, &sickle_data, kw, kh, kw / FRAME_W),
            pickaxe: build_action_frames(&mut images, &pickaxe_data, pw, ph, pw / FRAME_W),
            hoe: build_action_frames(&mut images, &hoe_data, ow, oh, ow / FRAME_W),
            shovel: build_action_frames(&mut images, &shovel_data, vw, vh, vw / FRAME_W),
            fists: build_action_frames(&mut images, &fists_data, fw, fh, fw / FRAME_W),
            horse_idle: build_horse_idle_frames(&mut images, &horse_idle_data, hiw, hih),
            horse_run: build_horse_run_frames(&mut images, &horse_run_data, hrw, hrh),
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
    sprite.custom_size = Some(if skins.mount {
        mount_sprite_size()
    } else {
        Vec2::splat(TINY_FIGURE_HEIGHT)
    });
    // Feet (bottom edge of the frame) on the entity transform.
    *anchor = Anchor::BOTTOM_CENTER;
    skins.anim_time = 0.0;
    skins.idle_frame = 0;
    skins.run_frame = 0;
    skins.walk_frame = 0;
    skins.attack = None;
    skins.attack_time = 0.0;
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

    // Horse mount: an equipped Horse swaps the sprite to the 32×48 ride
    // sheets in one transition, then animates gallop/idle like the feet.
    let mounted = matches!(
        player.owned_vehicle,
        Some(idlecore_core::Vehicle::Bicycle)
    );
    if skins.mount != mounted {
        skins.mount = mounted;
        skins.anim_time = 0.0;
        skins.mount_time = 0.0;
        skins.mount_frame = 0;
        skins.mount_run_frame = 0;
        skins.idle_frame = 0;
        skins.run_frame = 0;
        skins.walk_frame = 0;
        sprite.custom_size = Some(if mounted {
            mount_sprite_size()
        } else {
            Vec2::splat(TINY_FIGURE_HEIGHT)
        });
    }

    // Snapshot this direction's frames so the animation clock can be
    // mutated below without aliasing the borrow of the character.
    let Some(name) = skins.selected.clone() else {
        return;
    };
    let Some(character) = skins.characters.get(&name) else {
        return;
    };

    if skins.mount {
        let run_frames: Vec<Handle<Image>> =
            character.horse_run[d * HORSE_RUN_FRAMES..(d + 1) * HORSE_RUN_FRAMES].to_vec();
        let idle_frames: Vec<Handle<Image>> =
            character.horse_idle[d * HORSE_IDLE_FRAMES..(d + 1) * HORSE_IDLE_FRAMES].to_vec();
        // Attack freeze: keep ticking the one-shot clip (the movement lock
        // reads it) but the sprite holds the standing mount — the pack ships
        // no mounted attack sheets, so the rider stands the swing out.
        if skins.attack.is_some() {
            let anim = skins.attack.unwrap_or(AttackAnim::Sword);
            let t = skins.attack_time + time.delta_secs();
            if (t / ATTACK_FRAME_TIME) as usize >= anim.frames() {
                skins.attack = None; // clip finished
                skins.attack_time = 0.0;
            } else {
                skins.attack_time = t;
            }
            let target = idle_frames[0].clone();
            if sprite.image != target {
                sprite.image = target;
            }
            transform.scale = Vec3::ONE;
            transform.rotation = Quat::IDENTITY;
            return;
        }
        if speed > MOVE_EPS {
            let mut t = skins.mount_time + time.delta_secs();
            while t >= HORSE_RUN_FRAME_TIME {
                t -= HORSE_RUN_FRAME_TIME;
                skins.mount_run_frame = (skins.mount_run_frame + 1) % HORSE_RUN_FRAMES;
            }
            skins.mount_time = t;
            let target = run_frames[skins.mount_run_frame].clone();
            if sprite.image != target {
                sprite.image = target;
            }
        } else {
            let mut t = skins.mount_time + time.delta_secs();
            while t >= HORSE_IDLE_FRAME_TIME {
                t -= HORSE_IDLE_FRAME_TIME;
                skins.mount_frame = (skins.mount_frame + 1) % HORSE_IDLE_FRAMES;
            }
            skins.mount_time = t;
            let target = idle_frames[skins.mount_frame].clone();
            if sprite.image != target {
                sprite.image = target;
            }
        }
        // The sheet covers all 8 directions — no mirroring, no rotation wobble.
        transform.scale = Vec3::ONE;
        transform.rotation = Quat::IDENTITY;
        return;
    }

    let idle_frames: Vec<Handle<Image>> =
        character.idle[d * IDLE_FRAMES..(d + 1) * IDLE_FRAMES].to_vec();
    let walk_frames: Vec<Handle<Image>> =
        character.walk[d * WALK_FRAMES..(d + 1) * WALK_FRAMES].to_vec();
    let run_frames: Vec<Handle<Image>> =
        character.run[d * RUN_FRAMES..(d + 1) * RUN_FRAMES].to_vec();

    // Spec 030: the one-shot attack clip outranks idle/walk/run. It is NOT
    // looping — once past the last column it clears and normal animation
    // resumes next frame.
    if skins.attack.is_some() {
        // Borrow split: read the clip's sheet len through the map, then
        // mutate the clock freely.
        let anim = skins.attack.unwrap_or(AttackAnim::Sword);
        let frame_count = anim.frames();
        let within = {
            let attack = match anim {
                AttackAnim::Sword => &character.sword,
                AttackAnim::Bow => &character.bow,
                AttackAnim::Axe => &character.axe,
                AttackAnim::Sickle => &character.sickle,
                AttackAnim::Pickaxe => &character.pickaxe,
                AttackAnim::Hoe => &character.hoe,
                AttackAnim::Shovel => &character.shovel,
                AttackAnim::Fists => &character.fists,
            };
            attack.len() / DIRECTIONS
        };
        let t = skins.attack_time + time.delta_secs();
        let idx = (t / ATTACK_FRAME_TIME) as usize;
        if idx >= within {
            skins.attack = None; // clip finished
        } else {
            let base = d * frame_count;
            let idx = idx.min(frame_count - 1);
            let target = {
                let attack = match anim {
                    AttackAnim::Sword => &character.sword,
                    AttackAnim::Bow => &character.bow,
                    AttackAnim::Axe => &character.axe,
                    AttackAnim::Sickle => &character.sickle,
                    AttackAnim::Pickaxe => &character.pickaxe,
                    AttackAnim::Hoe => &character.hoe,
                    AttackAnim::Shovel => &character.shovel,
                    AttackAnim::Fists => &character.fists,
                };
                attack[base + idx].clone()
            };
            skins.attack_time = t;
            if sprite.image != target {
                sprite.image = target;
            }
            transform.scale = Vec3::ONE;
            transform.rotation = Quat::IDENTITY;
            return;
        }
    }


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
pub(crate) fn cycle_skin_dir(skins: &mut PlayerSkins, dir: i32) -> Option<String> {    let cur = skins
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

/// Fire the one-shot attack animation (Spec 030/031). `anim` picks the
/// sheet. Safe to call repeatedly — a retrigger restarts the clip from
/// frame 0.
pub fn start_attack(skins: &mut PlayerSkins, anim: AttackAnim) {
    if skins.attack != Some(anim) || skins.attack_time != 0.0 {
        skins.attack = Some(anim);
        skins.attack_time = 0.0;
        // Spec 034 C3: every fresh swing arms a lunge impulse (HLD step.
        // Player movement consumes + disarms it once).
        skins.lunge_pending = true;
    }
}

/// Spec 034 C3: lunge impulse size (u/s) — an exp-ease at the movement
/// controller's rate covers ≈ 0.2 tile forward inside the recovery, then
/// falls back under the commanded speed. Pure so the feel is unit-testable.
pub const LUNGE_IMPULSE: f32 = 6.0;

/// True when a lunge impulse should fire this frame (a fresh swing started
/// since the last movement step).
pub fn consume_lunge(skins: &mut PlayerSkins) -> bool {
    if skins.lunge_pending {
        skins.lunge_pending = false;
        return true;
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
    fn run_frame_wraps_at_eight() {
        // RUN_FRAMES = 8: advance at 7 wraps to 0, never mid-run.
        assert_eq!(step_run_frame(0), 1);
        assert_eq!(step_run_frame(7), 0);
    }

    #[test]
    fn movement_locks_while_a_swing_plays() {
        // Attack-freeze rule: the clip gates walking/dashing until it ends.
        let mut skins = PlayerSkins::default();
        assert!(!movement_locked(&skins));
        skins.attack = Some(AttackAnim::Sword);
        assert!(movement_locked(&skins));
        skins.attack = None;
        assert!(!movement_locked(&skins));
    }

    #[test]
    fn mount_frame_is_taller_not_wider_than_the_character() {
        // 32×48 ride art over a 32×32 character sheet keeps the pixel scale:
        // same world width, 1.5× the world height.
        let size = mount_sprite_size();
        assert_eq!(size.x, TINY_FIGURE_HEIGHT);
        assert_eq!(size.y, TINY_FIGURE_HEIGHT * 1.5);
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
