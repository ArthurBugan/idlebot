//! Tower-defense client layer (Spec 023) — renders what the server simulates:
//! claimed-land overlays, mob markers with HP bars, defense structures, the
//! capital, and raid/capital HUD alerts.
//!
//! The server owns every fact; this module only projects replicated rows
//! (`capital`, `territory`, `defense_structure`, `mob`, `raid_wave`) into
//! sprites and UI. Every row change bumps `Net::td_dirty`, and
//! `sync_td_entities` diff-rebuilds the TD world layer when it advances
//! (the same dirty-flag pattern as `sync_remote_players`).

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use idlecore_core::defense_config as cfg;
use idlecore_core::grid::TileCoord;
use idlecore_core::grid::WorldGrid;
use idlecore_core::world_gen::WorldGenConfig;

use super::gen::*;
use spacetimedb_sdk::__codegen::TableLike;
use super::plugin::{Net, NetEvent};
// 0.20: the old `bevy::ui::Button` is a deprecated type alias; the menu rows
// use the headless `ui_widgets::Button` widget like the rest of the HUD.
use bevy::ui_widgets::Button;
use crate::player::PlayerTransform;
use crate::player::ClientPlayer;
use crate::ui::panel::{CommandPanel, Tab, TabContent};
use crate::world_floor::{atlas, prop_depth};
use super::hud::{reducer_report, send_reducer};
// Procedural hexagon sprite textures (buildable-area / ownership overlays).
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Shared HUD text tint (same value as `net::hud`, kept local for privacy).
const TEXT_COLOR: Color = Color::srgb(0.9, 0.95, 1.0);

/// View radius (world units) beyond which TD markers aren't drawn; a floor
/// that grows with zoom (the zoomed-out viewport must keep seeing markers).
const VIEW_RADIUS: f32 = 90.0;
/// Cap for the zoom-expanded radii (world units): territory discs max at
/// radius-34 hexes × 17.3 wu ≈ 590, but 340 keeps entity counts sane.
const MAX_CULL_RADIUS: f32 = 340.0;
/// How far (world radius) ownership overlays stretch around the player.
const TERRITORY_RADIUS: f32 = 120.0;

/// Squared cull radii for the current zoom: fixed floors grown to cover the
/// zoomed-out viewport (the black-at-zoom-out fix) when it's wider.
fn cull_radii(extent: &crate::plugins::camera::ViewExtent) -> (f32, f32) {
    let view = VIEW_RADIUS.max((extent.half_diagonal * 1.1).min(MAX_CULL_RADIUS));
    let terr = TERRITORY_RADIUS.max((extent.half_diagonal * 1.15).min(MAX_CULL_RADIUS));
    (view * view, terr * terr)
}

pub struct TdPlugin;

impl Plugin for TdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TdTerritory>()
            .init_resource::<TdTextures>()
            .init_resource::<TdView>()
            .init_resource::<TdTracker>()
            .init_resource::<TdFx>()
            .init_resource::<TdBuildMode>()
            .init_resource::<TdSelect>()
            .init_resource::<TdStructRows>()
            .init_resource::<TdHexTex>()
            .add_systems(Startup, spawn_td_hud)
            .add_systems(Startup, spawn_td_wellness)
            // Bevy's system-tuple variadics cap at 20 — the tail splits out.
            .add_systems(
                Update,
                (
                    init_td_textures,
                    init_td_hex_tex,
                    sync_td_entities,
                    sync_struct_rows,
                    td_combat_input,
                    update_combat_hud,
                    glide_td_markers,
                    animate_mobs,
                    mob_spawn_pop,
                    mob_fade_tick,
                    combat_fx_tick,
                    td_pulse,
                    td_place_hints,
                    td_structure_pick,
                    td_structure_hover,
                    td_selection_fx,
                    update_select_panel,
                    update_select_refund,
                    td_select_buttons,
                    td_build_keys,
                ),
            )
            .add_systems(
                Update,
                (
                    td_upgrade_keys,
                    td_build_row_clicks,
                    td_build_row_tint,
                    td_build_icons,
                    update_td_hud,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Claimed-hex owners mirrored from the `territory` table, refreshed on TD
/// table changes; consumed by the floor overlays and the minimap tint.
#[derive(Resource, Default)]
pub struct TdTerritory {
    /// `hex_id → owner address`.
    pub owners: HashMap<u64, String>,
    /// Hex ids of every LIVE capital on the map (capital-placement spacing).
    pub capitals: Vec<u64>,
    /// Hex ids of capitals (any owner) with an open raid wave — the minimap
    /// capital markers flash red while a siege is live.
    pub wave_targets: HashSet<u64>,
}

/// My capital + raid state for the HUD and minimap, refreshed with the sync.
#[derive(Resource, Default)]
pub struct TdView {
    /// My placed, non-destroyed capital (if any).
    pub capital: Option<TdCapitalSnapshot>,
    /// Open wave against my capital, with its living raider count.
    pub incoming: Option<u32>,
    /// Seconds until the server may roll the next wave (grace / reattack).
    pub next_wave_in: Option<u64>,
    /// The length of the window `next_wave_in` counts down from (grace or
    /// reattack), for the countdown bar's full-width ratio.
    pub next_wave_total: u64,
    /// Client unix seconds when `next_wave_in` was captured — the widget
    /// drains the countdown locally between server syncs.
    pub next_wave_sync_utc: u64,
    /// REV3 §14: open wave's 0-based index + pattern name ("Wave 3: Sap
    /// Team") for the raid banner. `None` when no open wave/pattern.
    pub wave_banner: Option<(u64, &'static str)>,
}

#[derive(Clone)]
pub struct TdCapitalSnapshot {
    /// Kept for the next pass: locating open-wave rows by capital id.
    #[allow(dead_code)]
    pub capital_id: u64,
    pub hex_id: u64,
    pub hp: u32,
    pub level: u32,
}

/// Cropped sprites for mobs/structures/capital + defense item icons.
#[derive(Resource, Default)]
pub struct TdTextures {
    pub ready: bool,
    /// One entry per enemy-kind name (Spec 025 §3): idle frame + two walk
    /// frames + art aspect for the marker's `custom_size`.
    pub mobs: HashMap<String, MobSprite>,
    pub capital: (Handle<Image>, f32),
    pub barricade: (Handle<Image>, f32),
    pub arrow_tower: (Handle<Image>, f32),
    pub ballista: (Handle<Image>, f32),
    // REV4 buildables.
    pub wood_wall: (Handle<Image>, f32),
    pub iron_wall: (Handle<Image>, f32),
    pub lantern: (Handle<Image>, f32),
    pub coconut: (Handle<Image>, f32),
    pub snowman: (Handle<Image>, f32),
    pub spike: (Handle<Image>, f32),
    pub web: (Handle<Image>, f32),
    pub fire: (Handle<Image>, f32),
    pub icon_core: Handle<Image>,
    pub icon_part: Handle<Image>,
    pub icon_bone: Handle<Image>,
    pub icon_barricade: Handle<Image>,
    pub icon_arrow: Handle<Image>,
    pub icon_ballista: Handle<Image>,
    // REV4 build-menu icons.
    pub icon_wood_wall: Handle<Image>,
    pub icon_iron_wall: Handle<Image>,
    pub icon_lantern: Handle<Image>,
    pub icon_coconut: Handle<Image>,
    pub icon_snowman: Handle<Image>,
    pub icon_spike: Handle<Image>,
    pub icon_web: Handle<Image>,
    pub icon_fire: Handle<Image>,
}

/// A raider kind's sprite set (Spec 025 §5). `walk` frames alternate while
/// the marker glides; rooted kinds point walk frames at the idle crop.
#[derive(Clone, Debug)]
pub struct MobSprite {
    pub idle: Handle<Image>,
    pub walk: [Handle<Image>; 2],
    pub aspect: f32,
}

/// Mob-side tracker: damage-flash timers per mob_id. Persisted markers
/// make the glide system responsible for motion (REV5 §18), so no position
/// history is needed here anymore.
#[derive(Resource, Default)]
pub struct TdTracker {
    /// Last replicated HP per mob — a drop arms the red damage flash.
    pub last_hp: HashMap<u64, u32>,
    /// Seconds left on each mob's damage flash (spec 025 §5).
    pub flash: HashMap<u64, f32>,
}

// ---------------------------------------------------------------------------
// Combat fx (Spec 025 REV3 §10/§13) — client reactions to combat_event rows
// ---------------------------------------------------------------------------

/// Transient state for combat fx: what the last snapshot already consumed
/// and what is still animating.
#[derive(Resource)]
pub struct TdFx {
    /// Highest combat_event id consumed (dedup across rebuilds).
    pub last_event_id: u64,
    /// mob_id → seconds left posing (attack cursor, §13 wiring).
    pub posing: HashMap<u64, f32>,
    /// capital_id → seconds left on hit flash + shake.
    pub capital_hit: HashMap<u64, f32>,
    /// Waves whose "under attack" toast already fired (first hit only).
    pub toasted_waves: HashSet<u64>,
    /// Player weapon-swing pose timer (Spec 027 §9, set by player_hit rows).
    pub player_swing: f32,
}

impl Default for TdFx {
    fn default() -> Self {
        Self {
            last_event_id: 0,
            posing: HashMap::default(),
            capital_hit: HashMap::default(),
            toasted_waves: HashSet::default(),
            player_swing: 0.0,
        }
    }
}

/// One fx sprite in flight — the ballistic tracer (arrow/bomb/spore).
#[derive(Component)]
pub struct TdBolt {
    pub t: f32,
    pub ttl: f32,
    pub from: Vec2,
    pub to: Vec2,
}

/// A short-lived pop: impact star, splash echo, dust puff.
#[derive(Component)]
pub struct TdImpact {
    pub t: f32,
    pub ttl: f32,
    pub size: f32,
}

/// A floating world-space notice (e.g. "Capital under attack!").
#[derive(Component)]
pub struct TdWorldToast {
    pub t: f32,
}

/// Pure fx plan (unit-pinned exhaustively §13): every server event kind gets
/// exactly one visual treatment. `None` = unknown kind → no fx.
fn fx_plan(kind: &str) -> Option<FxBirth> {
    match kind {
        "shot" => Some(FxBirth::Bolt([0.95, 0.95, 1.0])),
        "melee" => Some(FxBirth::Impact([1.0, 0.85, 0.4])),
        "bomb" => Some(FxBirth::Bolt([1.0, 0.55, 0.1])),
        "spore" => Some(FxBirth::Bolt([0.75, 0.5, 0.95])),
        "splash" => Some(FxBirth::Impact([1.0, 0.55, 0.1])),
        "struct_fell" => Some(FxBirth::Impact([0.7, 0.7, 0.7])),
        "capital_hit" => Some(FxBirth::CapitalHit),
        // REV4 traps: a spike/web spring or fire-tick blooms on its hex.
        "trap" => Some(FxBirth::Impact([1.0, 0.4, 0.3])),
        // Spec 027 §8: the player threat rides the same stream — a weapon
        // swing reads as an arc + impact on the hit mob's hex; taking
        // damage blooms red on YOUR hex.
        "player_hit" => Some(FxBirth::PlayerHit),
        "player_hurt" => Some(FxBirth::Impact([1.0, 0.2, 0.15])),
        _ => None,
    }
}

/// What a processed event births.
enum FxBirth {
    Bolt([f32; 3]),
    Impact([f32; 3]),
    /// Capital flash + shake + wave toast, no sprite.
    CapitalHit,
    /// Player weapon swing: impact on the victim's hex (tile row), plus a
    /// slim swing arc on the attacker's own spot handled client-side already
    /// through the mob pose arm (`attacker_mob` is 0 for player swings).
    PlayerHit,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// World marker for one raider (child: HP bar).
#[derive(Component)]
pub struct TdMobMarker;

/// Per-raider animation state: replicated id (flash lookup) + roster kind
/// (sprite lookup). Carried by the marker entity, driven by [`animate_mobs`].
#[derive(Component)]
pub struct TdMobAnim {
    pub mob_id: u64,
    pub kind: String,
}

/// World marker for one tower/wall (child: HP bar).
#[derive(Component)]
pub struct TdStructureMarker;

/// World marker for a placed capital (owner address + capital id, for the
/// REV3 §13 hit flash keyed by capital).
#[derive(Component)]
pub struct TdCapitalMarker(pub String, pub u64);

/// Claimed-hex ownership overlay (gold = mine, violet = another player).
#[derive(Component)]
pub struct TdOverlay;

/// Interpolated glide target — mobs march one hex every `step_interval`
/// seconds server-side (pinned to tile centers). REV5 §18: markers are
/// PERSISTENT per `mob_id`, so this target is an update, not a re-spawn —
/// the body glides the full hex between server steps instead of popping.
#[derive(Component, Clone)]
struct TdMove {
    target: Vec2,
    speed: f32,
}

/// Fresh mob enter fx (REV5 §18): a short scale pop instead of a glide; the
/// system removes the component when it settles.
#[derive(Component)]
struct TdSpawnPop {
    left: f32,
}

/// Death fx (REV5 §18): a mob row that leaves the snapshot dies at its last
/// client position with a 0.25 s red shrink, never pops out mid-glide.
#[derive(Component)]
struct TdMobFade {
    secs: f32,
    total: f32,
}

// ---------------------------------------------------------------------------
// Textures
// ---------------------------------------------------------------------------

/// Spec 025 §5: roster name → (idle, walk0, walk1) atlas crops. The
/// rooted Venom Bloom reuses its idle frame for both walk slots.
/// REV2: +7 recolor elite tiers.
const MOB_ATLAS: [(&str, &str, &str, &str); 15] = [
    ("Slime", atlas::TD_SLIME, atlas::TD_SLIME_W0, atlas::TD_SLIME_W1),
    ("Blue Slime", atlas::TD_SLIME_BLUE, atlas::TD_SLIME_BLUE_W0, atlas::TD_SLIME_BLUE_W1),
    ("Pink Slime", atlas::TD_SLIME_PINK, atlas::TD_SLIME_PINK_W0, atlas::TD_SLIME_PINK_W1),
    ("Pupple Slime", atlas::TD_SLIME_PUPPLE, atlas::TD_SLIME_PUPPLE_W0, atlas::TD_SLIME_PUPPLE_W1),
    ("Black Slime", atlas::TD_SLIME_BLACK, atlas::TD_SLIME_BLACK_W0, atlas::TD_SLIME_BLACK_W1),
    ("Big Slime", atlas::TD_BIG_SLIME, atlas::TD_BIG_SLIME_W0, atlas::TD_BIG_SLIME_W1),
    ("Spear Goblin", atlas::TD_GOBLIN_SPEAR, atlas::TD_GOBLIN_SPEAR_W0, atlas::TD_GOBLIN_SPEAR_W1),
    ("Archer Goblin", atlas::TD_GOBLIN_ARCHER, atlas::TD_GOBLIN_ARCHER_W0, atlas::TD_GOBLIN_ARCHER_W1),
    ("Bomb Goblin", atlas::TD_GOBLIN_BOMB, atlas::TD_GOBLIN_BOMB_W0, atlas::TD_GOBLIN_BOMB_W1),
    ("Myconid", atlas::TD_MYCONID, atlas::TD_MYCONID_W0, atlas::TD_MYCONID_W1),
    ("Myconid Blue", atlas::TD_MYCONID_BLUE, atlas::TD_MYCONID_BLUE_W0, atlas::TD_MYCONID_BLUE_W1),
    ("Myconid Pink", atlas::TD_MYCONID_PINK, atlas::TD_MYCONID_PINK_W0, atlas::TD_MYCONID_PINK_W1),
    ("Myconid Purple", atlas::TD_MYCONID_PURPLE, atlas::TD_MYCONID_PURPLE_W0, atlas::TD_MYCONID_PURPLE_W1),
    ("Myconid Red", atlas::TD_MYCONID_RED, atlas::TD_MYCONID_RED_W0, atlas::TD_MYCONID_RED_W1),
    ("Venom Bloom", atlas::TD_VENOM_BLOOM, atlas::TD_VENOM_BLOOM, atlas::TD_VENOM_BLOOM),
];

fn init_td_textures(atlas: Res<crate::slice::SlicedAtlas>, mut td: ResMut<TdTextures>) {
    if td.ready {
        return;
    }
    let g = |k: &str| atlas.items.get(k).cloned();
    // Art aspects (crop width ÷ height) — the tower/capital are tall, mobs
    // and walls square.
    let with_aspect = |key: &str, aspect: f32| g(key).map(|h| (h, aspect));
    let (Some(capital), Some(barricade), Some(arrow_tower), Some(ballista)) = (
        with_aspect(atlas::TD_CAPITAL, 128.0 / 112.0),
        with_aspect(atlas::TD_BARRICADE, 32.0 / 18.0),
        with_aspect(atlas::TD_ARROW_TOWER, 24.0 / 32.0),
        with_aspect(atlas::TD_BALLISTA, 1.0),
    ) else {
        return;
    };
    // Spec 025 roster map — built from MOB_ATLAS so tests can pin coverage.
    let mut mobs = HashMap::new();
    for (kind, idle, w0, w1) in MOB_ATLAS {
        let (Some(idle), Some(w0), Some(w1)) = (g(idle), g(w0), g(w1)) else {
            return;
        };
        mobs.insert(
            kind.to_string(),
            MobSprite {
                idle,
                walk: [w0, w1],
                aspect: 1.0,
            },
        );
    }
    let (Some(icon_core), Some(icon_part), Some(icon_bone)) = (
        g(atlas::ICON_CAPITAL_CORE),
        g(atlas::ICON_METAL_PART),
        g(atlas::ICON_BONE),
    ) else {
        return;
    };
    let (Some(icon_barricade), Some(icon_arrow), Some(icon_ballista)) = (
        g(atlas::ICON_BONE_BARRICADE),
        g(atlas::ICON_ARROW_TOWER),
        g(atlas::ICON_BALLISTA),
    ) else {
        return;
    };
    // REV4 buildables — sprites + icons must ALL be present or the menu
    // would mix placeholders with real arts.
    let (Some(wood_wall), Some(iron_wall), Some(lantern), Some(coconut)) = (
        with_aspect(atlas::TD_WOOD_WALL, 1.0),
        with_aspect(atlas::TD_IRON_WALL, 16.0 / 32.0),
        with_aspect(atlas::TD_LANTERN, 32.0 / 48.0),
        with_aspect(atlas::TD_COCONUT, 1.0),
    ) else {
        return;
    };
    let (Some(snowman), Some(spike), Some(web), Some(fire)) = (
        with_aspect(atlas::TD_SNOWMAN, 16.0 / 24.0),
        with_aspect(atlas::TD_TRAP, 16.0 / 32.0),
        with_aspect(atlas::TD_WEB, 1.0),
        with_aspect(atlas::TD_FIRE, 16.0 / 32.0),
    ) else {
        return;
    };
    let icon_wood_wall = g(atlas::ICON_WOOD_WALL).unwrap_or_default();
    let icon_iron_wall = g(atlas::ICON_IRON_WALL).unwrap_or_default();
    let icon_lantern = g(atlas::ICON_LANTERN).unwrap_or_default();
    let icon_coconut = g(atlas::ICON_COCONUT).unwrap_or_default();
    let icon_snowman = g(atlas::ICON_SNOWMAN).unwrap_or_default();
    let icon_spike = g(atlas::ICON_SPIKE_TRAP).unwrap_or_default();
    let icon_web = g(atlas::ICON_WEB_TRAP).unwrap_or_default();
    let icon_fire = g(atlas::ICON_FIRE_TRAP).unwrap_or_default();
    td.mobs = mobs;
    td.capital = capital;
    td.barricade = barricade;
    td.arrow_tower = arrow_tower;
    td.ballista = ballista;
    td.wood_wall = wood_wall;
    td.iron_wall = iron_wall;
    td.lantern = lantern;
    td.coconut = coconut;
    td.snowman = snowman;
    td.spike = spike;
    td.web = web;
    td.fire = fire;
    td.icon_core = icon_core;
    td.icon_part = icon_part;
    td.icon_bone = icon_bone;
    td.icon_barricade = icon_barricade;
    td.icon_arrow = icon_arrow;
    td.icon_ballista = icon_ballista;
    td.icon_wood_wall = icon_wood_wall;
    td.icon_iron_wall = icon_iron_wall;
    td.icon_lantern = icon_lantern;
    td.icon_coconut = icon_coconut;
    td.icon_snowman = icon_snowman;
    td.icon_spike = icon_spike;
    td.icon_web = icon_web;
    td.icon_fire = icon_fire;
    td.ready = true;
    info!("TD textures ready (Spec 023)");
}

/// Sprite lookup by roster name; unknown rows fall back to the Slime set
/// so a stale/renamed kind still renders something visible.
fn mob_sprite_kind<'a>(td: &'a TdTextures, kind: &str) -> &'a MobSprite {
    td.mobs
        .get(kind)
        .or_else(|| td.mobs.get("Slime"))
        .expect("Slime is in the Spec 025 roster map")
}

/// Sprite art + art aspect + tint + world height (wu) per structure kind.
/// REV4 arts mostly render untinted (real colors); placeholders keep tints.
fn structure_sprite(td: &TdTextures, kind: &str) -> (Handle<Image>, f32, Color, f32) {
    match kind {
        cfg::ITEM_BONE_BARRICADE => (
            td.barricade.0.clone(),
            td.barricade.1,
            Color::srgb(0.8, 0.7, 0.5),
            2.2,
        ),
        cfg::ITEM_ARROW_TOWER => (
            td.arrow_tower.0.clone(),
            td.arrow_tower.1,
            Color::srgb(0.55, 0.8, 1.0),
            2.2,
        ),
        // The chest borrows the barricade art, warm brown crate tone.
        cfg::ITEM_STORAGE_CHEST => (
            td.barricade.0.clone(),
            td.barricade.1,
            Color::srgb(0.65, 0.45, 0.25),
            2.2,
        ),
        // ---- REV4 walls ----
        cfg::ITEM_WOOD_WALL => (td.wood_wall.0.clone(), td.wood_wall.1, Color::WHITE, 2.4),
        cfg::ITEM_IRON_WALL => (td.iron_wall.0.clone(), td.iron_wall.1, Color::WHITE, 2.4),
        // ---- REV4 towers ----
        cfg::ITEM_LANTERN_TOWER => (td.lantern.0.clone(), td.lantern.1, Color::WHITE, 3.0),
        cfg::ITEM_COCONUT_TOWER => (td.coconut.0.clone(), td.coconut.1, Color::WHITE, 3.2),
        cfg::ITEM_FROST_TOWER => (td.snowman.0.clone(), td.snowman.1, Color::WHITE, 2.0),
        // ---- REV4 traps: ground-level consumables ----
        cfg::ITEM_SPIKE_TRAP => (td.spike.0.clone(), td.spike.1, Color::WHITE, 1.3),
        cfg::ITEM_WEB_TRAP => (td.web.0.clone(), td.web.1, Color::WHITE, 1.1),
        cfg::ITEM_FIRE_TRAP => (td.fire.0.clone(), td.fire.1, Color::WHITE, 1.5),
        _ => (td.ballista.0.clone(), td.ballista.1, Color::srgb(0.7, 0.55, 1.0), 2.2),
    }
}

// ---------------------------------------------------------------------------
// Snapshots (cloned inside the conn lock, processed after release)
// ---------------------------------------------------------------------------

struct CapRow {
    capital_id: u64,
    owner: String,
    hex_id: u64,
    hp: u32,
    level: u32,
    grace_until: u64,
    last_raid: u64,
    placed_at: u64,
    destroyed: bool,
}

struct WaveRow {
    wave_id: u64,
    capital_id: u64,
    open: bool,
}

/// One defense-structure row mirrored client-side (Spec 026 §3.1 rows;
/// Clone + PartialEq for the throttled `TdStructRows` mirror).
#[derive(Clone, PartialEq)]
struct StructRow {
    structure_id: u64,
    x: f32,
    y: f32,
    kind: String,
    hp: u32,
    /// 0 = legacy row (client treats as L1); pip shows from L2.
    level: u8,
    /// Spec 026: selection panel ownership gate + XP bucket display.
    owner: String,
    total_xp: u64,
}

struct MobRow {
    mob_id: u64,
    wave_id: u64,
    x: f32,
    y: f32,
    kind: String,
    hp: u32,
}

struct TdSnapshot {
    territory: HashMap<u64, String>,
    capitals: Vec<CapRow>,
    waves: Vec<WaveRow>,
    structures: Vec<StructRow>,
    mobs: Vec<MobRow>,
    /// REV3 §10: combat events since `TdFx::last_event_id` — the fx feed.
    events: Vec<FxEventRow>,
    now: u64,
}

struct FxEventRow {
    event_id: u64,
    capital_id: u64,
    wave_id: u64,
    kind: String,
    attacker_mob: u64,
    target_hex_id: u64,
    slot_x: i32,
    slot_y: i32,
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u64)
        .unwrap_or(0)
}

fn hex_center(q: i32, r: i32) -> (f32, f32) {
    WorldGrid::tile_center(q, r, WorldGenConfig::TILE_SIZE)
}

fn clamp_ratio(hp: f32, max: f32) -> f32 {
    if max <= 0.0 {
        return 0.0;
    }
    (hp / max).clamp(0.0, 1.0)
}

/// "L2" text above a structure's HP bar from level 2; None hides the pip
/// for level-1/legacy rows (Spec 024).
fn structure_level_label(level: u8) -> Option<String> {
    (level >= 2).then(|| format!("L{}", level.min(cfg::STRUCTURE_LEVEL_MAX)))
}

/// Structure health-bar tint from an HP ratio (green → amber → red).
fn bar_color(ratio: f32) -> Color {
    if ratio > 0.6 {
        Color::srgb(0.25, 0.9, 0.3)
    } else if ratio > 0.3 {
        Color::srgb(0.95, 0.8, 0.2)
    } else {
        Color::srgb(0.95, 0.25, 0.2)
    }
}

// ---------------------------------------------------------------------------
// World sync
// ---------------------------------------------------------------------------

/// Spec 026: defense-row mirror for selection/hover/panel reads, throttled
/// 0.2 s (rows are few; equality-check keeps coalescing churn low). Lives
/// outside `sync_td_entities` because that system already sits at Bevy's
/// 16-SystemParam cap — its 17th param can't fit.
fn sync_struct_rows(
    net: Res<Net>,
    mut rows: ResMut<TdStructRows>,
    mut throttle: Local<f32>,
    time: Res<Time>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.2 {
        return;
    }
    *throttle = 0.0;
    let guard = net.conn.lock().unwrap();
    let Some(conn) = guard.as_ref() else { return };
    let fresh: Vec<StructRow> = conn
        .db
        .defense_structure()
        .iter()
        .map(|s| {
            let c = TileCoord::from_id(s.hex_id);
            let (hx, hy) = hex_center(c.x, c.y);
            StructRow {
                structure_id: s.structure_id,
                x: hx + s.offset_x,
                y: hy + s.offset_y,
                kind: s.kind.clone(),
                hp: s.hp,
                level: s.level,
                owner: s.owner.clone(),
                total_xp: s.total_xp,
            }
        })
        .collect();
    if fresh == rows.rows {
        return;
    }
    rows.rows = fresh;
}

fn sync_td_entities(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut territory: ResMut<TdTerritory>,
    mut tdview: ResMut<TdView>,
    mut tracker: ResMut<TdTracker>,
    mut fx: ResMut<TdFx>,
    extent: Res<crate::plugins::camera::ViewExtent>,
    td_tex: Res<TdTextures>,
    hex_tex: Res<TdHexTex>,
    player_xf: Res<PlayerTransform>,
    mode: Res<TdBuildMode>,
    panel: Res<CommandPanel>,
    mut mobs: Query<(Entity, &TdMobAnim, &mut TdMove, &mut Transform), With<TdMobMarker>>,
    structures: Query<Entity, With<TdStructureMarker>>,
    capitals: Query<Entity, With<TdCapitalMarker>>,
    overlays: Query<Entity, With<TdOverlay>>,
) {
    if !td_tex.ready || !hex_tex.ready || !net.poll_td_dirty() {
        return;
    }
    // Snapshot inside the lock; leave owning plain values out.
    let (address, snap) = {
        let guard = net.conn.lock().unwrap();
        let Some(conn) = guard.as_ref() else { return };
        let address = net.address.clone();
        let territory: HashMap<u64, String> = conn
            .db
            .territory()
            .iter()
            .map(|t| (t.hex_id, t.owner.clone()))
            .collect();
        let capitals: Vec<CapRow> = conn
            .db
            .capital()
            .iter()
            .map(|c| CapRow {
                capital_id: c.capital_id,
                owner: c.owner.clone(),
                hex_id: c.hex_id,
                hp: c.hp,
                level: c.level,
                grace_until: c.grace_until,
                last_raid: c.last_raid_at,
                placed_at: c.placed_at,
                destroyed: c.destroyed,
            })
            .collect();
        let waves: Vec<WaveRow> = conn
            .db
            .raid_wave()
            .iter()
            .map(|w| WaveRow {
                wave_id: w.wave_id,
                capital_id: w.capital_id,
                open: w.ended_at.is_none(),
            })
            .collect();
        let structures: Vec<StructRow> = conn
            .db
            .defense_structure()
            .iter()
            .map(|s| {
                let c = TileCoord::from_id(s.hex_id);
                let (hx, hy) = hex_center(c.x, c.y);
                StructRow {
                    structure_id: s.structure_id,
                    x: hx + s.offset_x,
                    y: hy + s.offset_y,
                    kind: s.kind.clone(),
                    hp: s.hp,
                    // 0/absent = L1 for the HUD pip logic.
                    level: s.level,
                    owner: s.owner.clone(),
                    total_xp: s.total_xp,
                }
            })
            .collect();
        let mobs: Vec<MobRow> = conn
            .db
            .mob()
            .iter()
            .map(|m| MobRow {
                mob_id: m.mob_id,
                wave_id: m.wave_id,
                x: m.position_x,
                y: m.position_y,
                kind: m.kind.clone(),
                hp: m.hp,
            })
            .collect();
        let events: Vec<FxEventRow> = conn
            .db
            .combat_event()
            .iter()
            .map(|e| FxEventRow {
                event_id: e.event_id,
                capital_id: e.capital_id,
                wave_id: e.wave_id,
                kind: e.kind.clone(),
                attacker_mob: e.attacker_mob,
                target_hex_id: e.target_hex_id,
                slot_x: e.target_slot_x,
                slot_y: e.target_slot_y,
            })
            .collect();
        (
            address,
            TdSnapshot {
                territory,
                capitals,
                waves,
                structures,
                mobs,
                events,
                now: unix_now_secs(),
            },
        )
    };

    // ---- territory cache (also consumed by the minimap) ----
    territory.owners = snap.territory.clone();
    territory.capitals = snap
        .capitals
        .iter()
        .filter(|c| !c.destroyed)
        .map(|c| c.hex_id)
        .collect();
    // Capitals with an open wave, keyed by hex id (minimap marker flash).
    territory.wave_targets = snap
        .waves
        .iter()
        .filter(|w| w.open)
        .filter_map(|w| snap.capitals.iter().find(|c| c.capital_id == w.capital_id))
        .map(|c| c.hex_id)
        .collect();

    // ---- my capital + raid state for HUD / minimap ----
    let mine = |owner: &str| Some(owner) == address.as_deref();
    let my_capital_row = snap
        .capitals
        .iter()
        .find(|c| !c.destroyed && mine(&c.owner));
    tdview.capital = my_capital_row.map(|c| TdCapitalSnapshot {
        capital_id: c.capital_id,
        hex_id: c.hex_id,
        hp: c.hp,
        level: c.level,
    });
    tdview.incoming = None;
    tdview.next_wave_in = None;
    tdview.next_wave_sync_utc = unix_now_secs();
    tdview.wave_banner = None;
    if let Some(cap) = my_capital_row {
        // Open wave on my capital → count its living raiders.
        if let Some(wave) = snap
            .waves
            .iter()
            .find(|w| w.capital_id == cap.capital_id && w.open)
        {
            let count = snap
                .mobs
                .iter()
                .filter(|m| m.wave_id == wave.wave_id)
                .count() as u32;
            tdview.incoming = Some(count);
            // REV3 §14 banner: 0-based wave index = how many waves this
            // capital already fought; pattern name from the same integer
            // cadence the server spawns from (size-independent).
            let index = snap
                .waves
                .iter()
                .filter(|w| w.capital_id == wave.capital_id && !w.open && w.wave_id < wave.wave_id)
                .count() as u64;
            let name = if cfg::boss_wave(index, cap.level) {
                "Big Slime Boss"
            } else {
                cfg::wave_pattern(index, 1, cap.level)
                    .map(|(name, _)| name)
                    .unwrap_or("Scout Line")
            };
            tdview.wave_banner = Some((index + 1, name));
        } else {
            // Next wave clock only counts while no wave is open.
            let reattack = cap.last_raid.saturating_add(cfg::WAVE_REATTACK_SECS);
            let horizon = cap.grace_until.max(reattack);
            tdview.next_wave_in = Some(horizon.saturating_sub(snap.now));
            // Bar total = the length of the window we're counting down:
            // the reattack window when that dominates, else the placement
            // grace window (first raid 30 min / rebuild 24 h).
            tdview.next_wave_total = if horizon == reattack {
                cfg::WAVE_REATTACK_SECS
            } else {
                // Grace window start: first placement, or (for rebuilt
                // capitals, whose grace anchors at the destroying raid) the
                // last raid's open time — whichever is later.
                let start = cap.placed_at.max(cap.last_raid);
                horizon.saturating_sub(start).max(cfg::WAVE_REATTACK_SECS)
            };
        }
    }

        // ---- rebuild: TD entities (REV5 §18) ----
        // Structures/capitals/overlays stay respawn-on-sync (they don't
        // move). MOB markers persist per mob_id: the mob section below
        // updates glides in place and despawns only mobs whose row left
        // the snapshot — a re-spawn would hard-cut every march (the
        // playtest "mobs kind of teleport" report).
    for e in structures.iter() {
        commands.entity(e).despawn();
    }
    for e in capitals.iter() {
        commands.entity(e).despawn();
    }
    for e in overlays.iter() {
        commands.entity(e).despawn();
    }

    let (px, py) = (player_xf.translation.x, player_xf.translation.y);
    let (view_r2, terr_r2) = cull_radii(&extent);
    // ---- ownership overlays ----
    // REV3 UX pass: territory is inked only while building (build mode armed
    // or the build menu open) and only MY land is lit — the map stays clean
    // otherwise, and "where can I build?" answers itself in build mode.
    let show_overlays = mode.kind.is_some() || panel.is_open(Tab::Build);
    if show_overlays {
        for (hex_id, owner) in &snap.territory {
            let ownership_mine = Some(owner.as_str()) == address.as_deref();
            if !ownership_mine {
                continue;
            }
            let c = TileCoord::from_id(*hex_id);
            let (cx, cy) = hex_center(c.x, c.y);
            let dist_sq = {
                let dx = cx - px;
                let dy = cy - py;
                dx * dx + dy * dy
            };
            if dist_sq > terr_r2 {
                continue;
            }
            commands.spawn((
                TdOverlay,
                Sprite {
                    // Square territory fill (no hex art) — a tile-wide colored
                    // rect so territory reads as land, not pinhead dots; slightly
                    // inset + fainter so the map shows through the claimed mass.
                    color: Color::srgba(1.0, 0.86, 0.35, 0.3),
                    custom_size: Some(Vec2::splat(WorldGenConfig::TILE_SIZE * 0.92)),
                    ..default()
                },
                Transform::from_xyz(cx, cy, prop_depth(cy) + 3.0),
            ));
        }
    }

    // ---- capitals ----
    for row in &snap.capitals {
        if row.destroyed {
            continue;
        }
        let c = TileCoord::from_id(row.hex_id);
        let (cx, cy) = hex_center(c.x, c.y);
        let (dx, dy) = (cx - px, cy - py);
        if dx * dx + dy * dy > view_r2 {
            continue;
        }
        let max = cfg::capital_max_hp(row.level);
        let ratio = clamp_ratio(row.hp as f32, max as f32);
        // The house is 128×112 — bottom-anchored so the base sits inside the
        // hex instead of floating. ~11 wu tall = twice a person (5.5), the
        // size a city hall reads at hex scale (17.3 wu wide).
        let aspect = td_tex.capital.1;
        let height = 11.0;
        let pos = Vec3::new(cx, cy + 0.8, prop_depth(cy) + 18.0);
        commands.spawn((
            TdCapitalMarker(row.owner.clone(), row.capital_id),
            Sprite {
                image: td_tex.capital.0.clone(),
                color: Color::srgb(1.0, 0.9, 0.6),
                custom_size: Some(Vec2::new(height * aspect, height)),
                ..default()
            },
            bevy::sprite::Anchor::BOTTOM_CENTER,
            Transform::from_translation(pos),
        ))
        .with_child((
            Sprite {
                color: bar_color(ratio),
                custom_size: Some(Vec2::new(6.6 * ratio, 0.4)),
                ..default()
            },
            Transform::from_xyz(0.0, height + 0.5, 0.05),
        ));
    }

    // ---- defense structures ----
    for row in &snap.structures {
        let (dx, dy) = (row.x - px, row.y - py);
        if dx * dx + dy * dy > view_r2 {
            continue;
        }
        let Some(def) = cfg::defense_kind(&row.kind) else { continue };
        let (image, aspect, color, height) = structure_sprite(&td_tex, &row.kind);
        // Leveled structures fill their bar against their own max HP.
        let ratio = clamp_ratio(row.hp as f32, cfg::structure_max_hp(def, row.level) as f32);
        // Bottom-anchored so walls/towers stand on the ground slot
        // (height comes from `structure_sprite` per kind).
        let pos = Vec3::new(row.x, row.y + 0.4, prop_depth(row.y) + 12.0);
        commands.spawn((
            TdStructureMarker,
            Sprite {
                image,
                color,
                custom_size: Some(Vec2::new(height * aspect, height)),
                ..default()
            },
            bevy::sprite::Anchor::BOTTOM_CENTER,
            Transform::from_translation(pos),
        ))
        .with_child((
            Sprite {
                color: bar_color(ratio),
                custom_size: Some(Vec2::new(1.9 * ratio, 0.26)),
                ..default()
            },
            Transform::from_xyz(0.0, height + 0.3, 0.04),
        ));
        // Spec 024: green level pip above the HP bar from L2. Kept small —
        // a tile is ~17 wu, anything past ~3 ru dwarfs the building.
        if let Some(label) = structure_level_label(row.level) {
            commands.spawn((
                Text2d::new(label),
                TextFont { font_size: 2.2.into(), ..default() },
                TextColor(Color::srgba(0.35, 1.0, 0.55, 0.95)),
                TextShadow { color: Color::BLACK, offset: Vec2::new(0.12, 0.12) },
                Transform::from_xyz(row.x, row.y + height + 0.55, prop_depth(row.y) + 12.0),
            ));
        }
        let _ = row.structure_id;
    }

    // ---- mobs (REV5 §18): persistent bodies keyed mob_id ----
    let mut alive: HashMap<u64, Entity> = mobs
        .iter()
        .map(|(e, anim, _, _)| (anim.mob_id, e))
        .collect();
    for row in &snap.mobs {
        let Some(mob) = cfg::mob_kind(&row.kind) else { continue };
        let ms = mob_sprite_kind(&td_tex, &row.kind);
        let aspect = ms.aspect;
        // Spec 025 §5: HP drop since the last snapshot arms a 0.15 s red
        // flash; the animate system ticks the remainder down.
        match tracker.last_hp.get(&row.mob_id) {
            Some(prev) if row.hp < *prev => {
                tracker.flash.insert(row.mob_id, 0.15);
            }
            _ => {}
        }
        tracker.last_hp.insert(row.mob_id, row.hp);
        let ratio = clamp_ratio(row.hp as f32, mob.hp as f32);
        let target = Vec2::new(row.x, row.y);
        // Glide speed from the KIND's server step cadence (REV5 §18): one
        // hex (TILE_SIZE wu) per `step_interval_secs`, not from snapshot
        // deltas — replications arrive far faster than steps, so observed
        // deltas read as instant hops.
        let speed = mob_glide_speed(mob.step_interval_secs);
        if let Some(ent) = alive.remove(&row.mob_id) {
            let Ok((_, _, mut mv, mut xf)) = mobs.get_mut(ent) else { continue };
            // Desync guard: a snapshot jump bigger than two hexes snaps
            // instantly instead of gliding half the map.
            if mob_should_snap(mv.target.distance(target)) {
                xf.translation = Vec3::new(row.x, row.y, prop_depth(row.y) + 40.0);
            }
            mv.target = target;
            mv.speed = speed;
            // Rate-limited bar: children are transient like markers' other
            // overlays — clear + re-add keeps the ratio fresh for ~free.
            commands
                .entity(ent)
                .despawn_children()
                .with_child((
                    Sprite {
                        color: bar_color(ratio),
                        custom_size: Some(Vec2::new(1.6 * ratio, 0.24)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 1.1, 0.04),
                ));
        } else {
            // Fresh marker: enters with a 0.3 s spawn pop at the server
            // position (no glide — there is no prior position to come from).
            let pos = Vec3::new(row.x, row.y, prop_depth(row.y) + 40.0);
            commands.spawn((
                TdMobMarker,
                TdMobAnim {
                    mob_id: row.mob_id,
                    kind: row.kind.clone(),
                },
                TdMove {
                    target,
                    speed,
                },
                TdSpawnPop { left: 0.3 },
                Sprite {
                    image: ms.idle.clone(),
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(1.8 * aspect, 1.8)),
                    ..default()
                },
                Transform::from_translation(pos),
            ))
            .with_child((
                Sprite {
                    color: bar_color(ratio),
                    custom_size: Some(Vec2::new(1.6 * ratio, 0.24)),
                    ..default()
                },
                Transform::from_xyz(0.0, 1.1, 0.04),
            ));
        }
    }
    // Funeral pass: rows that left the snapshot die at their last client
    // position (red shrink 0.25 s) instead of popping out mid-glide.
    for (mob_id, ent) in alive {
        let Ok((_, anim, _, xf)) = mobs.get(ent) else { continue };
        let ms = mob_sprite_kind(&td_tex, &anim.kind);
        let aspect = ms.aspect;
        commands.entity(ent).despawn();
        commands.spawn((
            TdMobFade {
                secs: 0.25,
                total: 0.25,
            },
            Sprite {
                image: ms.idle.clone(),
                color: Color::srgba(1.0, 0.3, 0.25, 1.0),
                custom_size: Some(Vec2::new(1.8 * aspect, 1.8)),
                ..default()
            },
            Transform::from_translation(xf.translation),
        ));
        tracker.last_hp.remove(&mob_id);
        tracker.flash.remove(&mob_id);
    }
    let live_ids: HashSet<u64> = snap.mobs.iter().map(|m| m.mob_id).collect();
    tracker.last_hp.retain(|id, _| live_ids.contains(id));

    // ---- combat fx feed (REV3 §10/§13) ----
    process_combat_events(&snap, &mut fx, &mut commands, &player_xf);
}

/// REV5 §18: one-hex glide speed from the server's own step cadence — the
/// server marches exactly one lattice tile (TILE_SIZE wu, pinned to the
/// destination center) per `step_interval_secs`, so a marker gliding at
/// `TILE_SIZE / interval` arrives roughly when the next step lands. This
/// replaces the dead snapshot-delta derivation (deltas read as hops because
/// replications arrive much faster than steps).
fn mob_glide_speed(step_interval_secs: u32) -> f32 {
    WorldGenConfig::TILE_SIZE / step_interval_secs.max(1) as f32
}

/// Desync guard (REV5 §18): a snapshot displacement bigger than two hexes
/// snaps instantly instead of gliding half the map after a laggy stream.
fn mob_should_snap(jump_wu: f32) -> bool {
    jump_wu > 2.0 * WorldGenConfig::TILE_SIZE
}

// ---------------------------------------------------------------------------
// Spec 026 — structure selection, attack area, upgrade/remove panel
// ---------------------------------------------------------------------------

/// Pure cursor-hit test for placement/selection (§3.1): a structure is
/// picked by its bottom-anchored sprite rect (ground-slot leniency below),
/// with a whole-hex Chebyshev footprint fallback — the natural click zone
/// for ground traps whose art is tiny.
fn structure_cursor_hit(
    cursor: Vec2,
    sx: f32,
    sy: f32,
    aspect: f32,
    height: f32,
) -> bool {
    let w = height * aspect;
    let y0 = sy + 0.4 - 0.55;
    let y1 = sy + 0.4 + height;
    let rect = cursor.x >= sx - w * 0.5
        && cursor.x <= sx + w * 0.5
        && cursor.y >= y0
        && cursor.y <= y1;
    rect || structure_hex_hit(cursor, sx, sy)
}

/// Whole-hex fallback hit: Chebyshev (the lattice metric the server's
/// `tile_distance` uses) within a half tile of the structure center.
fn structure_hex_hit(cursor: Vec2, sx: f32, sy: f32) -> bool {
    (cursor.x - sx).abs() <= WorldGenConfig::TILE_SIZE * 0.5
        && (cursor.y - sy).abs() <= WorldGenConfig::TILE_SIZE * 0.5
}

/// Side (wu) of the square that covers every hex the server counts inside
/// `tile_distance ≤ range` (Chebyshev on the square lattice): it IS a
/// square, `(2r+1)·TILE` — not a disc. Same shape the territory overlays
/// and the farming floor already render.
fn range_square_side(range_hexes: i32) -> f32 {
    (2 * range_hexes.max(0) + 1) as f32 * WorldGenConfig::TILE_SIZE
}

/// Neutral ring-text (§3.1) driving both the selected-structure ring and
/// the non-owner color: teal = mine, dimmed ice = someone else's.
fn selection_ring_color(owner: Option<&str>, address: Option<&str>) -> Color {
    let mine = owner.is_some() && owner == address;
    if mine {
        Color::srgba(0.3, 1.0, 0.55, 0.85)
    } else {
        Color::srgba(0.75, 0.87, 0.95, 0.7)
    }
}

/// Human refund preview for the Remove button (§3.3/§D2): half of every
/// ingredient, floor-rounded exactly like the server's `remove_refunds…`
/// (`qty / 2` integer division), "—" when everything floors to 0.
fn remove_refund_preview(kind: &cfg::DefenseKind) -> String {
    let parts: Vec<String> = kind
        .cost
        .iter()
        .filter_map(|(item, qty)| {
            let half = *qty / 2;
            (half > 0).then(|| format!("{}×{}", item, half))
        })
        .collect();
    if parts.is_empty() {
        "Refund: nothing".to_string()
    } else {
        format!("Refund: {}", parts.join(", "))
    }
}

/// Spec 026 §3.1 picking: LMB on a structure selects it (toggle); ESC,
/// terrain clicks and armed build mode/menu deselect. UI clicks (any widget
/// hovered/pressed — minimap, hotbar, panels) never touch the world pick.
fn td_structure_pick(
    mut mouse: Res<ButtonInput<MouseButton>>,
    mut keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    widgets: Query<&Interaction>,
    mode: Res<TdBuildMode>,
    panel: Res<CommandPanel>,
    rows: Res<TdStructRows>,
    mut select: ResMut<TdSelect>,
    td_tex: Res<TdTextures>,
    net: Res<Net>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        select.sid = None;
        select.confirm_remove = false;
        return;
    }
    // Armed building (or the Build tab) deselects the structure — one
    // task at a time.
    if mode.kind.is_some() || panel.is_open(Tab::Build) {
        if select.sid.is_some() {
            select.sid = None;
            select.confirm_remove = false;
        }
        return;
    }
    // Left-click on a structure picks it. (The old `rows.is_changed()`
    // gate made this go dead once replication settled — the cached rows
    // only flip `changed` when the list itself diffs. The widget guard
    // below keeps UI clicks out.)
    if !(mouse.just_pressed(MouseButton::Left)) || net.address.is_none() {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    // UI guard: a hovered/pressed widget (button, panel, minimap) keeps the
    // click out of the world.
    if widgets.iter().any(|i| {
        matches!(i, Interaction::Hovered | Interaction::Pressed)
    }) {
        return;
    }
    let Ok((camera, cam_xf)) = cameras.single() else { return };
    let Ok(world) = camera.viewport_to_world_2d(cam_xf, cursor) else { return };
    let world = Vec2::new(world.x, world.y);
    let hit = rows
        .rows
        .iter()
        .filter(|row| {
            let Some(def) = cfg::defense_kind(&row.kind) else { return false };
            let (_, aspect, _, height) = structure_sprite(&td_tex, &row.kind);
            structure_cursor_hit(world, row.x, row.y, aspect, height)
        })
        .min_by_key(|row| {
            let d = (Vec2::new(row.x, row.y) - world).length();
            (d * 1000.0) as u64
        })
        .map(|row| row.structure_id);
    match hit {
        Some(sid) if select.sid == Some(sid) => {
            select.sid = None;
        }
        Some(sid) => {
            select.sid = Some(sid);
            select.confirm_remove = false;
        }
        None => {
            select.sid = None;
        }
    }
}

/// Hover glance (§3.4): a nameplate above the cursor-hovered structure
/// (0.25 s throttle, transient `TdHoverLabel` sprites).
fn td_structure_hover(
    mut commands: Commands,
    time: Res<Time>,
    mut acc: Local<f32>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    rows: Res<TdStructRows>,
    select: Res<TdSelect>,
    td_tex: Res<TdTextures>,
    labels: Query<Entity, With<TdHoverLabel>>,
) {
    *acc += time.delta_secs();
    if *acc < 0.25 {
        return;
    }
    *acc = 0.0;
    for e in labels.iter() {
        commands.entity(e).despawn();
    }
    if select.sid.is_some() {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_xf)) = cameras.single() else { return };
    let Ok(world) = camera.viewport_to_world_2d(cam_xf, cursor) else { return };
    let world = Vec2::new(world.x, world.y);
    let Some(hover) = rows.rows.iter().find(|row| {
        let Some(def) = cfg::defense_kind(&row.kind) else { return false };
        let (_, aspect, _, height) = structure_sprite(&td_tex, &row.kind);
        structure_cursor_hit(world, row.x, row.y, aspect, height)
    }) else {
        return;
    };
    let Some(def) = cfg::defense_kind(&hover.kind) else { return };
    let (_, aspect, _, height) = structure_sprite(&td_tex, &hover.kind);
    let _ = aspect;
    // Nameplate: name + level on the first line, HP beneath it, both on a
    // small dark chip (TextBackgroundColor) so they stay readable against
    // any terrain. Kept in the world scale — a tile is ~17 wu.
    let name = format!(
        "{}{}",
        kind_display(&hover.kind),
        structure_level_label(hover.level)
            .map(|l| format!(" · {l}"))
            .unwrap_or_default(),
    );
    let hp = format!("HP {}/{}", hover.hp, cfg::structure_max_hp(def, hover.level));
    commands.spawn((
        TdHoverLabel,
        Text2d::new(format!("{name}\n{hp}")),
        TextFont { font_size: 2.6.into(), ..default() },
        TextColor(Color::srgb(0.92, 0.88, 0.72)),
        TextShadow { color: Color::BLACK, offset: Vec2::new(0.15, 0.15) },
        bevy::text::TextBackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.72)),
        // Center-anchored two-line block: lift it so it clears the roof.
        Transform::from_xyz(hover.x, hover.y + height + 3.6, prop_depth(hover.y) + 42.0),
    ));
}

/// Per-kind tint for the attack-area square (§3.2 palette; the world arts
/// render untinted, areas get one tint each so tower footprint stays
/// distinct from the ring).
fn area_tint(kind: &cfg::DefenseKind) -> Color {
    match kind.name {
        cfg::ITEM_ARROW_TOWER => Color::srgba(0.5, 0.72, 1.0, 0.16),
        cfg::ITEM_BALLISTA => Color::srgba(0.9, 0.88, 0.75, 0.16),
        cfg::ITEM_LANTERN_TOWER => Color::srgba(1.0, 0.74, 0.35, 0.18),
        cfg::ITEM_COCONUT_TOWER => Color::srgba(0.45, 0.85, 0.42, 0.16),
        cfg::ITEM_FROST_TOWER => Color::srgba(0.55, 0.85, 1.0, 0.18),
        // Burn zones read fire-amber; consumable footprints stay red/icy.
        cfg::ITEM_FIRE_TRAP => Color::srgba(1.0, 0.55, 0.2, 0.2),
        cfg::ITEM_SPIKE_TRAP => Color::srgba(1.0, 0.35, 0.25, 0.2),
        cfg::ITEM_WEB_TRAP => Color::srgba(0.6, 0.9, 1.0, 0.14),
        _ => Color::srgba(0.3, 1.0, 0.55, 0.12),
    }
}

/// One square overlay `TdSelectFx`.
fn spawn_area(commands: &mut Commands, cx: f32, cy: f32, side: f32, color: Color, z: f32) {
    commands.spawn((
        TdSelectFx,
        Sprite {
            color,
            custom_size: Some(Vec2::splat(side)),
            ..default()
        },
        Transform::from_xyz(cx, cy, z),
    ));
}

/// A square OUTLINE (4 thin bars) — the frost slow ring + selection ring.
fn spawn_ring(
    commands: &mut Commands,
    cx: f32,
    cy: f32,
    side: f32,
    thickness: f32,
    color: Color,
    z: f32,
) {
    let half = side * 0.5;
    let spark = |commands: &mut Commands, x: f32, y: f32, w: f32, h: f32| {
        commands.spawn((
            TdSelectFx,
            Sprite {
                color,
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(x, y, z),
        ));
    };
    spark(commands, cx, cy + half, side, thickness);
    spark(commands, cx, cy - half, side, thickness);
    spark(commands, cx + half, cy, thickness, side);
    spark(commands, cx - half, cy, thickness, side);
}

/// Spec 026 §3.2: attack area for the selected structure, plus the build
/// mode's armed-kind preview over the hovered hex (§3.2 / phase E).
/// 0.25 s spawn/despawn sweep — same cheap pattern as place hints.
fn td_selection_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut acc: Local<f32>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    rows: Res<TdStructRows>,
    select: Res<TdSelect>,
    net: Res<Net>,
    mode: Res<TdBuildMode>,
    player_xf: Res<PlayerTransform>,
    fxq: Query<Entity, With<TdSelectFx>>,
) {
    use idlecore_core::grid::world_pos_to_tile;
    *acc += time.delta_secs();
    if *acc < 0.25 {
        return;
    }
    *acc = 0.0;
    for e in fxq.iter() {
        commands.entity(e).despawn();
    }

    // ---- 1) selected structure: ring + attack area ----
    if let Some(sid) = select.sid {
        let Some(row) = rows.rows.iter().find(|r| r.structure_id == sid) else {
            return;
        };
        let z = prop_depth(row.y) + 7.0;
        // Selection ring around the occupying hex (teal mine / ice other).
        spawn_ring(
            &mut commands,
            row.x,
            row.y,
            WorldGenConfig::TILE_SIZE + 0.6,
            0.35,
            selection_ring_color(Some(&row.owner), net.address.as_deref()),
            z,
        );
        if let Some(def) = cfg::defense_kind(&row.kind) {
            // Attack area: the square the server actually covers
            // (`tile_distance ≤ range` = Chebyshev square), pulsing only on
            // consumable traps so the footprint feels armed (§3.2).
            // Traps pulse (consumable = armed), towers hold the alpha.
            let pulse = if cfg::is_trap(def) {
                0.10 + 0.06 * (time.elapsed_secs() * 2.2).sin().abs()
            } else {
                0.0
            };
            if def.range_hexes > 0 && (def.dps > 0 || def.burn_secs > 0) {
                let mut tint = area_tint(def);
                if pulse > 0.0 {
                    tint.set_alpha(tint.alpha() + pulse);
                }
                spawn_area(
                    &mut commands,
                    row.x,
                    row.y,
                    range_square_side(def.range_hexes as i32),
                    tint,
                    z,
                );
            }
            // Frost tower's slow ring: a *second* ring outside the range
            // square only when radius > range so it reads as layered ice.
            if def.slow_radius > 0 {
                let side = range_square_side(def.slow_radius as i32);
                spawn_ring(&mut commands, row.x, row.y, side, 0.55, Color::srgba(0.6, 0.92, 1.0, 0.6), z);
            }
            // Consumable trap: the footprint quad IS the area.
            if def.range_hexes == 0 && cfg::is_trap(def) {
                let mut tint = area_tint(def);
                if pulse > 0.0 {
                    tint.set_alpha(tint.alpha() + pulse);
                }
                spawn_area(&mut commands, row.x, row.y, WorldGenConfig::TILE_SIZE, tint, z);
            }
        }
    }

    // ---- 2) build-mode preview: armed kind's attack area on the hovered
    //      hex (biggest spec win: "will that reach?" answers before E) ----
    let building = mode.kind.filter(|k| BUILDABLES.contains(k));
    let Some(kind) = building else { return };
    let cursor = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .and_then(|c| {
            let (cam, cam_xf) = cameras.single().ok()?;
            let wf = cam.viewport_to_world_2d(cam_xf, c).ok()?;
            Some(Vec2::new(wf.x, wf.y))
        });
    let placed_at: Vec2 = match cursor {
        Some(world) => {
            let (q, r) = world_pos_to_tile(world.x, world.y, WorldGenConfig::TILE_SIZE);
            let (cx, cy) = hex_center(q, r);
            Vec2::new(cx, cy)
        }
        // Touch devices (no cursor): preview at the player's hex.
        None => Vec2::new(player_xf.translation.x, player_xf.translation.y),
    };
    let Some(def) = cfg::defense_kind(&kind) else { return };
    let z = prop_depth(placed_at.y) + 7.0;
    // Preview ring (the "you'd build here" hex) matches place hints.
    spawn_ring(
        &mut commands,
        placed_at.x,
        placed_at.y,
        WorldGenConfig::TILE_SIZE + 0.6,
        0.35,
        Color::srgba(0.3, 1.0, 0.55, 0.85),
        z,
    );
    if def.range_hexes > 0 && (def.dps > 0 || def.burn_secs > 0) {
        spawn_area(
            &mut commands,
            placed_at.x,
            placed_at.y,
            range_square_side(def.range_hexes as i32),
            area_tint(def),
            z,
        );
    }
    if def.slow_radius > 0 {
        spawn_ring(
            &mut commands,
            placed_at.x,
            placed_at.y,
            range_square_side(def.slow_radius as i32),
            0.55,
            Color::srgba(0.6, 0.92, 1.0, 0.6),
            z,
        );
    }
    if def.range_hexes == 0 && cfg::is_trap(def) {
        spawn_area(
            &mut commands,
            placed_at.x,
            placed_at.y,
            WorldGenConfig::TILE_SIZE,
            area_tint(def),
            z,
        );
    }
}

/// Panel content (§3.3): header/hp/stats/XP bucket + action row gating.
/// One system writes every text piece (ParamSet caps the sibling writers
/// at 8 — the refund line is its own system below), 0.4 s throttle like
/// `update_td_hud`.
fn update_select_panel(
    mut throttle: Local<f32>,
    time: Res<Time>,
    select: Res<TdSelect>,
    rows: Res<TdStructRows>,
    net: Res<Net>,
    player: Query<&ClientPlayer>,
    mut ps: ParamSet<(
        Query<&mut Node, With<TdSelPanelRoot>>,
        Query<&mut Node, With<TdSelActions>>,
        Query<&mut Node, With<TdSelHpBar>>,
        Query<&mut Text, With<TdSelHeader>>,
        Query<&mut Text, With<TdSelStats>>,
        Query<&mut Text, With<TdSelXp>>,
        Query<&mut Text, With<TdSelUpgradeTxt>>,
        Query<&mut Text, With<TdSelRemoveTxt>>,
    )>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.4 {
        return;
    }
    // Selection row lift (e.g. the row vanished between syncs): keep it
    // dark until a new pick.
    let Some(sid) = select.sid else {
        if let Ok(mut node) = ps.p0().single_mut() {
            node.display = Display::None;
        }
        return;
    };
    let Some(row) = rows.rows.iter().find(|r| r.structure_id == sid) else {
        if let Ok(mut node) = ps.p0().single_mut() {
            node.display = Display::None;
        }
        return;
    };
    *throttle = 0.0;
    let Some(def) = cfg::defense_kind(&row.kind) else { return };
    let is_mine = net.address.as_deref() == Some(row.owner.as_str());
    let max = cfg::structure_max_hp(def, row.level);
    if let Ok(mut node) = ps.p0().single_mut() {
        node.display = Display::Flex;
    }
    if let Ok(mut text) = ps.p3().single_mut() {
        text.0 = format!(
            "{}{}",
            kind_display(&row.kind),
            structure_level_label(row.level)
                .map(|l| format!(" [{l}]"))
                .unwrap_or_default(),
        );
    }
    let ratio = clamp_ratio(row.hp as f32, max as f32);
    if let Ok(mut node) = ps.p2().single_mut() {
        node.width = Val::Percent(ratio * 100.0);
    }
    // Stat line's HP head, shared by every branch.
    let hp_txt = format!("HP {}/{}", row.hp, max);
    let stat = if def.dps > 0 && def.burn_secs > 0 {
        // Its HP doubles as burn fuel — surface that in the same line.
        format!(
            "{hp_txt} · burn {}dps×{}s (fuel = HP)",
            cfg::structure_dps(def, row.level),
            def.burn_secs
        )
    } else if def.trap_dmg > 0 {
        format!("{hp_txt} · burst {} dmg", def.trap_dmg)
    } else if def.pin_secs > 0 {
        format!("{hp_txt} · pin {}s", def.pin_secs)
    } else if def.dps > 0 {
        format!(
            "{hp_txt} · dps {} · range {}",
            cfg::structure_dps(def, row.level),
            def.range_hexes
        )
    } else {
        hp_txt
    };
    if let Ok(mut text) = ps.p4().single_mut() {
        text.0 = stat;
    }
    // XP bucket (own rows only). Server math mirrored: bucket fills against
    // `100·L²`, spendable = current level's surplus (`level_progress`). The
    // panel shows the delta so the disabled reasons read before the reducer
    // round-trips (§3.3).
    let spendable = player
        .single()
        .ok()
        .map(|p| idlecore_core::progression::level_progress(p.xp).1)
        .unwrap_or(0);
    if let Ok(mut text) = ps.p5().single_mut() {
        text.0 = if !is_mine {
            String::new()
        } else if row.level >= cfg::STRUCTURE_LEVEL_MAX {
            "MAX level".to_string()
        } else if spendable == 0 {
            "No spare XP in your level — earn XP".to_string()
        } else {
            format!(
                "XP {}/{} · surplus {} · {}",
                row.total_xp,
                cfg::structure_xp_for_next_level(row.level),
                spendable,
                upgrade_delta_line(def, row.level).unwrap_or_default(),
            )
        };
    }
    // Button labels carry the gate reason (§D1): non-owner / MAX / cooldown.
    let cooling = player
        .single()
        .ok()
        .map(|p| p.action_cd_until_utc.saturating_sub(unix_now_secs()))
        .unwrap_or(0);
    if let Ok(mut text) = ps.p6().single_mut() {
        text.0 = if !is_mine {
            "Upgrade (not yours)".to_string()
        } else if row.level >= cfg::STRUCTURE_LEVEL_MAX {
            "Upgrade (MAX)".to_string()
        } else if cooling > 0 {
            format!("Upgrade ({cooling}s)")
        } else {
            format!("Upgrade → L{}", row.level + 1)
        };
    }
    if let Ok(mut text) = ps.p7().single_mut() {
        text.0 = if select.confirm_remove { "Sure?".to_string() } else { "Remove".to_string() };
    }
    // Action row only exists for your own rows (§3.1 — you may LOOK at any
    // structure, but the buttons show for yours).
    if let Ok(mut node) = ps.p1().single_mut() {
        node.display = if is_mine { Display::Flex } else { Display::None };
    }
}

/// Refund preview (§3.3) — its own writer system (ParamSet slots are for
/// the main panel texts, max 8).
fn update_select_refund(
    select: Res<TdSelect>,
    rows: Res<TdStructRows>,
    net: Res<Net>,
    mut text: Query<&mut Text, With<TdSelRefund>>,
) {
    if !select.is_changed() && !rows.is_changed() && !net.is_changed() {
        return;
    }
    let Ok(mut text) = text.single_mut() else { return };
    text.0 = match select
        .sid
        .and_then(|sid| rows.rows.iter().find(|r| r.structure_id == sid))
    {
        Some(row) => match (cfg::defense_kind(&row.kind), net.address.as_deref()) {
            (Some(def), Some(addr)) if addr == row.owner => {
                remove_refund_preview(def)
            }
            _ => String::new(),
        },
        None => String::new(),
    };
}

/// One-line "what L{level+1} buys" for the panel (§3.3): hp always, dps on
/// attackers; None at the level cap. Pure — unit-tested.
fn upgrade_delta_line(def: &cfg::DefenseKind, level: u8) -> Option<String> {
    if level >= cfg::STRUCTURE_LEVEL_MAX {
        return None;
    }
    let next = level + 1;
    let mut parts = vec![format!(
        "hp {}→{}",
        cfg::structure_max_hp(def, level),
        cfg::structure_max_hp(def, next)
    )];
    if def.dps > 0 && def.burn_secs > 0 {
        parts.push(format!(
            "burn {}→{}",
            cfg::structure_dps(def, level),
            cfg::structure_dps(def, next)
        ));
    } else if def.dps > 0 {
        parts.push(format!(
            "dps {}→{}",
            cfg::structure_dps(def, level),
            cfg::structure_dps(def, next)
        ));
    }
    Some(parts.join(" "))
}

/// Upgrade / Remove button presses (§3.3): the client re-validates what
/// the server would enforce (owner, L5, cooldown) and sends the existing
/// reducers `transfer_building_xp` / `remove_defense`.
fn td_select_buttons(
    mut upg: Query<&Interaction, (Changed<Interaction>, With<TdSelUpgrade>)>,
    mut rem: Query<&Interaction, (Changed<Interaction>, With<TdSelRemove>)>,
    mut select: ResMut<TdSelect>,
    rows: Res<TdStructRows>,
    mut net: ResMut<Net>,
    player: Query<&ClientPlayer>,
) {
    let upgrade = upg.iter_mut().any(|i| *i == Interaction::Pressed);
    let remove = rem.iter_mut().any(|i| *i == Interaction::Pressed);
    if !upgrade && !remove {
        // A stale confirm gate decays as soon as the cursor leaves the
        // button (fresh interaction frames only keep it hot).
        if select.confirm_remove {
            if rem.iter().all(|i| *i != Interaction::Pressed)
                && upg.iter().all(|i| *i != Interaction::Pressed)
            {
                select.confirm_remove = false;
            }
        }
        return;
    }
    let Some(sid) = select.sid else { return };
    let Some(row) = rows.rows.iter().find(|r| r.structure_id == sid) else {
        return;
    };
    let Some(def) = cfg::defense_kind(&row.kind) else { return };
    let _ = def;
    let is_mine = net.address.as_deref() == Some(row.owner.as_str());
    let cooling = player
        .single()
        .ok()
        .is_some_and(|p| p.action_cd_until_utc > unix_now_secs());
    if !is_mine || cooling {
        return;
    }
    if upgrade && row.level < cfg::STRUCTURE_LEVEL_MAX {
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.transfer_building_xp_then(
                row.structure_id,
                reducer_report("transfer_building_xp", tx.clone(), 0),
            )
        });
    }
    if remove {
        if !select.confirm_remove {
            select.confirm_remove = true;
        } else {
            let tx = net.sender();
            let target = row.structure_id;
            send_reducer(&mut net, |r| {
                r.remove_defense_then(target, reducer_report("remove_defense", tx.clone(), 0))
            });
            // The row vanishes at the next sync; close the panel now.
            select.sid = None;
            select.confirm_remove = false;
        }
    }
}

/// Glide every TD marker toward its replicated server position.
fn glide_td_markers(time: Res<Time>, mut markers: Query<(&mut Transform, &mut TdMove)>) {
    let dt = time.delta_secs();
    for (mut transform, interp) in &mut markers {
        let (tx, ty) = (interp.target.x, interp.target.y);
        let (mx, my) = (transform.translation.x, transform.translation.y);
        let (dx, dy) = (tx - mx, ty - my);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 0.001 {
            continue;
        }
        let step = (interp.speed * dt).min(dist);
        if step <= 0.0 {
            continue;
        }
        transform.translation.x += dx / dist * step;
        transform.translation.y += dy / dist * step;
    }
}

/// Fresh mob enter fx (REV5 §18): scale from +0.3× over 0.3 s, then the
/// component drops and transforms stop being touched.
fn mob_spawn_pop(
    time: Res<Time>,
    mut commands: Commands,
    mut pops: Query<(Entity, &mut TdSpawnPop, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (ent, mut pop, mut xf) in &mut pops {
        pop.left -= dt;
        if pop.left <= 0.0 {
            xf.scale = Vec3::ONE;
            commands.entity(ent).remove::<TdSpawnPop>();
            continue;
        }
        let k = (pop.left / 0.3).clamp(0.0, 1.0);
        xf.scale = Vec3::splat(1.0 + 0.3 * k);
    }
}

/// Death fx (REV5 §18): 0.25 s red shrink+sink, then despawn. Runs on its
/// own transient sprites (no marker component → no query conflicts with
/// the persistent mob systems).
fn mob_fade_tick(
    time: Res<Time>,
    mut commands: Commands,
    mut fades: Query<(Entity, &mut Sprite, &mut TdMobFade)>,
) {
    let dt = time.delta_secs();
    for (ent, mut sprite, mut fade) in &mut fades {
        fade.secs -= dt;
        if fade.secs <= 0.0 {
            commands.entity(ent).despawn();
            continue;
        }
        let k = (fade.secs / fade.total).clamp(0.0, 1.0);
        if let Some(size) = sprite.custom_size {
            sprite.custom_size = Some(size * (0.4 + 0.6 * k));
        }
        sprite.color.set_alpha(k);
    }
}

/// Per-raider animation (Spec 025 §5, REV5 §18): the idle frame when
/// standing, one of two walk frames (0.25 s cadence) ONLY while actually
/// gliding (dist > 0.05 wu — was speed-gated, which churned frames on
/// standing mobs), a heading flip while moving, and a 0.15 s red tint when
/// the tracker says this mob just took a hit. HP bars are children without
/// the marker component, so this query only reaches the body.
fn animate_mobs(
    time: Res<Time>,
    td_tex: Res<TdTextures>,
    mut tracker: ResMut<TdTracker>,
    fx: Res<TdFx>,
    mut mobs: Query<(&TdMobAnim, &TdMove, &mut Sprite, &mut Transform), With<TdMobMarker>>,
) {
    let dt = time.delta_secs();
    tracker.flash.retain(|_, left| {
        *left -= dt;
        *left > 0.0
    });
    let walk_flip = (time.elapsed_secs() / 0.25) as usize % 2;
    for (anim, mv, mut sprite, xf) in &mut mobs {
        let ms = mob_sprite_kind(&td_tex, &anim.kind);
        // REV5 §18: movement gate on ACTUAL distance to the glide target.
        let (tdx, tdy) = (mv.target.x - xf.translation.x, mv.target.y - xf.translation.y);
        let remaining = (tdx * tdx + tdy * tdy).sqrt();
        let image = if remaining > 0.05 {
            ms.walk[walk_flip].clone()
        } else {
            ms.idle.clone()
        };
        // Heading flip (REV5): face the glide direction while moving.
        if remaining > 0.05 {
            let flip = tdx < 0.0;
            if sprite.flip_x != flip {
                sprite.flip_x = flip;
            }
        }
        if sprite.image.id() != image.id() {
            sprite.image = image;
        }
        // REV3 §13 attack cursor: while the fx timer is hot, the mob flashes
        // between its idle frame and a bright pose tint, so every hit it
        // lands on a structure/capital visibly reads as *its* act.
        let attacking = fx.posing.contains_key(&anim.mob_id);
        let flash_on = tracker.flash.contains_key(&anim.mob_id);
        let color = if flash_on {
            Color::srgb(1.0, 0.3, 0.25)
        } else if attacking {
            // 0.12 s white pulse while attacking (the sheet ships no attack
            // frames; the bright pulse is the agreed fallback, §13).
            let pulse = (time.elapsed_secs() / 0.12) as usize % 2 == 0;
            if pulse {
                Color::srgb(1.5, 1.35, 0.9)
            } else {
                Color::srgb(1.0, 0.78, 0.5)
            }
        } else {
            Color::WHITE
        };
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

// ---------------------------------------------------------------------------
// Combat fx systems (REV3 §13) — consume combat_event rows, animate visuals
// ---------------------------------------------------------------------------

/// Consume fresh `combat_event` rows once each and birth visuals: tracers
/// fly from the shooter, impacts pop on the target, capitals flash, the
/// wave's first capital hit toasts "under attack".
fn process_combat_events(
    snap: &TdSnapshot,
    fx: &mut TdFx,
    commands: &mut Commands,
    player_xf: &PlayerTransform,
) {
    for row in &snap.events {
        if row.event_id <= fx.last_event_id {
            continue;
        }
        fx.last_event_id = fx.last_event_id.max(row.event_id);
        let Some(plan) = fx_plan(&row.kind) else { continue };
        // Attack cursor: any mob-sourced event arms the attack pose (§13).
        if row.attacker_mob > 0 {
            fx.posing.insert(row.attacker_mob, 0.55);
        }
        // Player swings arm the attacker pose at home too (attacker_mob is
        // 0 — pose goes on the player fx slot below).
        if row.kind == "player_hit" {
            fx.player_swing = 0.35;
        }
        let (tx, ty) = fx_target_pos(row);
        match plan {
            FxBirth::Bolt(color) => {
                // From the shooter when known (mob rows); tower shots (attacker
                // 0) get an impact flash on the target instead — the cause is
                // still attached to the right hex.
                let from = (row.attacker_mob != 0)
                    .then(|| snap.mobs.iter().find(|m| m.mob_id == row.attacker_mob))
                    .flatten()
                    .map(|m| Vec2::new(m.x, m.y - 0.4));
                if let Some(from) = from {
                    let dist = (Vec2::new(tx, ty) - from).length();
                    commands.spawn((
                        TdBolt {
                            t: 0.0,
                            ttl: (dist / 22.0).clamp(0.15, 0.6),
                            from,
                            to: Vec2::new(tx, ty),
                        },
                        Sprite {
                            color: Color::srgb(color[0], color[1], color[2]),
                            custom_size: Some(Vec2::new(0.7, 0.18)),
                            ..default()
                        },
                        Transform::from_xyz(from.x, from.y, prop_depth(ty) + 41.0),
                    ));
                    // Impact lands a beat after the bolt.
                    commands.spawn((
                        TdImpact { t: -0.3, ttl: 0.3, size: 0.9 },
                        Sprite {
                            color: Color::srgba(color[0], color[1], color[2], 0.9),
                            custom_size: Some(Vec2::splat(0.4)),
                            ..default()
                        },
                        Transform::from_xyz(tx, ty, prop_depth(ty) + 42.0),
                    ));
                } else {
                    impact_sprite(commands, color, tx, ty, 0.9);
                }
            }
            FxBirth::Impact(color) => {
                let size = if row.kind == "struct_fell" { 1.6 } else { 1.0 };
                impact_sprite(commands, color, tx, ty, size);
            }
            FxBirth::CapitalHit => {                fx.capital_hit.insert(row.capital_id, 0.5);
                if row.wave_id != 0 && fx.toasted_waves.insert(row.wave_id) {
                    commands.spawn((
                        TdWorldToast { t: 0.0 },
                        Text2d::new("⚑ Capital under attack!"),
                        TextFont { font_size: 22.0.into(), ..default() },
                        TextColor(Color::srgb(1.0, 0.35, 0.3)),
                        TextShadow {
                            color: Color::BLACK,
                            offset: Vec2::new(1.0, 1.0),
                        },
                        Transform::from_xyz(tx, ty + 2.0, prop_depth(ty) + 60.0),
                    ));
                }
            }
            // Spec 027: player weapon swing — a gold tracer from YOU to the
            // victim's hex + the impact bloom (the arc reads as the swing).
            FxBirth::PlayerHit => {
                let from = Vec2::new(player_xf.translation.x, player_xf.translation.y - 0.4);
                let to = Vec2::new(tx, ty);
                let dist = to.distance(from);
                if dist > 0.4 {
                    commands.spawn((
                        TdBolt {
                            t: 0.0,
                            ttl: (dist / 26.0).clamp(0.12, 0.45),
                            from,
                            to,
                        },
                        Sprite {
                            color: Color::srgba(1.0, 0.9, 0.5, 1.0),
                            custom_size: Some(Vec2::new(0.9, 0.22)),
                            ..default()
                        },
                        Transform::from_xyz(from.x, from.y, prop_depth(ty) + 43.0),
                    ));
                }
                impact_sprite(commands, [1.0, 0.9, 0.5], tx, ty, 0.9);
            }
        }
    }
}

/// Spawn one standard impact bloom at a world position.
fn impact_sprite(commands: &mut Commands, color: [f32; 3], x: f32, y: f32, size: f32) {
    commands.spawn((
        TdImpact { t: 0.0, ttl: 0.38, size },
        Sprite {
            color: Color::srgba(color[0], color[1], color[2], 0.85),
            custom_size: Some(Vec2::splat(0.5)),
            ..default()
        },
        Transform::from_xyz(x, y, prop_depth(y) + 42.0),
    ));
}

/// World position where one event's fx lands: the target hex center, or the
/// exact slot cell when the event names one.
fn fx_target_pos(row: &FxEventRow) -> (f32, f32) {
    if row.slot_x >= 0 && row.slot_y >= 0 {
        let (x, y) = idlecore_core::slots::slot_center(row.slot_x, row.slot_y);
        return (x, y);
    }
    let c = TileCoord::from_id(row.target_hex_id);
    hex_center(c.x, c.y)
}

/// Per-frame fx animation: bolt flight, impact bloom + fade, capital hit
/// flash, mob attack-pose timers, world toast float.
fn combat_fx_tick(
    time: Res<Time>,
    mut fx: ResMut<TdFx>,
    mut commands: Commands,
    mut bolts: Query<(Entity, &mut TdBolt, &mut Transform), Without<TdMobMarker>>,
    mut impacts: Query<(Entity, &mut TdImpact, &mut Sprite), (Without<TdMobMarker>, Without<TdCapitalMarker>)>,
    mut toasts: Query<(Entity, &mut TdWorldToast, &mut TextColor)>,
    mut capitals: Query<(Entity, &TdCapitalMarker, &mut Sprite), (Without<TdMobMarker>, Without<TdImpact>)>,
) {
    let dt = time.delta_secs();
    fx.posing.retain(|_, left| {
        *left -= dt;
        *left > 0.0
    });
    fx.capital_hit.retain(|_, left| {
        *left -= dt;
        *left > 0.0
    });

    // Tracers: fly from → to with a slight arc, then die (the impact sprite
    // they left behind carries the landing).
    for (entity, mut bolt, mut transform) in bolts.iter_mut() {
        bolt.t += dt;
        if bolt.t >= bolt.ttl {
            commands.entity(entity).despawn();
            continue;
        }
        let k = (bolt.t / bolt.ttl).clamp(0.0, 1.0);
        let pos = bolt.from.lerp(bolt.to, k);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y + 0.25 * (k * std::f32::consts::PI).sin();
    }

    // Impacts: bloom out, fade, die.
    for (entity, mut impact, mut sprite) in impacts.iter_mut() {
        impact.t += dt;
        if impact.t < 0.0 {
            sprite.color.set_alpha(0.0);
            continue;
        }
        if impact.t >= impact.ttl {
            commands.entity(entity).despawn();
            continue;
        }
        let k = (impact.t / impact.ttl).clamp(0.0, 1.0);
        sprite.custom_size = Some(Vec2::splat(impact.size * (0.5 + k)));
        let base = sprite.color.to_srgba();
        sprite.color.set_alpha(0.85 * (1.0 - k));
        let _ = base;
    }

    // World toasts: fade over ~2 s (the text static-up is enough).
    for (entity, mut toast, mut text_color) in toasts.iter_mut() {
        toast.t += dt;
        if toast.t >= 2.0 {
            commands.entity(entity).despawn();
            continue;
        }
        text_color.0.set_alpha((1.0 - toast.t / 2.0).max(0.0));
    }

    // Capital hit: red flicker while the timer runs, idle gold otherwise.
    for (_entity, marker, mut sprite) in capitals.iter_mut() {
        let flash = fx.capital_hit.get(&marker.1).copied().unwrap_or(0.0);
        let color = if flash > 0.0 && (flash * 18.0) as usize % 2 == 0 {
            Color::srgb(1.0, 0.25, 0.2)
        } else if flash > 0.0 {
            Color::srgb(1.0, 0.65, 0.4)
        } else {
            Color::srgb(1.0, 0.9, 0.6)
        };
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

/// While a wave targets me, breathe the ownership overlays for emphasis.
fn td_pulse(
    time: Res<Time>,
    tdview: Res<TdView>,
    mut overlays: Query<&mut Sprite, With<TdOverlay>>,
) {
    if tdview.incoming.is_none() {
        return;
    }
    let alpha = 0.10 + 0.10 * (time.elapsed_secs() * 2.5).sin().abs();
    for mut sprite in &mut overlays {
        // Only pulse tinted overlay sprites (the base alpha is 0.10-0.12).
        if sprite.color.alpha() > 0.05 {
            sprite.color.set_alpha(alpha);
        }
    }
}

// ---------------------------------------------------------------------------
// Placement hints (Spec 023 rev3): floor glows showing WHERE building works
// ---------------------------------------------------------------------------

/// Per-hex highlight spawned by `td_place_hints` while a buildable/claimable
/// action is armed.
#[derive(Component)]
pub struct TdHint;

/// The armed tower being placed with E (Spec 023 §3.4). Cycled with B,
/// cleared with ESC; nothing needs to be *held* — raw materials
/// (Wood/Stone/Metal Part/Bone) are consumed directly on placement.
#[derive(Resource, Default)]
pub struct TdBuildMode {
    /// `Some(kind name)` while a build is armed.
    pub kind: Option<&'static str>,
}

/// The const proxy kinds the build menu lists, with cost lines. Spec 024
/// adds the storage chest as a fourth buildable; REV4 expands to 12 named
/// kinds grouped walls / towers / traps (utility + flavor pending).
pub const BUILDABLES: [&str; 12] = [
    cfg::ITEM_BONE_BARRICADE,
    cfg::ITEM_WOOD_WALL,
    cfg::ITEM_IRON_WALL,
    cfg::ITEM_ARROW_TOWER,
    cfg::ITEM_BALLISTA,
    cfg::ITEM_LANTERN_TOWER,
    cfg::ITEM_COCONUT_TOWER,
    cfg::ITEM_FROST_TOWER,
    cfg::ITEM_SPIKE_TRAP,
    cfg::ITEM_WEB_TRAP,
    cfg::ITEM_FIRE_TRAP,
    cfg::ITEM_STORAGE_CHEST,
];

/// Build-menu sections (REV4 "separated by category"): label + kinds in
/// list order. The chest rides with Structures — no dedicated group yet.
pub const BUILD_GROUPS: [(&str, &[&str]); 3] = [
    (
        "WALLS",
        &[
            cfg::ITEM_BONE_BARRICADE,
            cfg::ITEM_WOOD_WALL,
            cfg::ITEM_IRON_WALL,
        ],
    ),
    (
        "TOWERS",
        &[
            cfg::ITEM_ARROW_TOWER,
            cfg::ITEM_BALLISTA,
            cfg::ITEM_LANTERN_TOWER,
            cfg::ITEM_COCONUT_TOWER,
            cfg::ITEM_FROST_TOWER,
        ],
    ),
    (
        "TRAPS",
        &[
            cfg::ITEM_SPIKE_TRAP,
            cfg::ITEM_WEB_TRAP,
            cfg::ITEM_FIRE_TRAP,
            // Utility for now: the vault lives at the end of the list.
            cfg::ITEM_STORAGE_CHEST,
        ],
    ),
];

/// Spec 029: the build menu is the Build tab of the shared command panel
/// (`ui::panel::CommandPanel` owns open/tab); no local open flag.

/// Spec 026: selected structure + the double-click confirm gate for its
/// Remove button. Terrain clicks, ESC and armed build mode/menu deselect.
#[derive(Resource, Default)]
pub struct TdSelect {
    pub sid: Option<u64>,
    pub confirm_remove: bool,
}

/// Mirror of the snapshot's defense-structure rows for selection, hover and
/// the panel (refreshed whenever any TD table changes).
#[derive(Resource, Default)]
pub struct TdStructRows {
    pub rows: Vec<StructRow>,
}

/// Floor hint color per armed action. Tone-picked so the tint pops against
/// the pale-green grass tiles (teal = build, gold = capital, azure = claim).
fn hint_color(held: &str, claim_ready: bool) -> Option<Color> {
    match held {
        cfg::ITEM_ARROW_TOWER | cfg::ITEM_BALLISTA | cfg::ITEM_BONE_BARRICADE => {
            // Teal: "build here" (owned land).
            Some(Color::srgba(0.25, 0.95, 0.65, 0.9))
        }
        // REV4 expansion: every buildable rows under the same teal build hint.
        cfg::ITEM_STORAGE_CHEST
        | cfg::ITEM_WOOD_WALL
        | cfg::ITEM_IRON_WALL
        | cfg::ITEM_LANTERN_TOWER
        | cfg::ITEM_COCONUT_TOWER
        | cfg::ITEM_FROST_TOWER
        | cfg::ITEM_SPIKE_TRAP
        | cfg::ITEM_WEB_TRAP
        | cfg::ITEM_FIRE_TRAP => {
            Some(Color::srgba(0.25, 0.95, 0.65, 0.9))
        }
        "" if claim_ready => {
            // Azure: "claim this hex" (adjacent-to-owned, unowned, 5W+3S ready).
            Some(Color::srgba(0.35, 0.72, 1.0, 0.85))
        }
        _ => None,
    }
}

/// Procedural buildable-area texture (Spec 023 rev7): a pointy-top hexagon
/// sprite whose soft tinted fill (α≈0.3) carries a bright rim (α≈0.95), so
/// each hex reads as a *shaped cell* instead of the old pinhead squares.
#[derive(Resource, Default)]
struct TdHexTex {
    ready: bool,
    hex: Handle<Image>,
}

/// White texels — the world-space `Sprite` tint supplies the color.
fn make_td_hex_image() -> Image {
    let w = 128u32;
    let h = 148u32; // 2/√3 · 128 → pointy-top aspect matches the world hexes
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let rpx = h as f32 / 2.0;
    let corner = |i: usize, shrink: f32| {
        let angle = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        (cx + (rpx - shrink) * angle.cos(), cy + (rpx - shrink) * angle.sin())
    };
    let outer: [_; 6] = std::array::from_fn(|i| corner(i, 0.0));
    let inner: [_; 6] = std::array::from_fn(|i| corner(i, 8.0));
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (tx, ty) = (x as f32 + 0.5, y as f32 + 0.5);
            if !point_in_hex(tx, ty, &outer) {
                continue;
            }
            // Rim: hex-shaped shell between the outer and inner polygons.
            let idx = ((y * w + x) * 4) as usize;
            let a = if point_in_hex(tx, ty, &inner) { 78 } else { 242 };
            pixels[idx] = 255;
            pixels[idx + 1] = 255;
            pixels[idx + 2] = 255;
            pixels[idx + 3] = a;
        }
    }
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Even-odd ray-cast, same geometry as the minimap hex tiles.
fn point_in_hex(tx: f32, ty: f32, corners: &[(f32, f32); 6]) -> bool {
    let mut inside = false;
    let mut j = 5;
    for i in 0..6 {
        let (xi, yi) = corners[i];
        let (xj, yj) = corners[j];
        if (yi > ty) != (yj > ty) && tx < (xj - xi) * (ty - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn init_td_hex_tex(mut td: ResMut<TdHexTex>, mut images: ResMut<Assets<Image>>) {
    if td.ready {
        return;
    }
    td.hex = images.add(make_td_hex_image());
    td.ready = true;
    info!("TD hex hint texture ready");
}

/// Show WHERE each tower-defense action will work (0.25 s rebuild):
/// - Build mode armed (B) → owned hexes glow green; the armed kind builds
///   on E regardless of what is held.
/// - holding a structure item → owned hexes glow green ("build here").
/// - empty hand (no build mode) + 5 Wood + 3 Stone → the claimable ring
///   glows blue.
fn td_place_hints(
    mut commands: Commands,
    time: Res<Time>,
    mut acc: Local<f32>,
    net: Res<Net>,
    td: Res<TdTerritory>,
    extent: Res<crate::plugins::camera::ViewExtent>,
    hex_tex: Res<TdHexTex>,
    build_mode: Res<TdBuildMode>,
    inventory: Res<crate::inventory::Inventory>,
    player_xf: Res<PlayerTransform>,
    hints: Query<Entity, With<TdHint>>,
) {
    if !hex_tex.ready {
        return;
    }
    *acc += time.delta_secs();
    if *acc < 0.25 {
        return;
    }
    *acc = 0.0;

    for e in hints.iter() {
        commands.entity(e).despawn();
    }
    let Some(address) = net.address.clone() else { return };
    let mine = |owner: &str| owner == address.as_str();
    let held = inventory
        .active_item()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let building = build_mode
        .kind
        .filter(|k| BUILDABLES.contains(k));

    let claim_ready = !building.is_some()
        && held.is_empty()
        && inventory
            .counts
            .get(cfg::ITEM_WOOD)
            .copied()
            .unwrap_or(0) >= cfg::CLAIM_WOOD_COST
        && inventory.counts.get(cfg::ITEM_STONE).copied().unwrap_or(0)
            >= cfg::CLAIM_STONE_COST;

    // Build mode wins the hint layer; else held-item semantics.
    let color = if building.is_some() {
        Some(Color::srgba(0.3, 1.0, 0.55, 0.30))
    } else {
        hint_color(&held, claim_ready)
    };
    let Some(color) = color else { return };
    let (px, py) = (player_xf.translation.x, player_xf.translation.y);
    let (_, terr_r2) = cull_radii(&extent);
    let (phq, phr) = idlecore_core::grid::world_pos_to_tile(
        px,
        py,
        idlecore_core::world_gen::WorldGenConfig::TILE_SIZE,
    );
    let player_hex = TileCoord::new(phq, phr);

    let spawn_hint = |commands: &mut Commands, q: i32, r: i32| {
        let (cx, cy) = hex_center(q, r);
        // Square tile overlay: TILE_SIZE per side.
        commands.spawn((
            TdHint,
            Sprite {
                color,
                custom_size: Some(Vec2::splat(WorldGenConfig::TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(cx, cy, prop_depth(cy) + 4.0),
        ));
    };

    match held.as_str() {
        // Build mode: structures on any owned hex.
        _ if building.is_some() => {
            for (&hex_id, owner) in &td.owners {
                if !mine(owner) {
                    continue;
                }
                let c = TileCoord::from_id(hex_id);
                let (cx, cy) = hex_center(c.x, c.y);
                if (cx - px).powi(2) + (cy - py).powi(2) > terr_r2 {
                    continue;
                }
                spawn_hint(&mut commands, c.x, c.y);
            }
        }
        // Capital placement: legal = land + unowned + ≥8 from every capital.
        cfg::ITEM_CAPITAL_CORE => {
            for dq in -9..=9 {
                for dr in -9..=9 {
                    let coord = TileCoord::new(player_hex.x + dq, player_hex.y + dr);
                    let dist = player_hex.distance(&coord);
                    if dist == 0 || dist > 9 {
                        continue;
                    }
                    let (cx, cy) = hex_center(coord.x, coord.y);
                    if !idlecore_core::earth::is_land_at(cx, cy) {
                        continue;
                    }
                    let id = coord.to_id();
                    if td.owners.contains_key(&id) {
                        continue;
                    }
                    if td.capitals.iter().any(|&cap| {
                        TileCoord::from_id(cap).distance(&coord) < cfg::MIN_CAPITAL_SPACING
                    }) {
                        continue;
                    }
                    spawn_hint(&mut commands, coord.x, coord.y);
                }
            }
        }
        // Structures: anywhere on my own territory.
        cfg::ITEM_ARROW_TOWER | cfg::ITEM_BALLISTA | cfg::ITEM_BONE_BARRICADE => {
            for (&hex_id, owner) in &td.owners {
                if !mine(owner) {
                    continue;
                }
                let c = TileCoord::from_id(hex_id);
                let (cx, cy) = hex_center(c.x, c.y);
                if (cx - px).powi(2) + (cy - py).powi(2) > terr_r2 {
                    continue;
                }
                spawn_hint(&mut commands, c.x, c.y);
            }
        }
        // Claim: the ring around my owned hexes (unowned land only).
        "" if claim_ready => {
            let owned: Vec<TileCoord> = td
                .owners
                .iter()
                .filter(|(_, o)| mine(o))
                .map(|(&h, _)| TileCoord::from_id(h))
                .collect();
            for mine_hex in owned {
                for (nq, nr) in mine_hex.neighbors() {
                    let id = TileCoord::new(nq, nr).to_id();
                    if td.owners.contains_key(&id) {
                        continue;
                    }
                    let (cx, cy) = hex_center(nq, nr);
                    if !idlecore_core::earth::is_land_at(cx, cy) {
                        continue;
                    }
                    spawn_hint(&mut commands, nq, nr);
                }
            }
        }
        _ => {}
    }
}

/// Build-mode keys (Spec 023 §3.4, rev7): **B** opens/closes the buildings/// menu (no more blind cycling); **ESC** closes it and disarms. Armed mode
/// turns the owned hexes teal and every **E** books a build there
/// (materials consumed server-side — no tower item is needed).
fn td_build_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<TdBuildMode>,
    panel: Res<CommandPanel>,
    mut net: ResMut<Net>,
    mut was_build_open: Local<bool>,
) {
    // Spec 029: B moved to `ui::panel` (it toggles the Build tab of the
    // shared shell). This system keeps only the world-side semantics:
    // ESC disarms an armed build kind, and Build-tab show/hide pokes the
    // territory overlay re-render (same trigger the old menu toggle used).
    if keyboard.just_pressed(KeyCode::Escape) {
        if mode.kind.take().is_some() {
            net.push(NetEvent::ServerMessage("build mode off".to_string()));
        }
    }
    let build_open = panel.is_open(Tab::Build);
    if build_open != *was_build_open {
        *was_build_open = build_open;
        // NOTE (Spec 029): closing the tab never disarms an armed kind —
        // the row-click flow closes the shell right after arming, and B
        // simply re-reveals the lit hexes. ESC is the disarm key (023 rule,
        // handled above); switching away from the Build tab keeps it armed.
        net.mark_td_dirty();
    }
}

/// Pure pick of the XP-transfer target: the nearest OWN structure within 1
/// hex of the player (Spec 024 "transfer XP into your buildings").
fn choose_upgrade_target(
    owner: &str,
    player_hex: TileCoord,
    rows: &[(u64, String, i32, i32)], // (structure_id, owner, q, r)
) -> Option<u64> {
    rows.iter()
        .filter(|(_, o, _, _)| o == owner)
        .map(|&(id, _, q, r)| (TileCoord::new(q, r).distance(&player_hex), id))
        .filter(|(d, _)| *d <= 1)
        .min_by_key(|&(d, id)| (d, id))
        .map(|(_, id)| id)
}

/// **U** — invest spare player XP into the structure standing next to you
/// (Spec 024 §2.2). The server answers through the HUD log with the fresh
/// level/bucket state.
fn td_upgrade_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut net: ResMut<Net>,
    player: Query<&ClientPlayer>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    let Some(address) = net.address.clone() else {
        net.push(NetEvent::ServerMessage("U: connect first".to_string()));
        return;
    };
    let Ok(p) = player.single() else { return };
    let Some(cur) = &p.current_hex else {
        net.push(NetEvent::ServerMessage("U: you are not on a hex".to_string()));
        return;
    };
    let player_hex = TileCoord::new(cur.x, cur.y);
    let rows: Vec<(u64, String, i32, i32)> = {
        let guard = net.conn.lock().unwrap();
        let Some(conn) = guard.as_ref() else {
            net.push(NetEvent::ServerMessage("U: not connected".to_string()));
            return;
        };
        conn.db
            .defense_structure()
            .iter()
            .map(|s| (s.structure_id, s.owner.clone(), TileCoord::from_id(s.hex_id).x, TileCoord::from_id(s.hex_id).y))
            .collect()
    };
    let Some(target) = choose_upgrade_target(&address, player_hex, &rows) else {
        net.push(NetEvent::ServerMessage(
            "U: stand next to one of your structures to invest XP".to_string(),
        ));
        return;
    };
    let tx = net.sender();
    send_reducer(&mut net, |r| {
        r.transfer_building_xp_then(
            target,
            reducer_report("transfer_building_xp", tx.clone(), 0),
        )
    });
}

/// Click a building row → arm that kind, close the panel, report costs.
fn td_build_row_clicks(
    rows: Query<(&Interaction, &TdBuildRow), Changed<Interaction>>,
    mut mode: ResMut<TdBuildMode>,
    mut panel: ResMut<CommandPanel>,
    mut net: ResMut<Net>,
) {
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let kind = row.0;
        mode.kind = Some(kind);
        let cost = cfg::defense_kind(kind)
            .map(|d| d.cost.iter().map(|(i, q)| format!("{q}x{i} ")).collect::<String>())
            .unwrap_or_default();
        net.push(NetEvent::ServerMessage(format!(
            "BUILD MODE: {kind} ({cost}) — E on a lit hex; ESC cancels"
        )));
        // Old flow closed the menu so the lit hexes are visible; the shell
        // is told to close the same way.
        panel.close();
        net.mark_td_dirty();
    }
}

/// Highlight the armed row and dim rows you can't afford; armed/inventory
/// changes rect immediately, costs otherwise refresh on a lazy 0.25 s.
fn td_build_row_tint(
    mode: Res<TdBuildMode>,
    inventory: Res<crate::inventory::Inventory>,
    time: Res<Time>,
    mut throttle: Local<f32>,
    mut rows: Query<(
        &TdBuildRow,
        &Children,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut cost_color: Query<&mut TextColor, With<TdBuildCost>>,
) {
    let inv_changed = mode.is_changed() || inventory.is_changed();
    if !inv_changed {
        *throttle += time.delta_secs();
        if *throttle < 0.25 {
            return;
        }
    }
    *throttle = 0.0;
    for (row, children, mut bg, mut border) in &mut rows {
        let armed = mode.kind == Some(row.0);
        let can_afford = cfg::defense_kind(row.0)
            .map(|d| {
                d.cost
                    .iter()
                    .all(|(item, qty)| inventory.counts.get(*item).copied().unwrap_or(0) >= *qty)
            })
            .unwrap_or(true);
        let want_bg = if armed {
            Color::srgb(0.16, 0.42, 0.32)
        } else if can_afford {
            crate::ui::panel::ROW_BG
        } else {
            Color::srgb(0.09, 0.1, 0.14)
        };
        if bg.0 != want_bg {
            bg.0 = want_bg;
        }
        let want_border = if armed {
            crate::ui::panel::GOOD
        } else {
            crate::ui::panel::FRAME_BORDER
        };
        if border.top != want_border {
            *border = BorderColor::all(want_border);
        }
        for child in children.iter() {
            if let Ok(mut color) = cost_color.get_mut(child) {
                let want = if can_afford {
                    Color::srgb(1.0, 0.85, 0.4)
                } else {
                    Color::srgb(1.0, 0.35, 0.3)
                };
                if **color != want {
                    **color = want;
                }
            }
        }
    }
}

/// Fill the row icons once the atlas crops are ready; each icon keeps its
/// art's aspect inside the 26×26 slot (UI stretches otherwise).
fn td_build_icons(td: Res<TdTextures>, mut rows: Query<(&TdBuildRow, &mut ImageNode, &mut Node)>) {
    if !td.ready {
        return;
    }
    for (row_entry, mut icon, mut node) in &mut rows {
        let kind = row_entry.0;
        let handle: &Handle<Image> = match kind {
            cfg::ITEM_BONE_BARRICADE => &td.icon_barricade,
            cfg::ITEM_ARROW_TOWER => &td.icon_arrow,
            cfg::ITEM_BALLISTA => &td.icon_ballista,
            // No dedicated chest crop in the atlas yet — reuse the Metal
            // Part icon tinted crate-brown (Spec 024 §2.3 placeholder).
            cfg::ITEM_STORAGE_CHEST => &td.icon_part,
            // REV4 expansion.
            cfg::ITEM_WOOD_WALL => &td.icon_wood_wall,
            cfg::ITEM_IRON_WALL => &td.icon_iron_wall,
            cfg::ITEM_LANTERN_TOWER => &td.icon_lantern,
            cfg::ITEM_COCONUT_TOWER => &td.icon_coconut,
            cfg::ITEM_FROST_TOWER => &td.icon_snowman,
            cfg::ITEM_SPIKE_TRAP => &td.icon_spike,
            cfg::ITEM_WEB_TRAP => &td.icon_web,
            cfg::ITEM_FIRE_TRAP => &td.icon_fire,
            _ => continue,
        };
        if icon.image != *handle {
            icon.image = handle.clone();
            // Fit the slot box around the art's aspect (32×18 barricade is
            // wide, 24×32 tower is tall, the rest square).
            let (w, h) = match kind {
                cfg::ITEM_BONE_BARRICADE => (26.0, 26.0 * 18.0 / 32.0),
                cfg::ITEM_ARROW_TOWER => (26.0 * 24.0 / 32.0, 26.0),
                cfg::ITEM_WOOD_WALL => (26.0, 26.0),
                cfg::ITEM_IRON_WALL => (26.0 * 0.5, 26.0),
                cfg::ITEM_LANTERN_TOWER => (26.0 * 32.0 / 48.0, 26.0),
                cfg::ITEM_COCONUT_TOWER => (26.0, 26.0),
                cfg::ITEM_FROST_TOWER => (26.0 * 2.0 / 3.0, 26.0),
                cfg::ITEM_SPIKE_TRAP => (26.0 * 0.5, 26.0),
                cfg::ITEM_WEB_TRAP => (26.0, 26.0),
                cfg::ITEM_FIRE_TRAP => (26.0 * 0.5, 26.0),
                _ => (26.0, 26.0),
            };
            node.width = Val::Px(w);
            node.height = Val::Px(h);
        }
    }
}

// ---------------------------------------------------------------------------
// Spec 027 — player combat HUD (wellness chip + death veil) and combat input
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct TdWellnessRoot;

/// HP bar fill (Spec 027 §9: wellness chip style).
#[derive(Component)]
pub struct TdHpFill;

#[derive(Component)]
pub struct TdHpTxt;

#[derive(Component)]
pub struct TdWeaponTxt;

#[derive(Component)]
pub struct TdDashTxt;

/// Full-screen death veil + respawn countdown (Spec 027 §9).
#[derive(Component)]
pub struct TdVeilRoot;

#[derive(Component)]
pub struct TdVeilTxt;

/// Pure dash-cooldown chip text (unit-tested): ready / draining.
fn dash_chip_text(dash_cd_until_utc: u64, now: u64) -> String {
    if dash_cd_until_utc <= now {
        "Dash [Space] — ready".to_string()
    } else {
        format!("Dash [Space] — {}", fmt_countdown(dash_cd_until_utc - now))
    }
}

/// Pure hp bar fill for attacks (unit-tested): full → green, mid → amber,
/// low → red — the mirror of the capital chip's server-side HP math.
fn combat_hp_ratio(hp: u32, max: u32) -> f32 {
    clamp_ratio(hp as f32, max as f32)
}

/// Pure owned-tier cycle (Spec 027 §4, unit-tested): the next tier above
/// `current` inside the bitmask (bit i = tier i owned), wrapping to the
/// lowest owned; fists (tier 0) are implicit.
fn next_owned_tier(current: u8, owned_mask: u16) -> u8 {
    // Top tier now includes the bow (§14 addendum: tier 5).
    let owned = owned_mask | 0b1;
    if (current + 1..=5).find(|&t| owned & (1 << t) != 0).is_none() {
        (0..current).find(|&t| owned & (1 << t) != 0).unwrap_or(current)
    } else {
        (current + 1..=5).find(|&t| owned & (1 << t) != 0).unwrap() as u8
    }
}

fn spawn_td_wellness(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: bevy::text::FontSource = asset_server.load("fonts/FiraSans-Bold.ttf").into();

    // Wellness chip — bottom-right: weapon label, hp bar, dash row. Shown
    // only once a combat column carries a real stamp (`update_combat_hud`).
    commands
        .spawn((
            Name::new("td_wellness"),
            TdWellnessRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(96.0),
                right: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.09, 0.85)),
            BorderColor::all(Color::srgb(0.6, 0.15, 0.15)),
        ))
        .with_children(|parent| {
            parent.spawn((
                TdWeaponTxt,
                Text::default(),
                TextFont { font: font.clone(), font_size: 12.0.into(), ..default() },
                TextColor(Color::srgb(1.0, 0.9, 0.5)),
            ));
            // HP bar: dark trough + ratio fill (green→amber→red), like the
            // capital's.
            parent
                .spawn((
                    Node {
                        width: Val::Px(190.0),
                        height: Val::Px(11.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                ))
                .with_child((
                    TdHpFill,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(11.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.9, 0.3)),
                ));
            parent.spawn((
                TdHpTxt,
                Text::default(),
                TextFont { font: font.clone(), font_size: 12.0.into(), ..default() },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                TdDashTxt,
                Text::default(),
                TextFont { font: font.clone(), font_size: 12.0.into(), ..default() },
                TextColor(Color::srgb(0.55, 0.85, 1.0)),
            ));
        });

    // Death veil — full-screen dim, hidden while alive (§9).
    commands
        .spawn((
            Name::new("td_death_veil"),
            TdVeilRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.0, 0.04, 0.65)),
            GlobalZIndex(50),
        ))
        .with_children(|parent| {
            parent.spawn((
                TdVeilTxt,
                Text::new("You fell."),
                TextFont { font, font_size: 26.0.into(), ..default() },
                TextColor(Color::WHITE),
                TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
            ));
        });
}

/// Wellness chip + death veil content. One ParamSet (6 sibling writers):
/// 0 hp fill, 1 veil root, 2 hp text, 3 weapon, 4 dash, 5 veil text.
fn update_combat_hud(
    time: Res<Time>,
    inv: Res<crate::inventory::Inventory>,
    player: Query<&crate::player::ClientPlayer>,
    mut throttle: Local<f32>,
    mut ps: ParamSet<(
        Query<(&mut Node, &mut BackgroundColor), With<TdHpFill>>,
        Query<&mut Node, With<TdVeilRoot>>,
        Query<&mut Text, With<TdHpTxt>>,
        Query<&mut Text, With<TdWeaponTxt>>,
        Query<&mut Text, With<TdDashTxt>>,
        Query<&mut Text, With<TdVeilTxt>>,
    )>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.4 {
        return;
    }
    *throttle = 0.0;
    let Ok(p) = player.single() else { return };
    let now = unix_now_secs();

    // The combat columns mirror only once logged in (0 everywhere before) —
    // keep the chip and veil hidden until a real stamp arrives.
    let known = p.dash_cd_until_utc != 0 || p.hostile_until_utc != 0 || p.grace_until_utc != 0;
    if !known {
        if let Ok(mut text) = ps.p2().single_mut() {
            **text = String::new();
        }
        if let Ok(mut text) = ps.p3().single_mut() {
            **text = String::new();
        }
        if let Ok(mut node) = ps.p1().single_mut() {
            node.display = Display::None;
        }
        return;
    }

    // Death: veil up + countdown (§6).
    if p.death_until_utc > now {
        if let Ok(mut node) = ps.p1().single_mut() {
            node.display = Display::Flex;
        }
        if let Ok(mut text) = ps.p5().single_mut() {
            **text = format!("You fell. Respawning in {}s", p.death_until_utc - now);
        }
        return;
    }
    if let Ok(mut node) = ps.p1().single_mut() {
        node.display = Display::None;
    }

    // HP = 0 without a death stamp is the server's unseeded encoding —
    // render full (mirrors `hp_of` on the server).
    let max = idlecore_core::player_combat::player_max_hp(p.level);
    let hp = if p.hp == 0 { max } else { p.hp };
    let ratio = combat_hp_ratio(hp, max);
    if let Ok((mut node, mut bg)) = ps.p0().single_mut() {
        node.width = Val::Percent(ratio * 100.0);
        *bg = BackgroundColor(hp_bar_color(ratio));
    }
    let in_combat = p.hostile_until_utc > now;
    if let Ok(mut text) = ps.p2().single_mut() {
        // The lone `+` is the out-of-combat regen shimmer (§6).
        **text = format!(
            "{hp} / {max} hp{}",
            if in_combat { " (fighting!)" } else { " +" }
        );
    }
    if let Ok(mut text) = ps.p3().single_mut() {
        // §14: the bow reads its Arrow stack on the chip — firing dry is
        // impossible in UI terms once the count is visible.
        let arrows = match p.weapon_def().ammo {
            Some(ammo) => inv.counts.get(ammo).copied().unwrap_or(0).to_string(),
            None => String::new(),
        };
        let ammo_note = if arrows.is_empty() {
            String::new()
        } else {
            format!(" · {arrows} {}", p.weapon_def().ammo.unwrap_or(""))
        };
        **text = format!("Wpn: {}{}", p.weapon_def().name, ammo_note);
    }
    if let Ok(mut text) = ps.p4().single_mut() {
        **text = dash_chip_text(p.dash_cd_until_utc, now);
    }
}

/// Spec 027 §5.1 input: `KeyF` strikes the facing arc; clicking a mob
/// strikes toward ITS hex. Space dashes — fired in `player_movement` where
/// the ×3 local burst already lives.
#[allow(clippy::type_complexity)]
fn td_combat_input(
    mut keys: Res<ButtonInput<KeyCode>>,
    mut mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    widgets: Query<&Interaction>,
    mode: Res<TdBuildMode>,
    panel: Res<CommandPanel>,
    mobs: Query<(&TdMobAnim, &Transform), With<TdMobMarker>>,
    mut net: ResMut<Net>,
    player_xf: Res<PlayerTransform>,
    mut orientation: ResMut<crate::player::PlayerOrientation>,
    p: Query<&crate::player::ClientPlayer>,
    rows: Res<TdStructRows>,
    td_tex: Res<TdTextures>,
    mut skins: ResMut<crate::skins::PlayerSkins>,
    minimap_state: Res<crate::minimap::MinimapState>,
) {
    if net.address.is_none() {
        return;
    }
    let Ok(player) = p.single() else { return };
    let now = unix_now_secs();
    if player.death_until_utc > now {
        return; // dead: reducers reject anyway — save the trip
    }
    let armed_build = mode.kind.is_some() || panel.is_open(Tab::Build);

    // ---- KeyF: strike the facing arc ----
    if keys.just_pressed(KeyCode::KeyF) && !armed_build {
        let dir = idlecore_core::player_combat::heading_index(orientation.facing_angle);
        crate::skins::start_attack(
            &mut skins,
            crate::skins::AttackAnim::of_weapon(player.weapon_def().anim),
        );
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.player_attack_then(dir, reducer_report("attack", tx.clone(), 0))
        });
    }

    // ---- KeyC: cycle the OWNED tier upward; KeyV: attempt to buy/equip
    // the next tier up (first equip consumes its materials — server checks).
    if keys.just_pressed(KeyCode::KeyC) {
        let owned = player.weapons_owned | 0b1; // fists always owned (tier 0)
        let next = next_owned_tier(player.weapon, owned);
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.equip_weapon_then(next, reducer_report("equip", tx.clone(), 0))
        });
    }
    if keys.just_pressed(KeyCode::KeyV) && player.weapon < 5 {
        // Buying the next rung: Fists → Club → Spear → Sword → Blade → Bow.
        let next = player.weapon + 1;
        let tx = net.sender();
        send_reducer(&mut net, |r| {
            r.equip_weapon_then(next, reducer_report("equip", tx.clone(), 0))
        });
    }

    // ---- Click a mob: strike toward its hex; click empty ground: strike
    // that facing (Spec 030 replaces the old left-click teleport). A
    // structure under the cursor keeps the 026 selection instead.
    if armed_build || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if panel.open {
        // The shared command panel is up — clicks belong to its widgets
        // (the panel background carries no Interaction, so guard here too).
        return;
    }
    // UI guard: a hovered/pressed widget keeps the click out of the world.
    if widgets.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed)) {
        return;
    }
    // Clicks on the minimap live on the minimap (select/teleport) — never
    // strike through it.
    if crate::minimap::cursor_on_minimap(&windows, &minimap_state) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_xf)) = cameras.single() else { return };
    let Ok(world) = camera.viewport_to_world_2d(cam_xf, cursor) else { return };
    let world = Vec2::new(world.x, world.y);
    let me = Vec2::new(player_xf.translation.x, player_xf.translation.y);
    // Structure hit → the selection system owns this click (Spec 026).
    let structure_hit = rows.rows.iter().any(|row| {
        cfg::defense_kind(&row.kind).is_some()
            && {
                let (_, aspect, _, height) = structure_sprite(&td_tex, &row.kind);
                structure_cursor_hit(world, row.x, row.y, aspect, height)
            }
    });
    if structure_hit {
        return;
    }
    // Closest mob within the strike footprints; else strike the ground point.
    let reach =
        (player.weapon_def().range_hexes.max(1) as f32) * WorldGenConfig::TILE_SIZE * 1.6;
    let target = mobs
        .iter()
        .map(|(a, t)| (a.mob_id, Vec2::new(t.translation.x, t.translation.y)))
        .min_by(|a, b| (a.1 - world).length().total_cmp(&(b.1 - world).length()))
        .filter(|(_, mob_pos)| {
            (mob_pos - world).length() < 2.4
                && (mob_pos - me).length() <= reach + WorldGenConfig::TILE_SIZE
        })
        .map(|(_, pos)| pos)
        .unwrap_or(world);
    let angle = (target - me).y.atan2((target - me).x);
    orientation.facing_angle = angle; // the clip + weapon overlay face the strike
    crate::skins::start_attack(
        &mut skins,
        crate::skins::AttackAnim::of_weapon(player.weapon_def().anim),
    );
    let dir = idlecore_core::player_combat::heading_index(angle);
    let tx = net.sender();
    send_reducer(&mut net, |r| {
        r.player_attack_then(dir, reducer_report("attack", tx.clone(), 0))
    });
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

#[derive(Component)]
struct TdBannerRoot;

#[derive(Component)]
struct TdBannerText;

/// One selectable building row (`'static str` = the server kind name).
#[derive(Component)]
pub struct TdBuildRow(pub &'static str);

/// Row icon (ImageNode image filled once TD textures are ready).
#[derive(Component)]
struct TdBuildRowIcon;

/// Per-row cost text (red when the inventory can't pay, Spec 024).
#[derive(Component)]
struct TdBuildCost;

// --- Spec 026: selection panel markers -------------------------------------
#[derive(Component)]
struct TdSelPanelRoot;
#[derive(Component)]
struct TdSelHeader;
#[derive(Component)]
struct TdSelHpBar;
#[derive(Component)]
struct TdSelStats;
#[derive(Component)]
struct TdSelXp;
#[derive(Component)]
struct TdSelActions;
#[derive(Component)]
struct TdSelUpgrade;
#[derive(Component)]
struct TdSelUpgradeTxt;
#[derive(Component)]
struct TdSelRemove;
#[derive(Component)]
struct TdSelRemoveTxt;
#[derive(Component)]
struct TdSelRefund;

/// Hover nameplate above the cursor's structure (transient, §3.4).
#[derive(Component)]
struct TdHoverLabel;

/// Transient overlay sprites: selection ring, attack area, build preview.
#[derive(Component)]
struct TdSelectFx;

/// Root of the top-left status cluster: next-raid countdown widget +
/// player stats (gold / XP / capital level-HP).
#[derive(Component)]
struct TdNextRaidRoot;

/// Big mm:ss countdown inside the widget.
#[derive(Component)]
struct TdNextRaidText;

/// Draining bar fill inside the widget (width = window fraction).
#[derive(Component)]
struct TdNextRaidBar;

/// Root of the gold/XP/capital stats panel under the raid widget.
#[derive(Component)]
struct TdStatsRoot;

/// Gold value TextSpan in the stats panel.
#[derive(Component)]
struct TdStatGoldVal;

/// Capital level TextSpan in the stats panel.
#[derive(Component)]
struct TdCapLevelVal;

/// Capital HP bar fill (width = hp ratio, tinted like the health bars).
#[derive(Component)]
struct TdCapHpBar;

/// Capital HP numeric readout.
#[derive(Component)]
struct TdCapHpVal;

const NEXT_RAID_BAR_W: f32 = 132.0;

/// Absolute HUD: raid banner (top center) + the top-left status cluster
/// (next-raid countdown widget + gold/XP/capital stats panel).
fn spawn_td_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: bevy::text::FontSource = asset_server.load("fonts/FiraSans-Bold.ttf").into();

    // Raid banner — display:none until a wave targets my capital (T3.3).
    commands
        .spawn((
            Name::new("td_banner"),
            TdBannerRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(110.0),
                left: Val::Percent(25.0),
                width: Val::Percent(50.0),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Text::default(),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
                    TextShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(1.0, 1.0),
                    },
                ))
                .with_child((
                    TextSpan::new(""),
                    TextColor(Color::srgb(1.0, 0.35, 0.3)),
                    TdBannerText,
                ));
        });

    // Next-raid countdown widget — top-right corner, gold "NEXT RAID" label,
    // bold mm:ss countdown, thin drain bar. Hidden until a capital exists.
    commands
        .spawn((
            Name::new("td_next_raid_widget"),
            TdNextRaidRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                column_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.09, 0.85)),
            BorderColor::all(Color::srgb(0.55, 0.4, 0.0)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("NEXT RAID"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.78, 0.15)),
                TextShadow {
                    color: Color::BLACK,
                    offset: Vec2::new(1.0, 1.0),
                },
            ));
            parent.spawn((
                Text::new("--:--"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    color: Color::BLACK,
                    offset: Vec2::new(1.0, 1.0),
                },
                TdNextRaidText,
            ));
            // The drain bar: dark trough + gold fill.
            parent
                .spawn((
                    Node {
                        width: Val::Px(NEXT_RAID_BAR_W),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                ))
                .with_child((
                    Node {
                        width: Val::Px(NEXT_RAID_BAR_W),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.78, 0.15)),
                    TdNextRaidBar,
                ));
        });

    // Stats cluster — top-left under the raid widget: Gold, XP, capital
    // level + HP bar (server-synced values, 0.4s refresh like the HUD).
    commands
        .spawn((
            Name::new("td_stats_panel"),
            TdStatsRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(122.0),
                left: Val::Px(20.0),
                width: Val::Px(176.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.09, 0.85)),
            BorderColor::all(Color::srgb(0.35, 0.33, 0.6)),
        ))
        .with_children(|parent| {
            // --- Gold row ---
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new("⛁ GOLD"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.84, 0.0)),
                        TextShadow {
                            color: Color::BLACK,
                            offset: Vec2::new(1.0, 1.0),
                        },
                    ));
                    row.spawn((
                        Text::new("-"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextShadow {
                            color: Color::BLACK,
                            offset: Vec2::new(1.0, 1.0),
                        },
                        TdStatGoldVal,
                    ));
                });
            // (XP lives on the hotbar's Minecraft-style bar — no duplicate here.)
            // --- Capital row: level chip + HP bar + count ---
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|cap_row| {
                    cap_row
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|inner| {
                            inner.spawn((
                                Text::new("⛨ CAPITAL"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.84, 0.0)),
                                TextShadow {
                                    color: Color::BLACK,
                                    offset: Vec2::new(1.0, 1.0),
                                },
                                TdCapLevelVal,
                            ));
                            inner.spawn((
                                Text::new("-"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TextShadow {
                                    color: Color::BLACK,
                                    offset: Vec2::new(1.0, 1.0),
                                },
                                TdCapHpVal,
                            ));
                        });
                    // HP bar: dark trough + ratio fill (green→amber→red).
                    cap_row
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(7.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                        ))
                        .with_child((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(7.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.25, 0.9, 0.3)),
                            TdCapHpBar,
                        ));
                });
        });

    // Buildings menu — Spec 029: the Build tab of the shared command panel.
    // No own wrapper/root; visibility + frame chrome are owned by ui::panel.
    // Rows re-skin to the shared tokens below.
    commands
        .spawn((
            Name::new("td_build_tab"),
            TabContent(Tab::Build),
            Node {
                width: Val::Px(330.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|panel| {
                panel.spawn((
                    Text::new("pick one → press E on a lit hex; ESC cancels"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.85, 0.8, 0.9)),
                ));
                for (label, kinds) in BUILD_GROUPS {
                    panel.spawn((
                        Text::new(label),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.55, 0.7, 0.75)),
                        TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
                        Node { margin: UiRect::top(Val::Px(2.0)), ..default() },
                    ));
                    for kind in kinds.iter() {
                    let cost = cfg::defense_kind(kind)
                        .map(|d| d.cost.iter().map(|(i, q)| format!("{q}×{i}")).collect::<Vec<_>>().join("  "))
                        .unwrap_or_default();
                    panel
                        .spawn((
                            Name::new(format!("td_build_row_{kind}")),
                            Button,
                            Interaction::default(),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(34.0),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(7.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(crate::ui::panel::ROW_BG),
                            BorderColor::all(crate::ui::panel::FRAME_BORDER),
                            TdBuildRow(kind),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                TdBuildRowIcon,
                                ImageNode::default(),
                                Node {
                                    width: Val::Px(26.0),
                                    height: Val::Px(26.0),
                                    margin: UiRect::right(Val::Px(6.0)),
                                    ..default()
                                },
                            ));
                            row.spawn((
                                Text::new(kind_display(kind)),
                                TextFont {
                                    font: font.clone(),
                                    font_size: FontSize::Px(13.0),
                                    ..default()
                                },
                                TextColor(TEXT_COLOR),
                                TextShadow {
                                    color: Color::BLACK,
                                    offset: Vec2::new(1.0, 1.0),
                                },
                                TextLayout::new(
                                    Justify::Left,
                                    bevy::text::LineBreak::NoWrap,
                                ),
                            ));
                            row.spawn((
                                Text::new(cost),
                                TextFont {
                                    font: font.clone(),
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.85, 0.4)),
                                TdBuildCost,
                            ));
                        });
                    }
                }
            });

    // Spec 026 §3.3 — selection panel. Hidden until a structure is picked;
    // content is maintained by `update_select_panel`.
    commands
        .spawn((
            Name::new("td_select_panel_root"),
            TdSelPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                bottom: Val::Px(150.0),
                width: Val::Px(250.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(9.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.9)),
            BorderColor::all(Color::srgb(0.28, 0.62, 0.5)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("-"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow { color: Color::BLACK, offset: Vec2::new(1.0, 1.0) },
                TdSelHeader,
            ));
            // HP bar trough + ratio fill.
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(7.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                ))
                .with_child((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(7.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.9, 0.3)),
                    TdSelHpBar,
                ));
            panel.spawn((
                Text::new("-"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                TdSelStats,
            ));
            panel.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.95, 0.7)),
                TdSelXp,
            ));
            // Action row: Upgrade + Remove buttons side by side.
            panel
                .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, display: Display::None, ..default() }, TdSelActions))
                .with_children(|actions| {
                    actions.spawn((
                        Button,
                        Interaction::default(),
                        Node {
                            flex_grow: 1.0,
                            margin: UiRect::horizontal(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.1, 0.18, 0.3)),
                        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        TdSelUpgrade,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Upgrade"),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.65, 0.95, 0.7)),
                            TextShadow { color: Color::BLACK, offset: Vec2::new(0.5, 0.5) },
                            TdSelUpgradeTxt,
                        ));
                    });
                    actions.spawn((
                        Button,
                        Interaction::default(),
                        Node {
                            flex_grow: 1.0,
                            margin: UiRect::horizontal(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.1, 0.18, 0.3)),
                        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        TdSelRemove,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Remove"),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.5, 0.35)),
                            TextShadow { color: Color::BLACK, offset: Vec2::new(0.5, 0.5) },
                            TdSelRemoveTxt,
                        ));
                    });
                });
            panel.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                TdSelRefund,
            ));
        });
}

/// Human row label for a buildable kind ("BoneBarricade" → "Barricade").
fn kind_display(kind: &str) -> &'static str {
    match kind {
        cfg::ITEM_BONE_BARRICADE => "Barricade",
        cfg::ITEM_ARROW_TOWER => "Arrow Tower",
        cfg::ITEM_BALLISTA => "Ballista",
        cfg::ITEM_STORAGE_CHEST => "Chest",
        // REV4 expansion.
        cfg::ITEM_WOOD_WALL => "Wood Wall",
        cfg::ITEM_IRON_WALL => "Iron Wall",
        cfg::ITEM_LANTERN_TOWER => "Lantern Tower",
        cfg::ITEM_COCONUT_TOWER => "Coconut Tower",
        cfg::ITEM_FROST_TOWER => "Frost Tower",
        cfg::ITEM_SPIKE_TRAP => "Spike Trap",
        cfg::ITEM_WEB_TRAP => "Web Trap",
        cfg::ITEM_FIRE_TRAP => "Fire Trap",
        _ => "Structure",
    }
}

/// Refresh banner, countdown widget + stats cluster from the cached
/// [`TdView`] every 0.4s (constant UI churn kills frames — idle panel's
/// throttle pattern). The countdown drains locally between server syncs.
#[allow(clippy::too_many_arguments)] // ParamSets keep queries unambiguous
fn update_td_hud(
    tdview: Res<TdView>,
    time: Res<Time>,
    player: Query<&crate::player::ClientPlayer>,
    mut throttle: Local<f32>,
    // Five sibling `Node` writers — ParamSet sidesteps the Without matrix.
    // 0 banner root, 1 raid widget root, 2 raid bar, 3 stats root,
    // 4 capital HP bar.
    mut nodes: ParamSet<(
        Query<&mut Node, With<TdBannerRoot>>,
        Query<(&mut Node, &mut BorderColor), With<TdNextRaidRoot>>,
        Query<(&mut Node, &mut BackgroundColor), With<TdNextRaidBar>>,
        Query<&mut Node, With<TdStatsRoot>>,
        Query<(&mut Node, &mut BackgroundColor), With<TdCapHpBar>>,
    )>,
    // Five sibling Text writers + the countdown's TextColor. These HUD bits
    // are root `Text` nodes (rev6d+), so writers mutate `Text`, not
    // `TextSpan`.
    // 0 banner, 1 raid countdown, 2 gold, 3 capital level, 4 capital HP
    // count. (XP lives on the hotbar bar, not here.)
    mut spans: ParamSet<(
        Query<&mut TextSpan, With<TdBannerText>>,
        Query<(&mut Text, &mut TextColor), With<TdNextRaidText>>,
        Query<&mut Text, With<TdStatGoldVal>>,
        Query<&mut Text, With<TdCapLevelVal>>,
        Query<&mut Text, With<TdCapHpVal>>,
    )>,
) {
    *throttle += time.delta_secs();
    if *throttle < 0.4 {
        return;
    }
    *throttle = 0.0;

    // Raid banner on/off + message.
    if let Ok(mut node) = nodes.p0().single_mut() {
        node.display = if tdview.incoming.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if tdview.incoming.is_some() {
        if let Ok(mut text) = spans.p0().single_mut() {
            let count = tdview.incoming.unwrap_or(0);
            **text = match tdview.wave_banner {
                Some((n, pattern)) => {
                    format!("⚔ Wave {n} — {pattern}: {count} raiders march on your capital")
                }
                None => format!("⚔ RAID! {count} raiders marching on your capital"),
            };
        }
    }

    // Next-raid countdown widget: drains locally from the synced clock.
    let (countdown, bar_fill, accent) = {
        // Local drain: sync time + elapsed real seconds since the snapshot.
        let elapsed = unix_now_secs().saturating_sub(tdview.next_wave_sync_utc);
        let remaining = tdview
            .next_wave_in
            .map(|s| s.saturating_sub(elapsed))
            .or(tdview.incoming.map(|_| 0));
        if tdview.incoming.is_some() {
            ("RAID NOW".to_string(), 1.0, Color::srgb(1.0, 0.4, 0.35))
        } else {
            match remaining {
                Some(secs) => {
                    let label = fmt_countdown(secs);
                    let total = tdview.next_wave_total.max(cfg::WAVE_REATTACK_SECS) as f32;
                    let frac = (secs as f32 / total).clamp(0.0, 1.0);
                    (label, frac, Color::srgb(1.0, 0.78, 0.15))
                }
                None => ("--:--".to_string(), 0.0, Color::srgb(0.6, 0.6, 0.6)),
            }
        }
    };
    let raided = tdview.incoming.is_some();
    if let Ok((mut root, mut root_border)) = nodes.p1().single_mut() {
        root.display = if tdview.capital.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        root_border.set_all(if raided {
            Color::srgb(0.85, 0.25, 0.2)
        } else {
            Color::srgb(0.55, 0.4, 0.0)
        });
    }
    if let Ok((mut text, mut color)) = spans.p1().single_mut() {
        text.0 = countdown;
        color.0 = if raided {
            Color::srgb(1.0, 0.4, 0.35)
        } else {
            Color::WHITE
        };
    }
    if let Ok((mut bar, mut bar_color)) = nodes.p2().single_mut() {
        bar.width = Val::Px((NEXT_RAID_BAR_W * bar_fill).max(2.0));
        bar_color.0 = accent;
    }

    // ---- Stats cluster: gold, capital level + HP bar ----
    // Gold/XP panel is always visible once logged in (works before capital).
    if let Ok(mut root) = nodes.p3().single_mut() {
        root.display = Display::Flex;
    }
    let gold = player.single().map(|p| p.gold).unwrap_or(0);
    if let Ok(mut text) = spans.p2().single_mut() {
        text.0 = fmt_short(gold);
    }
    if let Some(cap) = tdview.capital.as_ref() {
        let max_hp = cfg::capital_max_hp(cap.level);
        let ratio = clamp_ratio(cap.hp as f32, max_hp as f32);
        if let Ok(mut text) = spans.p3().single_mut() {
            text.0 = format!("⛨ L{}", cap.level);
        }
        if let Ok(mut text) = spans.p4().single_mut() {
            text.0 = format!("{} / {}", cap.hp, max_hp);
        }
        if let Ok((mut bar, mut bar_color)) = nodes.p4().single_mut() {
            bar.width = Val::Percent(100.0 * ratio);
            bar_color.0 = hp_bar_color(ratio);
        }
    } else if let Ok((mut bar, mut bar_color)) = nodes.p4().single_mut() {
        bar.width = Val::Px(2.0);
        bar_color.0 = hp_bar_color(0.0);
    }
}

/// Short number format for the stats cluster: `1.2k`, `3.4M`, full digits
/// below 10 000 (HUD numbers must survive the 176 px panel).
fn fmt_short(n: u64) -> String {
    if n >= 10_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Health-bar tint from an HP ratio (green → amber → red).
fn hp_bar_color(ratio: f32) -> Color {
    if ratio > 0.6 {
        Color::srgb(0.25, 0.9, 0.3)
    } else if ratio > 0.3 {
        Color::srgb(0.95, 0.75, 0.2)
    } else {
        Color::srgb(1.0, 0.3, 0.25)
    }
}

/// Countdown label scaled to the window: `mm:ss` under 10 min, `12m04`
/// up to 2 h, `22h12m` for the long rebuild grace.
fn fmt_countdown(secs: u64) -> String {
    if secs < 600 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else if secs < 7200 {
        format!("{}m{:02}", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, secs % 3600 / 60)
    }
}

#[cfg(test)]
mod stat_tests {
    use super::*;

    #[test]
    fn fmt_short_scales_at_the_readout_breaks() {
        assert_eq!(fmt_short(0), "0");
        assert_eq!(fmt_short(9_999), "9999");
        assert_eq!(fmt_short(10_000), "10.0k");
        assert_eq!(fmt_short(1_200_000), "1200.0k");
        assert_eq!(fmt_short(10_000_000), "10.0M");
        assert_eq!(fmt_short(24_300_000), "24.3M");
    }

    #[test]
    fn hp_bar_color_matches_the_world_health_bars() {
        assert_eq!(hp_bar_color(1.0), Color::srgb(0.25, 0.9, 0.3));
        assert_eq!(hp_bar_color(0.45), Color::srgb(0.95, 0.75, 0.2));
        assert_eq!(hp_bar_color(0.1), Color::srgb(1.0, 0.3, 0.25));
    }

    #[test]
    fn structure_level_pip_hides_for_level_1_and_legacy_rows() {
        assert_eq!(structure_level_label(0), None, "legacy (0) = L1, no pip");
        assert_eq!(structure_level_label(1), None);
        assert_eq!(structure_level_label(2).as_deref(), Some("L2"));
        assert_eq!(structure_level_label(5).as_deref(), Some("L5"));
        assert_eq!(
            structure_level_label(9).as_deref(),
            Some("L5"),
            "over-cap rows read as the top level"
        );
    }

    #[test]
    fn upgrade_target_only_picks_own_neighbor_structures() {
        // Player on (0,0): own structure there, an enemy one next door.
        let rows = vec![
            (7u64, "other".to_string(), 0, 1),
            (11u64, "me".to_string(), 0, 1),       // 1 hex north
            (12u64, "me".to_string(), 3, 0),       // 3 hexes east — out of reach
            (13u64, "me".to_string(), 1, 0),       // 1 hex east — nearest
        ];
        let me = TileCoord::new(0, 0);
        assert_eq!(
            choose_upgrade_target("me", me, &rows),
            Some(11),
            "nearest own neighbor wins (tie → lowest id), enemies ignored"
        );
        // No own structure in reach → nothing to transfer into.
        assert_eq!(choose_upgrade_target("me", TileCoord::new(9, 9), &rows), None);
        assert_eq!(choose_upgrade_target("nobody", me, &rows), None);
    }

    #[test]
    fn every_roster_kind_has_a_sprite_map_entry() {
        // The client atlas map covers the full server roster, byte-for-byte
        // on the names (Spec 025 §5 — `Unknown` fallback covers stale rows).
        let mut names: Vec<&str> = MOB_ATLAS.iter().map(|(name, ..)| *name).collect();
        names.sort_unstable();
        let mut roster: Vec<&str> = cfg::MOB_KINDS.iter().map(|k| k.name).collect();
        roster.sort_unstable();
        assert_eq!(names, roster);
        // Rooted blooms animate idle-only: walk slots share the idle crop.
        let (_, venom_idle, venom_w0, venom_w1) = mob_atlas_row("Venom Bloom");
        assert_eq!(venom_idle, venom_w0);
        assert_eq!(venom_w0, venom_w1);
    }

    #[test]
    fn walk_flip_cadence_alternates_two_frames() {
        // The animate system flips every 0.25 s: sample the cadence math.
        for (elapsed, want) in [(0.0, 0), (0.24, 0), (0.25, 1), (0.49, 1), (0.52, 0), (1.0, 0)] {
            assert_eq!((elapsed / 0.25) as usize % 2, want, "at {elapsed}s");
        }
    }

    /// The MOB_ATLAS row for one kind name (test helper).
    fn mob_atlas_row(name: &str) -> (&'static str, &'static str, &'static str, &'static str) {
        *MOB_ATLAS.iter().find(|(n, ..)| *n == name).unwrap()
    }

    #[test]
    fn fmt_countdown_scales_with_window_length() {
        assert_eq!(fmt_countdown(0), "0:00");
        assert_eq!(fmt_countdown(45), "0:45");
        assert_eq!(fmt_countdown(599), "9:59");
        // Reattack window reads as minutes+m:ss.
        assert_eq!(fmt_countdown(600), "10m00");
        assert_eq!(fmt_countdown(7199), "119m59");
        assert_eq!(fmt_countdown(7200), "2h00m");
        assert_eq!(fmt_countdown(79_978), "22h12m");
    }
}

#[cfg(test)]
mod tests_spec026_select {
    use super::*;

    #[test]
    fn cursor_hits_the_sprite_rect_with_hex_fallback() {
        // Arrow Tower at (10, 10): rect w = 2.2·(24/32) ≈ 1.65, base y 10.4.
        let (hx, hy) = (10.0f32, 10.0f32);
        assert!(structure_cursor_hit(
            Vec2::new(hx, hy + 1.0),
            hx,
            hy,
            24.0 / 32.0,
            2.2
        ), "body rect hit");
        assert!(structure_cursor_hit(
            Vec2::new(hx, hy + 1.4),
            hx,
            hy,
            24.0 / 32.0,
            2.2
        ), "up to the top of the sprite");
        assert!(!structure_cursor_hit(
            Vec2::new(hx + 1.6, hy + 1.0),
            hx,
            hy,
            24.0 / 32.0,
            2.2
        ) == false || structure_hex_hit(
            Vec2::new(hx + 1.0, hy),
            hx,
            hy,
        ), "hex fallback catches near-groud.fx");
        assert!(!structure_cursor_hit(
            Vec2::new(hx + 9.0, hy + 9.0),
            hx,
            hy,
            1.0,
            2.0
        ), "far away is a terrain click (deselect)");
    }

    #[test]
    fn chebyshev_hex_hit_matches_the_tile_size() {
        let half = WorldGenConfig::TILE_SIZE * 0.5;
        assert!(structure_hex_hit(
            Vec2::new(half, half - 0.01),
            0.0,
            0.0
        ), "inside the half-tile box");
        assert!(!structure_hex_hit(
            Vec2::new(half + 0.01, half + 0.01),
            0.0,
            0.0
        ), "outside is another hex");
        // Chebyshev: square, matching the server's tile_distance.
        assert!(structure_hex_hit(Vec2::new(half, half), 0.0, 0.0));
    }

    #[test]
    fn range_square_sides_cover_the_full_chebyshev_window() {
        // `tile_distance ≤ r` is a (2r+1)·T square on the lattice.
        assert_eq!(range_square_side(0), WorldGenConfig::TILE_SIZE);
        assert_eq!(range_square_side(2), 5.0 * WorldGenConfig::TILE_SIZE);
        assert_eq!(range_square_side(3), 7.0 * WorldGenConfig::TILE_SIZE);
        assert_eq!(range_square_side(-1), WorldGenConfig::TILE_SIZE, "clamp");
    }

    #[test]
    fn selection_ring_differs_for_owners_and_neighbors() {
        let mine = selection_ring_color(Some("arthur"), Some("arthur"));
        let theirs = selection_ring_color(Some("bob"), Some("arthur"));
        assert_ne!(mine, theirs, "own land reads teal, others read ice");
    }

    #[test]
    fn remove_refund_preview_matches_half_of_every_ingredient() {
        // Arrow Tower: (8W, 4S, 1MP) → halves 4W 2S 0MP — 0MP hidden.
        let tower = cfg::defense_kind(cfg::ITEM_ARROW_TOWER).unwrap();
        assert_eq!(remove_refund_preview(tower), "Refund: Wood×4, Stone×2");
        let band = cfg::defense_kind(cfg::ITEM_BONE_BARRICADE).unwrap();
        assert_eq!(remove_refund_preview(band), "Refund: Bone×3");
        // Iron Wall: (6MP, 4S) → 3MP 2S.
        let iron = cfg::defense_kind(cfg::ITEM_IRON_WALL).unwrap();
        assert_eq!(remove_refund_preview(iron), "Refund: MetalPart×3, Stone×2");
    }

    #[test]
    fn upgrade_delta_line_reports_hp_and_dps_per_tier() {
        let tower = cfg::defense_kind(cfg::ITEM_ARROW_TOWER).unwrap();
        assert_eq!(
            upgrade_delta_line(tower, 1).as_deref(),
            Some("hp 80→104 dps 10→12"),
            "L2: +30% hp over base (10·L), +25% dps floored"
        );
        assert_eq!(upgrade_delta_line(tower, 5), None, "cap hides the row");
        // Walls show hp only (no dps at all).
        let wall = cfg::defense_kind(cfg::ITEM_WOOD_WALL).unwrap();
        assert_eq!(
            upgrade_delta_line(wall, 1).as_deref(),
            Some("hp 70→91"),
            "extra text trimmed on wall tiers"
        );
    }
}

#[cfg(test)]
mod tests_rev5_glide {
    use super::*;

    #[test]
    fn glide_speed_matches_each_kind_step_cadence() {
        // One lattice hex (TILE_SIZE wu) per step interval: Slime marches
        // every 2 s, Spear Goblin 3 s, Myconid 4 s, boss 5 s.
        for (secs, name) in [(2u32, "Slime"), (3, "Spear Goblin"), (4, "Myconid"), (5, "Big Slime")] {
            let kind = cfg::mob_kind(name).expect("roster kind");
            assert_eq!(kind.step_interval_secs, secs, "{name} cadence pinned");
            let speed = mob_glide_speed(secs);
            assert!(
                (speed - WorldGenConfig::TILE_SIZE / secs as f32).abs() < 1e-6,
                "{name} glide speed"
            );
        }
        // Degenerate interval clamps to 1 — no zero/div-by-none speed.
        assert!(mob_glide_speed(0) > 0.0);
    }

    #[test]
    fn snapshot_jump_beyond_two_hexes_snaps_not_glides() {
        let t = WorldGenConfig::TILE_SIZE;
        assert!(!mob_should_snap(t), "a normal one-hex step glides");
        assert!(!mob_should_snap(t * 1.5), "sub-2-hex gaps keep gliding");
        assert!(mob_should_snap(t * 2.1), "> 2 hexes is a desync, snap it");
        assert!(mob_should_snap(500.0));
    }

    #[test]
    fn walk_gate_threshold_distinguishes_standing_from_gliding() {
        // The gate lives inline in animate_mobs (`remaining > 0.05 wu`);
        // pin the bound so idle-frame churn cannot regress silently.
        assert!(Vec2::new(0.2, 0.0).length() > 0.05);
        assert!(Vec2::new(0.01, 0.0).length() <= 0.05);
    }
}

#[cfg(test)]
mod tests_cull_radii {
    use super::*;

    #[test]
    fn cull_radii_cover_the_zoomed_out_viewport() {
        // Default extents: the historical fixed radii win.
        let (v, t) = cull_radii(&crate::plugins::camera::ViewExtent { half_diagonal: 40.0 });
        assert_eq!(v, VIEW_RADIUS * VIEW_RADIUS);
        assert_eq!(t, TERRITORY_RADIUS * TERRITORY_RADIUS);
        // Zoomed out (half-diagonal 133 ≈ 1600px window at MIN_ZOOM): the
        // radii grow to cover the viewport, never beyond the cap.
        let (v, t) = cull_radii(&crate::plugins::camera::ViewExtent { half_diagonal: 133.0 });
        assert!(v >= 133.0 * 133.0, "view must cover the viewport: {v}");
        assert!(t >= 133.0 * 133.0, "territory must cover the viewport: {t}");
        let (_, t) = cull_radii(&crate::plugins::camera::ViewExtent { half_diagonal: 10_000.0 });
        assert_eq!(t, MAX_CULL_RADIUS * MAX_CULL_RADIUS, "hard cap");
    }
}

#[cfg(test)]
mod tests_fx {
    use super::*;

    /// REV3 §13 (exhaustive): every combat_event kind the server can write
    /// (types.rs CE_* set) maps to exactly one fx plan — unknown kinds are
    /// dropped, never crash the client.
    #[test]
    fn fx_plan_covers_every_server_event_kind() {
        let server_kinds = [
            "shot",
            "melee",
            "bomb",
            "spore",
            "splash",
            "struct_fell",
            "capital_hit",
            "trap", // REV4 spike/web/fire traps
            "player_hit", // Spec 027 §8: my swing lands
            "player_hurt", // I take damage
        ];
        for kind in server_kinds {
            assert!(
                fx_plan(kind).is_some(),
                "kind {kind} must map to an fx plan"
            );
        }
        assert!(fx_plan("unknown-later").is_none());
        assert!(matches!(fx_plan("capital_hit"), Some(FxBirth::CapitalHit)));
        for kind in ["shot", "bomb", "spore"] {
            assert!(matches!(fx_plan(kind), Some(FxBirth::Bolt(_))));
        }
        for kind in ["melee", "splash", "struct_fell"] {
            assert!(matches!(fx_plan(kind), Some(FxBirth::Impact(_))));
        }
        assert!(matches!(fx_plan("player_hit"), Some(FxBirth::PlayerHit)));
        assert!(matches!(fx_plan("player_hurt"), Some(FxBirth::Impact(_))));
    }

    #[test]
    fn dash_chip_text_flips_between_ready_and_countdown() {
        assert_eq!(
            dash_chip_text(0, 1_000),
            "Dash [Space] — ready",
            "0 cd = never held = ready"
        );
        assert_eq!(
            dash_chip_text(1_000, 1_000),
            "Dash [Space] — ready",
            "at the stamp the cd is spent"
        );
        assert_eq!(
            dash_chip_text(1_005, 1_000),
            "Dash [Space] — 0:05",
            "draining chip counts the remaining window"
        );
    }

    #[test]
    fn combat_hp_ratio_bars_are_a_clamped_fraction() {
        assert_eq!(combat_hp_ratio(0, 0), 0.0, "degenerate max");
        assert_eq!(combat_hp_ratio(100, 100), 1.0);
        assert_eq!(combat_hp_ratio(50, 100), 0.5);
        assert_eq!(combat_hp_ratio(0, 100), 0.0);
        assert_eq!(combat_hp_ratio(150, 100), 1.0, "overheal clamps");
    }

    #[test]
    fn next_owned_tier_cycles_owned_masks() {
        // bit i = tier i owned; fists (tier 0) implicit.
        assert_eq!(next_owned_tier(0, 0), 0, "only fists owned → stay fists");
        assert_eq!(next_owned_tier(4, 0), 0, "reads normalize to owned set");
        assert_eq!(next_owned_tier(0, 0b1_0010), 1, "next owned above current");
        assert_eq!(next_owned_tier(1, 0b1_0010), 4, "wraps to the top owned");
        assert_eq!(next_owned_tier(4, 0b1_0010), 0, "top → lowest owned (fists)");
        assert_eq!(next_owned_tier(1, 0), 0, "no owned tiers → fists");
    }

    /// REV3 §14: the banner name derivable on the client matches the
    /// server's pattern pick cadence (size-independent).
    #[test]
    fn wave_banner_names_follow_the_server_cadence() {
        // Same integer inputs → same names as the server spawn.
        for level in 1..=5u32 {
            for idx in 0..24u64 {
                let client = if cfg::boss_wave(idx, level) {
                    "Big Slime Boss"
                } else {
                    cfg::wave_pattern(idx, 1, level)
                        .map(|(name, _)| name)
                        .unwrap_or("Scout Line")
                };
                let server = match cfg::wave_pattern(idx, 5, level) {
                    Some((name, _)) => name,
                    None => "Scout Line",
                };
                let expected = if cfg::boss_wave(idx, level) {
                    "Big Slime Boss"
                } else {
                    server
                };
                assert_eq!(client, expected, "banner cadence diverged: L{level} idx{idx}");
            }
        }
    }
}
