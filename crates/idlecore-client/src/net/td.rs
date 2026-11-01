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
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use idlecore_core::defense_config as cfg;
use idlecore_core::hex::HexCoord;
use idlecore_core::hex_grid::HexGrid;
use idlecore_core::world_gen::WorldGenConfig;

use super::gen::*;
use spacetimedb_sdk::__codegen::TableLike;
use super::plugin::{Net, NetEvent};
// 0.20: the old `bevy::ui::Button` is a deprecated type alias; the menu rows
// use the headless `ui_widgets::Button` widget like the rest of the HUD.
use bevy::ui_widgets::Button;
use crate::player::PlayerTransform;
use crate::world_floor::{atlas, prop_depth};
use idlecore_core::slots::SLOT_SIZE;
// Procedural hexagon sprite textures (buildable-area / ownership overlays).
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Shared HUD text tint (same value as `net::hud`, kept local for privacy).
const TEXT_COLOR: Color = Color::srgb(0.9, 0.95, 1.0);

/// View radius (squared, world units) beyond which TD markers aren't drawn.
const VIEW_RADIUS_SQ: f32 = 90.0 * 90.0;

/// How far (world radius) ownership overlays stretch around the player.
const TERRITORY_RADIUS_SQ: f32 = 120.0 * 120.0;

pub struct TdPlugin;

impl Plugin for TdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TdTerritory>()
            .init_resource::<TdTextures>()
            .init_resource::<TdView>()
            .init_resource::<TdTracker>()
            .init_resource::<TdBuildMode>()
            .init_resource::<TdBuildMenu>()
            .init_resource::<TdHexTex>()
            .add_systems(Startup, spawn_td_hud)
            .add_systems(
                Update,
                (
                    init_td_textures,
                    init_td_hex_tex,
                    sync_td_entities,
                    glide_td_markers,
                    td_pulse,
                    td_place_hints,
                    td_build_keys,
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
    pub mob_scav: (Handle<Image>, f32),
    pub mob_walker: (Handle<Image>, f32),
    pub mob_husk: (Handle<Image>, f32),
    pub capital: (Handle<Image>, f32),
    pub barricade: (Handle<Image>, f32),
    pub arrow_tower: (Handle<Image>, f32),
    pub ballista: (Handle<Image>, f32),
    pub icon_core: Handle<Image>,
    pub icon_part: Handle<Image>,
    pub icon_bone: Handle<Image>,
    pub icon_barricade: Handle<Image>,
    pub icon_arrow: Handle<Image>,
    pub icon_ballista: Handle<Image>,
}

/// Last replicated mob positions (mob_id → position + observed time) used to
/// derive glide speeds: the server simulates on 1s ticks, so consecutive
/// snapshots give each mob a march velocity to animate between.
#[derive(Resource, Default)]
pub struct TdTracker {
    pub last: HashMap<u64, (Vec2, Instant)>,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// World marker for one raider (child: HP bar).
#[derive(Component)]
pub struct TdMobMarker;

/// World marker for one tower/wall (child: HP bar).
#[derive(Component)]
pub struct TdStructureMarker;

/// World marker for a placed capital (address of the row owner).
#[derive(Component)]
pub struct TdCapitalMarker(#[allow(dead_code)] pub String);

/// Claimed-hex ownership overlay (gold = mine, violet = another player).
#[derive(Component)]
pub struct TdOverlay;

/// Interpolated glide target — mobs march one hex every few seconds
/// server-side; speed is zero today (rebuilds land on new positions) but
/// the field stays for the interpolation pass.
#[derive(Component, Clone)]
struct TdMove {
    target: Vec2,
    speed: f32,
}

// ---------------------------------------------------------------------------
// Textures
// ---------------------------------------------------------------------------

fn init_td_textures(atlas: Res<crate::slice::SlicedAtlas>, mut td: ResMut<TdTextures>) {
    if td.ready {
        return;
    }
    let g = |k: &str| atlas.items.get(k).cloned();
    // Art aspects (crop width ÷ height) — the tower/capital are tall, mobs
    // and walls square.
    let with_aspect = |key: &str, aspect: f32| g(key).map(|h| (h, aspect));
    let (Some(mob_scav), Some(mob_walker), Some(mob_husk)) = (
        with_aspect(atlas::TD_MOB_SCAV, 1.0),
        with_aspect(atlas::TD_MOB_WALKER, 1.0),
        with_aspect(atlas::TD_MOB_HUSK, 1.0),
    ) else {
        return;
    };
    let (Some(capital), Some(barricade), Some(arrow_tower), Some(ballista)) = (
        with_aspect(atlas::TD_CAPITAL, 128.0 / 112.0),
        with_aspect(atlas::TD_BARRICADE, 1.0),
        with_aspect(atlas::TD_ARROW_TOWER, 16.0 / 32.0),
        with_aspect(atlas::TD_BALLISTA, 1.0),
    ) else {
        return;
    };
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
    td.mob_scav = mob_scav;
    td.mob_walker = mob_walker;
    td.mob_husk = mob_husk;
    td.capital = capital;
    td.barricade = barricade;
    td.arrow_tower = arrow_tower;
    td.ballista = ballista;
    td.icon_core = icon_core;
    td.icon_part = icon_part;
    td.icon_bone = icon_bone;
    td.icon_barricade = icon_barricade;
    td.icon_arrow = icon_arrow;
    td.icon_ballista = icon_ballista;
    td.ready = true;
    info!("TD textures ready (Spec 023)");
}

/// Placeholder tints — the icons stay readable with plain color blends.
fn mob_texture(td: &TdTextures, kind: &str) -> (Handle<Image>, f32, Color) {
    match kind {
        "Scavenger" => (td.mob_scav.0.clone(), td.mob_scav.1, Color::srgb(0.72, 0.5, 0.95)),
        "BoneWalker" => (td.mob_walker.0.clone(), td.mob_walker.1, Color::srgb(0.95, 0.93, 0.86)),
        _ => (td.mob_husk.0.clone(), td.mob_husk.1, Color::srgb(0.85, 0.45, 0.2)),
    }
}

fn structure_sprite(td: &TdTextures, kind: &str) -> (Handle<Image>, f32, Color) {
    match kind {
        cfg::ITEM_BONE_BARRICADE => (
            td.barricade.0.clone(),
            td.barricade.1,
            Color::srgb(0.8, 0.7, 0.5),
        ),
        cfg::ITEM_ARROW_TOWER => (
            td.arrow_tower.0.clone(),
            td.arrow_tower.1,
            Color::srgb(0.55, 0.8, 1.0),
        ),
        // The chest borrows the barricade art, warm brown crate tone.
        cfg::ITEM_STORAGE_CHEST => (
            td.barricade.0.clone(),
            td.barricade.1,
            Color::srgb(0.65, 0.45, 0.25),
        ),
        _ => (td.ballista.0.clone(), td.ballista.1, Color::srgb(0.7, 0.55, 1.0)),
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

struct StructRow {
    structure_id: u64,
    x: f32,
    y: f32,
    kind: String,
    hp: u32,
    /// 0 = legacy row (client treats as L1); pip shows from L2.
    level: u8,
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
    now: u64,
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u64)
        .unwrap_or(0)
}

fn hex_center(q: i32, r: i32) -> (f32, f32) {
    HexGrid::axial_to_world(q, r, WorldGenConfig::HEX_SIZE)
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

fn sync_td_entities(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut territory: ResMut<TdTerritory>,
    mut tdview: ResMut<TdView>,
    mut tracker: ResMut<TdTracker>,
    td_tex: Res<TdTextures>,
    hex_tex: Res<TdHexTex>,
    player_xf: Res<PlayerTransform>,
    mobs: Query<Entity, With<TdMobMarker>>,
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
                let c = HexCoord::from_id(s.hex_id);
                let (hx, hy) = hex_center(c.q, c.r);
                StructRow {
                    structure_id: s.structure_id,
                    x: hx + s.offset_x,
                    y: hy + s.offset_y,
                    kind: s.kind.clone(),
                    hp: s.hp,
                    // 0/absent = L1 for the HUD pip logic.
                    level: s.level,
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
        (
            address,
            TdSnapshot {
                territory,
                capitals,
                waves,
                structures,
                mobs,
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

    // ---- rebuild: TD entities despawn then respawn (rows are few) ----
    for e in mobs.iter() {
        commands.entity(e).despawn();
    }
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
    // ---- ownership overlays ----
    for (hex_id, owner) in &snap.territory {
        let c = HexCoord::from_id(*hex_id);
        let (cx, cy) = hex_center(c.q, c.r);
        let dist_sq = {
            let dx = cx - px;
            let dy = cy - py;
            dx * dx + dy * dy
        };
        if dist_sq > TERRITORY_RADIUS_SQ {
            continue;
        }
        let ownership_mine = Some(owner.as_str()) == address.as_deref();
        commands.spawn((
            TdOverlay,
            Sprite {
                image: hex_tex.hex.clone(),
                // Tone: a warm gold ring for mine, cool violet for others —
                // full-hex so territory reads as land, not pinhead dots.
                color: if ownership_mine {
                    Color::srgba(1.0, 0.86, 0.35, 0.5)
                } else {
                    Color::srgba(0.62, 0.42, 1.0, 0.4)
                },
                custom_size: Some(Vec2::new(
                    f32::sqrt(3.0) * WorldGenConfig::HEX_SIZE,
                    2.0 * WorldGenConfig::HEX_SIZE,
                )),
                ..default()
            },
            Transform::from_xyz(cx, cy, prop_depth(cy) + 3.0),
        ));
    }

    // ---- capitals ----
    for row in &snap.capitals {
        if row.destroyed {
            continue;
        }
        let c = HexCoord::from_id(row.hex_id);
        let (cx, cy) = hex_center(c.q, c.r);
        let (dx, dy) = (cx - px, cy - py);
        if dx * dx + dy * dy > VIEW_RADIUS_SQ {
            continue;
        }
        let max = cfg::capital_max_hp(row.level);
        let ratio = clamp_ratio(row.hp as f32, max as f32);
        // The house is 128×112 — bottom-anchored so the base sits inside the
        // hex instead of floating; hash-hex building scale reads as "city".
        let aspect = td_tex.capital.1;
        let height = 4.8;
        let pos = Vec3::new(cx, cy + 0.8, prop_depth(cy) + 18.0);
        commands.spawn((
            TdCapitalMarker(row.owner.clone()),
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
                custom_size: Some(Vec2::new(4.4 * ratio, 0.34)),
                ..default()
            },
            Transform::from_xyz(0.0, height + 0.35, 0.05),
        ));
    }

    // ---- defense structures ----
    for row in &snap.structures {
        let (dx, dy) = (row.x - px, row.y - py);
        if dx * dx + dy * dy > VIEW_RADIUS_SQ {
            continue;
        }
        let Some(def) = cfg::defense_kind(&row.kind) else { continue };
        let (image, aspect, color) = structure_sprite(&td_tex, &row.kind);
        // Leveled structures fill their bar against their own max HP.
        let ratio = clamp_ratio(row.hp as f32, cfg::structure_max_hp(def, row.level) as f32);
        // Bottom-anchored so walls/towers stand on the ground slot.
        let height = 2.2;
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
        // Spec 024: green level pip above the HP bar from L2.
        if let Some(label) = structure_level_label(row.level) {
            commands.spawn((
                Text2d::new(label),
                TextFont { font_size: 14.0.into(), ..default() },
                TextColor(Color::srgb(0.3, 1.0, 0.55)),
                TextShadow { color: Color::BLACK, offset: Vec2::new(0.5, 0.5) },
                Transform::from_xyz(row.x, row.y + height + 0.75, prop_depth(row.y) + 12.0),
            ));
        }
        let _ = row.structure_id;
    }

    // ---- mobs ----
    for row in &snap.mobs {
        let Some(mob) = cfg::mob_kind(&row.kind) else { continue };
        let (image, aspect, color) = mob_texture(&td_tex, &row.kind);
        let ratio = clamp_ratio(row.hp as f32, mob.hp as f32);
        let pos = Vec3::new(row.x, row.y, prop_depth(row.y) + 40.0);
        // Glide speed from the previous replicated position: server ticks run
        // at 1s, so the observed delta gives a march velocity (Spec 023 §3.9).
        let speed = match tracker.last.remove(&row.mob_id) {
            Some((prev_pos, seen)) => {
                let since = seen.elapsed().as_secs_f32().max(0.05);
                (Vec2::new(row.x, row.y) - prev_pos).length() / since
            }
            None => 0.0,
        };
        tracker.last.insert(
            row.mob_id,
            (Vec2::new(row.x, row.y), Instant::now()),
        );
        commands.spawn((
            TdMobMarker,
            TdMove {
                target: Vec2::new(pos.x, pos.y),
                speed,
            },
            Sprite {
                image,
                color,
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
/// adds the storage chest as a fourth buildable.
pub const BUILDABLES: [&str; 4] = [
    cfg::ITEM_BONE_BARRICADE,
    cfg::ITEM_ARROW_TOWER,
    cfg::ITEM_BALLISTA,
    cfg::ITEM_STORAGE_CHEST,
];

/// Build menu (B) open/closed — the menu replaces blind B-cycling.
#[derive(Resource, Default)]
pub struct TdBuildMenu {
    pub open: bool,
}

/// Floor hint color per armed action. Tone-picked so the tint pops against
/// the pale-green grass tiles (teal = build, gold = capital, azure = claim).
fn hint_color(held: &str, claim_ready: bool) -> Option<Color> {
    match held {
        cfg::ITEM_CAPITAL_CORE => {
            // Gold: "here a capital city is legal".
            Some(Color::srgba(1.0, 0.82, 0.28, 0.85))
        }
        cfg::ITEM_ARROW_TOWER | cfg::ITEM_BALLISTA | cfg::ITEM_BONE_BARRICADE => {
            // Teal: "build here" (owned land).
            Some(Color::srgba(0.25, 0.95, 0.65, 0.9))
        }
        cfg::ITEM_STORAGE_CHEST => {
            // Amber-teal: the vault belongs on owned soil as well.
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
/// - holding the Capital Core → legal capital hexes (land, unclaimed, ≥8
///   from every capital) glow yellow.
/// - holding a structure item → owned hexes glow green ("build here").
/// - empty hand (no build mode) + 5 Wood + 3 Stone → the claimable ring
///   glows blue.
fn td_place_hints(
    mut commands: Commands,
    time: Res<Time>,
    mut acc: Local<f32>,
    net: Res<Net>,
    td: Res<TdTerritory>,
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
    let (phq, phr) = idlecore_core::hex::world_pos_to_hex(
        px,
        py,
        idlecore_core::world_gen::WorldGenConfig::HEX_SIZE,
    );
    let player_hex = HexCoord::new(phq, phr);

    let spawn_hint = |commands: &mut Commands, q: i32, r: i32| {
        let (cx, cy) = hex_center(q, r);
        // Full-hex overlay: √3·s wide, 2·s tall (pointy-top world hexes).
        commands.spawn((
            TdHint,
            Sprite {
                image: hex_tex.hex.clone(),
                color,
                custom_size: Some(Vec2::new(
                    f32::sqrt(3.0) * WorldGenConfig::HEX_SIZE,
                    2.0 * WorldGenConfig::HEX_SIZE,
                )),
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
                let c = HexCoord::from_id(hex_id);
                let (cx, cy) = hex_center(c.q, c.r);
                if (cx - px).powi(2) + (cy - py).powi(2) > TERRITORY_RADIUS_SQ {
                    continue;
                }
                spawn_hint(&mut commands, c.q, c.r);
            }
        }
        // Capital placement: legal = land + unowned + ≥8 from every capital.
        cfg::ITEM_CAPITAL_CORE => {
            for dq in -9..=9 {
                for dr in -9..=9 {
                    let coord = HexCoord::new(player_hex.q + dq, player_hex.r + dr);
                    let dist = player_hex.distance(&coord);
                    if dist == 0 || dist > 9 {
                        continue;
                    }
                    let (cx, cy) = hex_center(coord.q, coord.r);
                    if !idlecore_core::earth::is_land_at(cx, cy) {
                        continue;
                    }
                    let id = coord.to_id();
                    if td.owners.contains_key(&id) {
                        continue;
                    }
                    if td.capitals.iter().any(|&cap| {
                        HexCoord::from_id(cap).distance(&coord) < cfg::MIN_CAPITAL_SPACING
                    }) {
                        continue;
                    }
                    spawn_hint(&mut commands, coord.q, coord.r);
                }
            }
        }
        // Structures: anywhere on my own territory.
        cfg::ITEM_ARROW_TOWER | cfg::ITEM_BALLISTA | cfg::ITEM_BONE_BARRICADE => {
            for (&hex_id, owner) in &td.owners {
                if !mine(owner) {
                    continue;
                }
                let c = HexCoord::from_id(hex_id);
                let (cx, cy) = hex_center(c.q, c.r);
                if (cx - px).powi(2) + (cy - py).powi(2) > TERRITORY_RADIUS_SQ {
                    continue;
                }
                spawn_hint(&mut commands, c.q, c.r);
            }
        }
        // Claim: the ring around my owned hexes (unowned land only).
        "" if claim_ready => {
            let owned: Vec<HexCoord> = td
                .owners
                .iter()
                .filter(|(_, o)| mine(o))
                .map(|(&h, _)| HexCoord::from_id(h))
                .collect();
            for mine_hex in owned {
                for (nq, nr) in mine_hex.neighbors() {
                    let id = HexCoord::new(nq, nr).to_id();
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

/// Build-mode keys (Spec 023 §3.4, rev7): **B** opens/closes the buildings
/// menu (no more blind cycling); **ESC** closes it and disarms. Armed mode
/// turns the owned hexes teal and every **E** books a build there
/// (materials consumed server-side — no tower item is needed).
fn td_build_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<TdBuildMode>,
    mut menu: ResMut<TdBuildMenu>,
    mut root: Query<&mut Node, With<TdBuildMenuRoot>>,
    mut net: ResMut<Net>,
) {
    let mut changed = false;
    if keyboard.just_pressed(KeyCode::KeyB) {
        menu.open = !menu.open;
        changed = true;
        if menu.open {
            net.push(NetEvent::ServerMessage(
                "BUILDINGS: pick one — E on a lit hex to place".to_string(),
            ));
        } else if mode.kind.take().is_some() {
            net.push(NetEvent::ServerMessage("build mode off".to_string()));
        }
    }
    if keyboard.just_pressed(KeyCode::Escape) && (menu.open || mode.kind.take().is_some()) {
        menu.open = false;
        changed = true;
        net.push(NetEvent::ServerMessage("build mode off".to_string()));
    }
    if !changed {
        return;
    }
    if let Ok(mut node) = root.single_mut() {
        node.display = if menu.open { Display::Flex } else { Display::None };
    }
    net.mark_td_dirty();
}

fn close_menu(menu: &mut TdBuildMenu, root: &mut Query<&mut Node, With<TdBuildMenuRoot>>) {
    menu.open = false;
    if let Ok(mut node) = root.single_mut() {
        node.display = Display::None;
    }
}

/// Click a building row → arm that kind, close the menu, report costs.
fn td_build_row_clicks(
    rows: Query<(&Interaction, &TdBuildRow), Changed<Interaction>>,
    mut mode: ResMut<TdBuildMode>,
    mut menu: ResMut<TdBuildMenu>,
    mut root: Query<&mut Node, With<TdBuildMenuRoot>>,
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
        close_menu(&mut menu, &mut root);
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
            Color::srgb(0.1, 0.18, 0.3)
        } else {
            Color::srgb(0.09, 0.1, 0.14)
        };
        if bg.0 != want_bg {
            bg.0 = want_bg;
        }
        let want_border = if armed {
            Color::srgb(0.3, 0.95, 0.65)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.6)
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

/// Fill the row icons once the atlas crops are ready.
fn td_build_icons(td: Res<TdTextures>, mut rows: Query<(&TdBuildRow, &mut ImageNode)>) {
    if !td.ready {
        return;
    }
    for (row_entry, mut icon) in &mut rows {
        let kind = row_entry.0;
        let handle: &Handle<Image> = match kind {
            cfg::ITEM_BONE_BARRICADE => &td.icon_barricade,
            cfg::ITEM_ARROW_TOWER => &td.icon_arrow,
            cfg::ITEM_BALLISTA => &td.icon_ballista,
            // No dedicated chest crop in the atlas yet — reuse the Metal
            // Part icon tinted crate-brown (Spec 024 §2.3 placeholder).
            cfg::ITEM_STORAGE_CHEST => &td.icon_part,
            _ => continue,
        };
        if icon.image != *handle {
            icon.image = handle.clone();
        }
    }
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

#[derive(Component)]
struct TdBannerRoot;

#[derive(Component)]
struct TdBannerText;

/// Root wrapper of the B-key buildings menu (Display toggle).
#[derive(Component)]
struct TdBuildMenuRoot;

/// One selectable building row (`'static str` = the server kind name).
#[derive(Component)]
pub struct TdBuildRow(pub &'static str);

/// Row icon (ImageNode image filled once TD textures are ready).
#[derive(Component)]
struct TdBuildRowIcon;

/// Per-row cost text (red when the inventory can't pay, Spec 024).
#[derive(Component)]
struct TdBuildCost;

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

    // Buildings menu (B) — centered just above the hotbar, same panel
    // language as the top-left cluster. Hidden until toggled.
    commands
        .spawn((
            Name::new("td_build_menu_root"),
            TdBuildMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(126.0),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Name::new("td_build_panel"),
                Node {
                    width: Val::Px(330.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(9.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.9)),
                BorderColor::all(Color::srgb(0.28, 0.62, 0.5)),
            ))
            .with_children(|panel| {
                panel.spawn(((
                    Text::new("🛠 BUILDINGS"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.3, 0.95, 0.65)),
                    TextShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(1.0, 1.0),
                    },
                ),));
                panel.spawn((
                    Text::new("pick one → press E on a lit hex; ESC cancels"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.85, 0.8, 0.9)),
                ));
                for kind in BUILDABLES {
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
                            BackgroundColor(Color::srgb(0.1, 0.18, 0.3)),
                            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
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
            });
        });
}

/// Human row label for a buildable kind ("BoneBarricade" → "Barricade").
fn kind_display(kind: &str) -> &'static str {
    match kind {
        cfg::ITEM_BONE_BARRICADE => "Barricade",
        cfg::ITEM_ARROW_TOWER => "Arrow Tower",
        cfg::ITEM_BALLISTA => "Ballista",
        cfg::ITEM_STORAGE_CHEST => "Chest",
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
            **text = format!("⚔ RAID! {count} raiders marching on your capital");
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
