# Commit Message Options

## Option 1 (Conventional Commit)
```
refactor(client): migrate idlecore-client to Bevy 0.19 API

- Fix query access: get_single_mut() → single_mut()
- Update mesh API to Bevy 0.19 primitives
- Fix Player → ClientPlayer type references
- Reorganize module structure to match lib.rs
- Add world_pos_to_hex helper function
- Remove duplicate hex/renderer module declarations

break fix(server): migrate to SpacetimeDB 2.x API
- Update table access: db::table() → ctx.db.table()
- Fix entrypoints: #[entrypoint] → #[reducer]
- Add primary_key attribute to tables
- Remove obsolete attributes: #[module], #[pubsub], #[scheduled]

fix(core): resolve duplicate types and borrow issues
- Consolidate duplicate PlantType enum
- Add missing Vehicle methods: purchase_cost, display_name, all_vehicles
- Fix borrow checker in voice system
- Add Vec3/Component derives for Bevy integration
```

## Option 2 (Simplified)
```
fix: resolve all compilation errors across workspace

Workspace now compiles successfully with cargo check --workspace.

Key changes:
- idlecore-server: Migrated to SpacetimeDB 2.x API
- idlecore-chain: Fixed alloy 2.x imports
- idlecore-core: Fixed duplicate types, added missing methods
- idlecore-client: Updated to Bevy 0.19 API, fixed query access

36 files changed, 1352 insertions(+), 2356 deletions(-)
```

## Option 3 (Caveman Style)
```
fix: workspace compiles now, all errors gone

- server: update spacetimesdb api
- chain: fix alloy import
- core: merge duplicate types, add methods
- client: bevy 0.19 update, query fix

36 files, 1352 added, 2356 removed
```
