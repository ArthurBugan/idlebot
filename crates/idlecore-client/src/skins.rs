//! Player skins — 2D character art from `assets/models/Toon Characters/`.
//!
//! Each character ships pose sheets (`PNG/Poses/character_{key}_{pose}.png`).
//! We load the idle, walk and run sheets and the player sprite animates
//! between them based on movement speed. Cycle characters with [ and ].

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_4, FRAC_PI_8, TAU};
use crate::player::{ClientPlayer, Player, PlayerOrientation, PLAYER_SIZE};
use crate::plugins::player::BASE_SPEED;

/// Characters in `assets/models/Toon Characters/` (kept in sync with the
/// folder). Pose sheets are 96x128 px.
pub const TOON_CHARACTERS: &[&str] = &[
    "Female adventurer",
    "Female person",
    "Male adventurer",
    "Male person",
    "Robot",
    "Zombie",
];

/// Walk/run animation pacing.
const WALK_FRAME_TIME: f32 = 0.09;
const RUN_FRAME_TIME: f32 = 0.13;
/// Above this speed the run cycle plays instead of the walk cycle.
const RUN_SPEED: f32 = BASE_SPEED * 1.6;
/// Below this speed the character stands idle.
const MOVE_EPS: f32 = 0.5;

/// Camel-case pose key for a character folder name
/// ("Female adventurer" -> "femaleAdventurer").
fn pose_key(name: &str) -> String {
    name.split_whitespace()
        .enumerate()
        .map(|(i, word)| {
            if i == 0 {
                word.to_ascii_lowercase()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + chars.as_str()
                    }
                    None => String::new(),
                }
            }
        })
        .collect()
}

/// Relative asset path for a character pose sheet.
fn pose_path(name: &str, pose: &str) -> String {
    format!(
        "models/Toon Characters/{name}/PNG/Poses/character_{}_{pose}.png",
        pose_key(name)
    )
}

/// One selectable character with its animation pose sheets.
pub struct ToonCharacter {
    pub name: String,
    pub idle: Handle<Image>,
    /// Facing north (away from camera).
    pub back: Handle<Image>,
    /// Facing east/west (mirrored with `Sprite::flip_x` for west).
    pub side: Handle<Image>,
    pub walk: Vec<Handle<Image>>,
    pub run: Vec<Handle<Image>>,
}

/// Runtime skin state: loaded characters plus the currently selected one.
#[derive(Resource)]
pub struct PlayerSkins {
    pub characters: Vec<ToonCharacter>,
    pub current: usize,
    /// Set when the player sprite should switch to the current character.
    pub need_bake: bool,
    /// Animation accumulator (seconds) and current frame indices.
    pub anim_time: f32,
    pub walk_frame: usize,
    pub run_frame: usize,
}

impl Default for PlayerSkins {
    fn default() -> Self {
        Self {
            characters: Vec::new(),
            current: 0,
            need_bake: false,
            anim_time: 0.0,
            walk_frame: 0,
            run_frame: 0,
        }
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
            cycle_skin,
        ));
    }
}

/// One-time setup: load idle, walk and run pose sheets for every character.
fn setup_skins(
    asset_server: Res<AssetServer>,
    mut skins: ResMut<PlayerSkins>,
) {
    if !skins.characters.is_empty() {
        return;
    }

    let mut characters = Vec::with_capacity(TOON_CHARACTERS.len());
    for name in TOON_CHARACTERS {
        let walk = (0..8)
            .map(|i| asset_server.load::<Image>(pose_path(name, &format!("walk{i}"))))
            .collect();
        let run = (0..3)
            .map(|i| asset_server.load::<Image>(pose_path(name, &format!("run{i}"))))
            .collect();
        characters.push(ToonCharacter {
            name: name.to_string(),
            idle: asset_server.load::<Image>(pose_path(name, "idle")),
            back: asset_server.load::<Image>(pose_path(name, "back")),
            side: asset_server.load::<Image>(pose_path(name, "side")),
            walk,
            run,
        });
    }

    skins.characters = characters;
    skins.current = 0;
    skins.need_bake = true;
}

/// Eight-way facing derived from the movement angle (0 = east, +PI/2 = north).
/// Diagonals share a pose with their cardinal neighbour (back for the north
/// arc, front for the south arc, side for east/west).
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
        match angle.rem_euclid(TAU) {
            a if a < FRAC_PI_8 || a >= TAU - FRAC_PI_8 => Facing::East,
            a if a < 3.0 * FRAC_PI_8 => Facing::NorthEast,
            a if a < 5.0 * FRAC_PI_8 => Facing::North,
            a if a < 7.0 * FRAC_PI_8 => Facing::NorthWest,
            a if a < 9.0 * FRAC_PI_8 => Facing::West,
            a if a < 11.0 * FRAC_PI_8 => Facing::SouthWest,
            a if a < 13.0 * FRAC_PI_8 => Facing::South,
            _ => Facing::SouthEast,
        }
    }
}

/// Switch the player sprite to the current character's idle pose.
fn apply_skin_to_sprite(
    mut skins: ResMut<PlayerSkins>,
    mut player_query: Query<&mut Sprite, With<Player>>,
) {
    if !skins.need_bake {
        return;
    }
    let (name, idle) = {
        let Some(character) = skins.characters.get(skins.current) else {
            return;
        };
        (character.name.clone(), character.idle.clone())
    };
    let Ok(mut sprite) = player_query.single_mut() else {
        // The player sprite may not exist yet — retry next frame.
        return;
    };
    sprite.image = idle;
    sprite.flip_x = false;
    // Pose art is 96x128 (3:4); keep the player's height, scale width to match.
    sprite.custom_size = Some(Vec2::new(PLAYER_SIZE * 96.0 / 128.0, PLAYER_SIZE));
    skins.anim_time = 0.0;
    skins.walk_frame = 0;
    skins.run_frame = 0;
    skins.need_bake = false;
    info!("Skin applied to sprite: {name}");
}

/// Pick the pose for the current facing: `back` for the north arc
/// (N/NE/NW), `side` for east/west (mirrored for west) or the front idle for
/// the south arc (S/SE/SW).
fn facing_pose(character: &ToonCharacter, facing: Facing) -> Handle<Image> {
    match facing {
        Facing::North | Facing::NorthEast | Facing::NorthWest => character.back.clone(),
        Facing::East | Facing::West => character.side.clone(),
        Facing::South | Facing::SouthEast | Facing::SouthWest => character.idle.clone(),
    }
}

/// Cycle walk/run/idle poses on the player sprite based on movement speed and
/// facing direction.
fn animate_player_sprite(
    time: Res<Time>,
    orientation: Res<PlayerOrientation>,
    mut skins: ResMut<PlayerSkins>,
    mut player_query: Query<(&mut Sprite, &ClientPlayer), With<Player>>,
) {
    let Ok((mut sprite, player)) = player_query.single_mut() else {
        return;
    };
    let (idle, walk_len, run_len) = {
        let Some(character) = skins.characters.get(skins.current) else {
            return;
        };
        (facing_pose(character, Facing::from_angle(orientation.facing_angle)), character.walk.len(), character.run.len())
    };
    sprite.flip_x = Facing::from_angle(orientation.facing_angle) == Facing::West;

    let speed = player.velocity.length();
    let target: Handle<Image> = if speed > RUN_SPEED {
        if run_len == 0 {
            idle
        } else {
            skins.anim_time += time.delta_secs();
            if skins.anim_time >= RUN_FRAME_TIME {
                skins.anim_time = 0.0;
                skins.run_frame = (skins.run_frame + 1) % run_len;
            }
            skins.characters[skins.current].run[skins.run_frame].clone()
        }
    } else if speed > MOVE_EPS {
        if walk_len == 0 {
            idle
        } else {
            skins.anim_time += time.delta_secs();
            if skins.anim_time >= WALK_FRAME_TIME {
                skins.anim_time = 0.0;
                skins.walk_frame = (skins.walk_frame + 1) % walk_len;
            }
            skins.characters[skins.current].walk[skins.walk_frame].clone()
        }
    } else {
        skins.anim_time = 0.0;
        idle
    };

    if sprite.image != target {
        sprite.image = target;
    }
}

/// [ and ] cycle the equipped skin shown on the model.
fn cycle_skin(
    keys: Res<ButtonInput<KeyCode>>,
    mut skins: ResMut<PlayerSkins>,
) {
    let Some(dir) = (if keys.just_pressed(KeyCode::BracketRight) {
        Some(1)
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        Some(-1)
    } else {
        None
    }) else {
        return;
    };
    cycle_skin_dir(&mut skins, dir);
}

/// Advance the current skin by `dir` (±1, wrapping) and flag a re-bake.
/// Shared by the [ / ] keys and the HUD "Avatar -> Next" button.
pub(crate) fn cycle_skin_dir(skins: &mut PlayerSkins, dir: isize) {
    if skins.characters.is_empty() {
        return;
    }
    skins.current = (skins.current as isize + dir).rem_euclid(skins.characters.len() as isize) as usize;
    skins.need_bake = true;
    info!("Skin: {}", skins.characters[skins.current].name);
}

/// Select the skin by character name (e.g. restored from the persisted avatar
/// column). Returns true if a matching skin was applied.
pub(crate) fn set_skin_by_name(skins: &mut PlayerSkins, name: &str) -> bool {
    if skins.characters.is_empty() {
        return false;
    }
    let Some(idx) = skins
        .characters
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
        .or_else(|| map_legacy_skin(name).and_then(|n| skins.characters.iter().position(|c| c.name.eq_ignore_ascii_case(n))))
    else {
        return false;
    };
    if idx != skins.current {
        skins.current = idx;
        skins.need_bake = true;
        info!("Skin restored: {}", skins.characters[idx].name);
    }
    true
}

/// Best-effort mapping from the old `assets/skins/*.png` file names to a
/// Toon Characters character, so persisted avatar rows keep working.
fn map_legacy_skin(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("zombie") {
        Some("Zombie")
    } else if lower.contains("robot") || lower.contains("cyborg") || lower.contains("alien") {
        Some("Robot")
    } else if lower.contains("female") {
        Some("Female adventurer")
    } else if lower.contains("male") {
        Some("Male adventurer")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_8, PI};

    fn test_character(name: &str) -> ToonCharacter {
        ToonCharacter {
            name: name.to_string(),
            idle: Handle::default(),
            back: Handle::default(),
            side: Handle::default(),
            walk: vec![Handle::default(); 8],
            run: vec![Handle::default(); 3],
        }
    }

    fn skins_with_characters(names: &[&str]) -> PlayerSkins {
        PlayerSkins {
            characters: names.iter().map(|n| test_character(n)).collect(),
            ..PlayerSkins::default()
        }
    }

    #[test]
    fn pose_key_uses_camel_case() {
        assert_eq!(pose_key("Female adventurer"), "femaleAdventurer");
        assert_eq!(pose_key("Robot"), "robot");
        assert_eq!(pose_key("Male person"), "malePerson");
    }

    #[test]
    fn facing_from_angle_maps_eight_ways() {
        // Angles just inside each 45° sector (sector edges are exclusive).
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
        // Angle wraps around.
        assert_eq!(Facing::from_angle(TAU - 0.1), Facing::East);
        assert_eq!(Facing::from_angle(-TAU), Facing::East);
    }

    #[test]
    fn facing_pose_maps_arcs_to_panels() {
        let c = test_character("X");
        assert_eq!(facing_pose(&c, Facing::North), c.back);
        assert_eq!(facing_pose(&c, Facing::NorthEast), c.back);
        assert_eq!(facing_pose(&c, Facing::NorthWest), c.back);
        assert_eq!(facing_pose(&c, Facing::East), c.side);
        assert_eq!(facing_pose(&c, Facing::West), c.side);
        assert_eq!(facing_pose(&c, Facing::South), c.idle);
        assert_eq!(facing_pose(&c, Facing::SouthEast), c.idle);
        assert_eq!(facing_pose(&c, Facing::SouthWest), c.idle);
    }

    #[test]
    fn cycle_wraps_forward_and_backward() {
        let mut skins = skins_with_characters(&["A", "B", "C"]);
        cycle_skin_dir(&mut skins, 1);
        assert_eq!(skins.current, 1);
        assert!(skins.need_bake);
        cycle_skin_dir(&mut skins, 1);
        assert_eq!(skins.current, 2);
        cycle_skin_dir(&mut skins, 1); // wraps to 0
        assert_eq!(skins.current, 0);
        cycle_skin_dir(&mut skins, -1); // wraps to last
        assert_eq!(skins.current, 2);
    }

    #[test]
    fn cycle_ignores_empty_character_set() {
        let mut skins = PlayerSkins::default();
        cycle_skin_dir(&mut skins, 1);
        assert_eq!(skins.current, 0);
        assert!(!skins.need_bake);
    }

    #[test]
    fn set_by_name_applies_known_character() {
        let mut skins = skins_with_characters(TOON_CHARACTERS);
        assert!(set_skin_by_name(&mut skins, "Zombie"));
        assert_eq!(skins.current, TOON_CHARACTERS.len() - 1);
        assert!(skins.need_bake);
        // Same skin again is a no-op (no re-bake).
        skins.need_bake = false;
        assert!(set_skin_by_name(&mut skins, "zombie"));
        assert!(!skins.need_bake);
    }

    #[test]
    fn set_by_name_maps_legacy_skin_files() {
        let mut skins = skins_with_characters(TOON_CHARACTERS);
        assert!(set_skin_by_name(&mut skins, "zombieC"));
        assert_eq!(skins.characters[skins.current].name, "Zombie");
        assert!(set_skin_by_name(&mut skins, "robot2"));
        assert_eq!(skins.characters[skins.current].name, "Robot");
        assert!(set_skin_by_name(&mut skins, "astroFemaleA"));
        assert_eq!(skins.characters[skins.current].name, "Female adventurer");
        assert!(set_skin_by_name(&mut skins, "racerBlueMale"));
        assert_eq!(skins.characters[skins.current].name, "Male adventurer");
    }

    #[test]
    fn set_by_name_ignores_unknown_and_empty() {
        let mut skins = skins_with_characters(TOON_CHARACTERS);
        assert!(!set_skin_by_name(&mut skins, "Tetrahedron"));
        assert!(!set_skin_by_name(&mut skins, ""));
        assert!(!set_skin_by_name(&mut skins, "not-a-skin"));
        assert_eq!(skins.current, 0);
        assert!(!skins.need_bake);
        let mut empty = PlayerSkins::default();
        assert!(!set_skin_by_name(&mut empty, "Zombie"));
    }

    #[test]
    fn character_names_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in TOON_CHARACTERS {
            assert!(seen.insert(*name), "duplicate character: {name}");
        }
    }
}