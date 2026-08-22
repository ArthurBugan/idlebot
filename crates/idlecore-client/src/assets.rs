//! 2D asset helpers (Spec 016 T2.2/T2.3) and trail VFX (Spec 016 T5.5).
//!
//! World tiles come from the isometric sprite packs under `assets/models/`
//! (loaded by `world_floor`); vehicles/cosmetics render as 2D sprites;
//! trails and bursts are expanding colored diamonds.

use bevy::prelude::*;
use bevy::time::Time;
use idlecore_core::assets::{CosmeticAssetType, VehicleAssetType, vehicle_material_spec};
use idlecore_core::Vehicle;

use crate::player::ClientPlayer;
use crate::plugins::player::PhysicsBody;

/// Maps a server `Vehicle` to the asset pipeline's `VehicleAssetType`.
pub fn to_asset_type(vehicle: &Vehicle) -> VehicleAssetType {
    match vehicle {
        Vehicle::None => VehicleAssetType::None,
        Vehicle::Bicycle => VehicleAssetType::Bicycle,
        Vehicle::Scooter => VehicleAssetType::Scooter,
        Vehicle::Motorcycle => VehicleAssetType::Motorcycle,
        Vehicle::Boat => VehicleAssetType::Boat,
        Vehicle::Airplane => VehicleAssetType::Airplane,
    }
}

/// Initializes the 2D VFX resources (trail, burst, cosmetic layers).
pub fn load_all_assets(mut commands: Commands) {
    commands.init_resource::<TrailFx>();
    commands.init_resource::<BurstFx>();
    commands.init_resource::<CosmeticLayers>();
}

/// Draw-order helpers: everything VFX-related renders above tiles
/// (z = 1000 - y, see `world_floor`) but below the player (+50).
fn vfx_depth(y: f32, offset: f32) -> f32 {
    crate::world_floor::prop_depth(y) + offset
}

// ============================================================================
// Trail VFX (Spec 016 T5.5)
// ============================================================================

/// Per-vehicle flyweight spawned at the player position while riding.
#[derive(Component)]
pub struct TrailParticle {
    pub remaining: f32,
}

/// Trail VFX controller (Spec 016 T5.5).
#[derive(Resource, Default)]
pub struct TrailFx {
    pub accumulator: f32,
    pub active: bool,
}

/// Emits small colored diamonds behind the player while a trailing vehicle is
/// equipped and the player is moving.
#[allow(clippy::too_many_arguments)]
pub fn update_trail_vfx(
    body: Query<(&Transform, &ClientPlayer), (With<PhysicsBody>, Without<TrailParticle>)>,
    mut fx: ResMut<TrailFx>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok((body_t, player)) = body.single() else { return };
    let Some(vehicle) = player
        .owned_vehicle
        .as_ref()
        .filter(|v| **v != Vehicle::None)
    else {
        fx.active = false;
        fx.accumulator = 0.0;
        return;
    };
    let spec = vehicle_material_spec(to_asset_type(vehicle));
    if !spec.has_trail {
        fx.active = false;
        return;
    }
    if player.velocity.length_squared() < 0.01 {
        fx.active = false;
        return;
    }
    fx.active = true;
    fx.accumulator += time.delta_secs();
    let mut spawned = 0usize;
    while fx.accumulator >= spec.trail_interval && spawned < 8 {
        fx.accumulator -= spec.trail_interval;
        spawned += 1;
        commands.spawn((
            Name::new("trail-particle"),
            Sprite {
                color: Color::srgb(spec.r, spec.g, spec.b),
                custom_size: Some(Vec2::splat(0.4)),
                ..default()
            },
            Transform::from_xyz(
                body_t.translation.x,
                body_t.translation.y,
                vfx_depth(body_t.translation.y, 20.0),
            )
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            TrailParticle {
                remaining: spec.trail_lifetime,
            },
        ));
    }
}

/// Fades out and despawns expired trail particles.
pub fn expire_trail_particles(
    mut commands: Commands,
    mut particles: Query<(&mut TrailParticle, Entity)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut p, entity) in &mut particles {
        p.remaining -= dt;
        if p.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::time::TimePlugin;
    use idlecore_core::assets::vehicle_material_spec;

    #[test]
    fn bicycle_material_is_painted_steel() {
        let spec = vehicle_material_spec(VehicleAssetType::Bicycle);
        assert!(spec.metallic > 0.5);
        assert!(spec.roughness < 0.5);
        assert!(spec.has_trail);
    }

    #[test]
    fn to_asset_type_maps_identically() {
        assert_eq!(to_asset_type(&Vehicle::None), VehicleAssetType::None);
        assert_eq!(to_asset_type(&Vehicle::Airplane), VehicleAssetType::Airplane);
    }

    #[test]
    fn trail_defaults_inactive() {
        let fx = TrailFx::default();
        assert!(!fx.active);
        assert_eq!(fx.accumulator, 0.0);
    }

    #[test]
    fn trail_system_no_ops_without_body() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<TrailFx>();
        app.add_systems(Update, update_trail_vfx);
        app.update();
        app.update();
    }
}

// ============================================================================
// Cosmetic layers on the avatar (Spec 016 T4.2/T4.3 — 2D sprites)
// ============================================================================

/// Which cosmetic layer set is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CosmeticMode {
    #[default]
    None,
    Hat,
    Aura,
    Both,
}

/// Cycles None → Hat → Aura → Both → None (pure, testable).
pub fn next_cosmetic_mode(mode: CosmeticMode) -> CosmeticMode {
    match mode {
        CosmeticMode::None => CosmeticMode::Hat,
        CosmeticMode::Hat => CosmeticMode::Aura,
        CosmeticMode::Aura => CosmeticMode::Both,
        CosmeticMode::Both => CosmeticMode::None,
    }
}

#[derive(Resource, Default)]
pub struct CosmeticLayers {
    pub mode: CosmeticMode,
}

/// Marks a spawned cosmetic layer and its category.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct CosmeticLayerKind(pub CosmeticAssetType);

/// Spawns the hat (orange square above the head) and the aura ring (green
/// diamond behind the player), both hidden until toggled. Children of the
/// player body; the aura sits at z -1 so it renders behind the sprite.
pub fn spawn_cosmetic_layers(
    mut commands: Commands,
    bodies: Query<Entity, With<PhysicsBody>>,
    already_spawned: Query<(), With<CosmeticLayerKind>>,
) {
    if !already_spawned.is_empty() {
        return;
    }
    // Player root may not exist on the first frames; retry until it does.
    let Ok(player_root) = bodies.single() else { return };
    commands.spawn((
        Name::new("cosmetic-hat"),
        CosmeticLayerKind(CosmeticAssetType::Hat),
        Sprite {
            color: Color::srgb(1.0, 0.55, 0.1),
            custom_size: Some(Vec2::splat(0.9)),
            ..default()
        },
        Transform::from_xyz(0.0, 3.1, -0.5),
        Visibility::Hidden,
        ChildOf(player_root),
    ));
    commands.spawn((
        Name::new("cosmetic-aura"),
        CosmeticLayerKind(CosmeticAssetType::Aura),
        Sprite {
            color: Color::srgba(0.2, 1.0, 0.5, 0.35),
            custom_size: Some(Vec2::splat(7.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
        Visibility::Hidden,
        ChildOf(player_root),
    ));
}

/// Shows layers per the current `CosmeticLayers.mode`.
pub fn sync_cosmetic_layers(
    state: Option<Res<CosmeticLayers>>,
    mut layers: Query<(&CosmeticLayerKind, &mut Visibility)>,
) {
    for (kind, mut vis) in &mut layers {
        let mode = state.as_ref().map(|s| s.mode).unwrap_or_default();
        let on = match (mode, kind.0) {
            (CosmeticMode::None, _) => false,
            (CosmeticMode::Hat, CosmeticAssetType::Hat) => true,
            (CosmeticMode::Aura, CosmeticAssetType::Aura) => true,
            (CosmeticMode::Both, _) => true,
            _ => false,
        };
        *vis = if on { Visibility::Visible } else { Visibility::Hidden };
    }
}

/// J cycles through cosmetic layer modes.
pub fn toggle_cosmetic_layers(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CosmeticLayers>,
) {
    if keys.just_pressed(KeyCode::KeyJ) {
        state.mode = next_cosmetic_mode(state.mode);
    }
}

#[cfg(test)]
mod cosmetic_tests {
    use super::*;
    use bevy::app::App;

    #[test]
    fn mode_cycles_through_all_states() {
        let mut mode = CosmeticMode::None;
        let seen = vec![CosmeticMode::Hat, CosmeticMode::Aura, CosmeticMode::Both, CosmeticMode::None];
        for expected in seen {
            mode = next_cosmetic_mode(mode);
            assert_eq!(mode, expected);
        }
    }

    #[test]
    fn sync_shows_only_selected_layers() {
        let mut app = App::new();
        app.insert_resource(CosmeticLayers::default());
        let root = app.world_mut().spawn_empty().id();
        let mut commands = app.world_mut().commands();
        let mut spawn = |kind: CosmeticAssetType, y: f32| {
            commands.spawn((
                CosmeticLayerKind(kind),
                Sprite::default(),
                Transform::from_xyz(0.0, y, 0.0),
                Visibility::Hidden,
                ChildOf(root),
            ));
        };
        spawn(CosmeticAssetType::Hat, 1.5);
        spawn(CosmeticAssetType::Aura, 1.0);
        app.add_systems(Update, sync_cosmetic_layers);
        app.update();
        {
            let mut layers = app.world_mut().query::<(&CosmeticLayerKind, &Visibility)>();
            for (kind, vis) in layers.iter(app.world()) {
                assert_eq!(*vis, Visibility::Hidden, "{kind:?} should be hidden");
            }
        }
        app.world_mut()
            .resource_mut::<CosmeticLayers>()
            .mode = CosmeticMode::Hat;
        app.update();
        {
            let mut layers = app.world_mut().query::<(&CosmeticLayerKind, &Visibility)>();
            for (kind, vis) in layers.iter(app.world()) {
                let expect = matches!(kind.0, CosmeticAssetType::Hat);
                assert_eq!(*vis == Visibility::Visible, expect, "{kind:?} mismatch");
            }
        }
    }

    #[test]
    fn sync_no_ops_without_state() {
        // Res<CosmeticLayers> missing: system must be skipped gracefully.
        let mut app = App::new();
        app.world_mut().spawn((CosmeticLayerKind(CosmeticAssetType::Hat), Visibility::Hidden));
        app.add_systems(Update, sync_cosmetic_layers);
        app.update();
    }
}

// ============================================================================
// Burst / explosion VFX (Spec 016 T5.6 — optional, done on teleport arrival)
// ============================================================================

/// One expanding diamond of a teleport burst.
#[derive(Component)]
pub struct ExpandingParticle {
    pub t: f32,
    pub life: f32,
    pub dir: Vec3,
}

/// Progress 0..1 of a burst particle's lifetime (pure).
pub fn burst_step(t: f32, life: f32) -> f32 {
    (t / life).clamp(0.0, 1.0)
}

/// Size multiplier while an expanding particle lives: 1.0 → 3.0.
pub fn burst_scale(progress: f32) -> f32 {
    1.0 + 2.0 * progress
}

/// Pending burst requests (filled by net_drain on teleport/explosion).
#[derive(Resource, Default)]
pub struct BurstFx {
    pub pending: Vec<Vec3>,
}

impl BurstFx {
    pub fn request(&mut self, at: Vec3) {
        self.pending.push(at);
    }
}

const BURST_PARTICLES: usize = 8;
const BURST_LIFE: f32 = 0.6;
const BURST_SPREAD: f32 = 1.2;

/// Consumes pending requests and spawns an 8-diamond expanding ring.
pub fn update_burst_vfx(
    mut burst: ResMut<BurstFx>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for at in burst.pending.drain(..) {
        for k in 0..BURST_PARTICLES {
            let angle = k as f32 * std::f32::consts::TAU / BURST_PARTICLES as f32;
            let dir = Vec3::new(angle.cos(), angle.sin(), 0.0);
            commands.spawn((
                Name::new("burst-particle"),
                Sprite {
                    color: Color::srgb(1.0, 0.8, 0.2),
                    custom_size: Some(Vec2::splat(0.9)),
                    ..default()
                },
                Transform::from_xyz(at.x, at.y, vfx_depth(at.y, 20.0))
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                ExpandingParticle { t: 0.0, life: BURST_LIFE, dir },
            ));
        }
    }
    let _ = time;
}

/// Ages and despawns burst particles; expansion is applied by
/// `apply_burst_expansion` (separate system, borrow-clean).
pub fn expire_burst_particles(
    mut commands: Commands,
    mut particles: Query<(Entity, &mut ExpandingParticle)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (entity, mut p) in &mut particles {
        p.t += dt;
        if p.t >= p.life {
            commands.entity(entity).despawn();
        }
    }
}

/// Applies outward expansion + scale growth to living burst particles.
pub fn apply_burst_expansion(
    mut particles: Query<(&ExpandingParticle, &mut Transform)>,
) {
    for (p, mut t) in &mut particles {
        let progress = burst_step(p.t, p.life);
        let scale = burst_scale(progress);
        t.translation.x += p.dir.x * BURST_SPREAD * progress;
        t.translation.y += p.dir.y * BURST_SPREAD * progress;
        t.scale = Vec3::splat(scale);
    }
}

#[cfg(test)]
mod burst_tests {
    use super::*;
    use bevy::app::App;
    use bevy::time::TimePlugin;

    #[test]
    fn burst_math_is_monotonic() {
        for i in 0..10u32 {
            let t = i as f32;
            let p = i as f32 / 10.0;
            assert!((burst_step(t, 10.0) - p).abs() < 1e-6);
            assert!(burst_scale(0.0) >= 1.0);
        }
        assert!((burst_scale(1.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn requests_accumulate_until_drained() {
        let mut fx = BurstFx::default();
        assert!(fx.pending.is_empty());
        fx.request(Vec3::new(1.0, 2.0, 3.0));
        fx.request(Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(fx.pending.len(), 2);
    }

    #[test]
    fn update_spawns_ring_and_consumes_requests() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.init_resource::<BurstFx>();
        app.world_mut().resource_mut::<BurstFx>().request(Vec3::ZERO);
        app.add_systems(Update, update_burst_vfx);
        app.update();
        assert!(app.world_mut().resource::<BurstFx>().pending.is_empty());
        let count = app.world_mut().query::<&ExpandingParticle>().iter(app.world()).count();
        assert_eq!(count, BURST_PARTICLES);
    }

    #[test]
    fn expire_removes_old_particles() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.world_mut().spawn((
            ExpandingParticle { t: 5.0, life: 1.0, dir: Vec3::Y },
            Transform::default(),
        ));
        app.add_systems(Update, expire_burst_particles);
        app.update();
        let count = app.world_mut().query::<&ExpandingParticle>().iter(app.world()).count();
        assert_eq!(count, 0);
    }
}