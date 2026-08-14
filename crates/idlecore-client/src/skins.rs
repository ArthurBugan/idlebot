//! Player skins — 2D character art (assets/skins/*.png) painted onto the
//! player's 3D model as its materials' base-color texture. Cycle with [ and ].

use bevy::prelude::*;
use crate::player::Player;

/// All skin textures shipped in `assets/skins/` (kept in sync with the folder).
pub const SKIN_FILES: &[&str] = &[
    "alienA", "alienB", "astroFemaleA", "astroFemaleB", "astroMaleA",
    "astroMaleB", "athleteFemaleBlue", "athleteFemaleGreen", "athleteFemaleRed",
    "athleteFemaleYellow", "athleteMaleBlue", "athleteMaleGreen", "athleteMaleRed",
    "athleteMaleYellow", "businessMaleA", "businessMaleB", "casualFemaleA",
    "casualFemaleB", "casualMaleA", "casualMaleB", "cyborg", "fantasyFemaleA",
    "fantasyFemaleB", "fantasyMaleA", "fantasyMaleB", "farmerA", "farmerB",
    "militaryFemaleA", "militaryFemaleB", "militaryMaleA", "militaryMaleB",
    "racerBlueFemale", "racerBlueMale", "racerGreenFemale", "racerGreenMale",
    "racerOrangeFemale", "racerOrangeMale", "racerPurpleFemale", "racerPurpleMale",
    "racerRedFemale", "racerRedMale", "robot", "robot2", "robot3",
    "survivorFemaleA", "survivorFemaleB", "survivorMaleA", "survivorMaleB",
    "zombieA", "zombieB", "zombieC",
];

/// Runtime skin state: loaded textures plus the currently selected skin.
#[derive(Resource)]
pub struct PlayerSkins {
    /// Image handles in the same order as `SKIN_FILES`.
    pub textures: Vec<Handle<Image>>,
    pub current: usize,
    /// Set when the player model's materials should be re-baked with the
    /// current skin (the GLB spawns asynchronously, so we retry until the
    /// meshes exist).
    pub need_bake: bool,
}

impl Default for PlayerSkins {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            current: 0,
            need_bake: false,
        }
    }
}

pub struct SkinsPlugin;

impl Plugin for SkinsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSkins>();
        app.add_systems(Update, (setup_skins, bake_skin_to_model, cycle_skin));
    }
}

/// One-time setup: load all skin textures.
fn setup_skins(
    asset_server: Res<AssetServer>,
    mut skins: ResMut<PlayerSkins>,
) {
    if !skins.textures.is_empty() {
        return;
    }

    let mut textures = Vec::with_capacity(SKIN_FILES.len());
    for name in SKIN_FILES {
        textures.push(asset_server.load::<Image>(format!("skins/{name}.png")));
    }

    skins.textures = textures;
    skins.current = 0;
    skins.need_bake = true;
}

/// Paint the current skin onto every material of the player's 3D model.
fn bake_skin_to_model(
    mut skins: ResMut<PlayerSkins>,
    player_query: Query<Entity, With<Player>>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !skins.need_bake {
        return;
    }
    let Some(texture) = skins.textures.get(skins.current).cloned() else {
        return;
    };

    let mut baked = 0;
    if let Some(root) = player_mesh_entities(&player_query, &children) {
        for entity in root {
            if let Ok(handle) = mesh_materials.get(entity) {
                if let Some(mut material) = materials.get_mut(handle) {
                    material.base_color_texture = Some(texture.clone());
                    material.base_color = Color::WHITE;
                    material.unlit = true;
                    baked += 1;
                }
            }
        }
    }
    if baked == 0 {
        // The GLB subtree may not have spawned yet — retry next frame.
        return;
    }
    skins.need_bake = false;
    info!("Skin painted onto model: {}", SKIN_FILES[skins.current]);
}

/// All mesh entities under the player root (the rig subtree).
fn player_mesh_entities(
    player_query: &Query<Entity, With<Player>>,
    children: &Query<&Children>,
) -> Option<Vec<Entity>> {
    let player = player_query.single().ok()?;
    let mut entities = vec![player];
    if children.get(player).is_ok() {
        entities.extend(children.iter_descendants::<Children>(player));
    }
    Some(entities)
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
    if skins.textures.is_empty() {
        return;
    }
    skins.current = (skins.current as isize + dir).rem_euclid(skins.textures.len() as isize) as usize;
    skins.need_bake = true;
    info!("Skin: {}", SKIN_FILES[skins.current]);
}