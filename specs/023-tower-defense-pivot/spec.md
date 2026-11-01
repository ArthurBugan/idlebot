# Spec 023: Tower Defense Pivot — Land Claim, Capital City, Raids

> **Objective:** Transform the game loop into a tower-defense MMORPG layer on the
> shared Earth hex world: players walk, claim land around a placeable **Capital
> City**, spend gathered resources and mob drops (Metal Parts, Bones) on
> defense structures, and server-driven mob raids march into the territory to
> destroy the Capital.
>
> **Decisions (user-confirmed):** shared persistent world (no instancing) ·
> existing systems (idle gains, farming, vehicles, marketplace, voice, wallet
> auth) keep running untouched · players do not fight directly — towers do.

## 1. Problem Statement

The current loop is gathering + crafting (Spec 022) with no threat, no loss, and
no long-term spatial goal. Everything is kept and **additive**: resource scarcity
(farming/gathering) now feeds defense construction, and the idle/farm/market
economy continues under Constitution rules 7/8 without changes.

## 2. New Core Loop

```
WALK → CLAIM land → PLACE capital → BUILD defenses
      ↑                                      ↓
 GATHER (ground nodes, mob drops) ← RAID WAVE (mobs march on the capital)
```

1. **Spawn:** new players receive one **Capital Core** item (`new_player` grants it). It can be dropped exactly once on a legal hex — placing it **is** founding the Capital City.
2. **Claim:** the player walks to an unowned hex **adjacent to owned territory**, pays a resource cost, and the hex becomes theirs (owner tint on world + minimap). Claimed area is the only place where defenses can be built.
3. **Build:** on a **claimed** hex, the player consumes items (Wood, Stone, Metal Part, Bone…) to build towers/walls on empty plot slots (same slot/space rules as Spec 022 objects).
4. **Raid:** a server scheduler builds **raid waves** per capital scaled by pressure (territory size, capital level, time since last raid). Mobs spawn at the territory border ring and march toward the Capital.
5. **Fight (towers only):** towers in range auto-attack mobs. Mobs destroy structures standing in their way, then the Capital.
6. **Outcome:** mobs killed → item drops (Metal Part, Bone…) + Capital XP → capital level up (bigger claim radius, harder waves). Capital destroyed → territory released, structures removed, the Capital Core item is returned, and a 24h "no-raids" grace timer starts for the next placement.

## 3. Design

### 3.1 Capital City
- **Placing:** `place_capital(hex_id)` — requires: hex not spawned as Water, hex unclaimed, ≥ `MIN_CAPITAL_SPACING` (8) hexes from every other capital, player has the Capital Core item. Consumes the item; binds the player's identity to that capital.
- **Stats:** `hp / max_hp` (100 at L1), `level` (1), `last_raid_at`, `destroyed: bool`.
- **Level-up:** capital XP = per-mob kill; thresholds use `100 · level²` (same curve as `progression.rs`). Each level: +1 max claim radius (see below), +15% wave size.
- **Destruction:** `hp ≤ 0` → removed structures, territory freed, Capital Core returned to inventory (not placed), `grace_until = now + 24h`.

### 3.2 Land Claim
- `claim_land(hex_id)`: hex must be **unclaimed**, **walkable terrain** (all but Water), and adjacent (hex `distance == 1`) to a hex owned by the player, within the capital's claim radius (L1 → 2 hexes, +1 per level, max 5). Cost: **5 Wood + 3 Stone**. Reuses the per-player 5s action cooldown.
- One player ⇢ `N` hexes keyed by owner address; a `territory` table is the source of truth (single PK = hex id). Overlap is impossible since the claim reducer checks ownership atomically.
- Claimed hexes render with an owner tint; minimap draws the border.

### 3.3 Resources & Drops
- New inventory items (reusing the `player_item` table): **Metal Part**, **Bone**, **Capital Core**.
- Sources:
  - **Mob drops** (auto-credited to the defender's inventory on kill): Scavenger → Bone (40%), Rust Husk → Metal Part (50%), Bone Walker → Bone (90%).
  - **Ground nodes** (existing gather flow): Rock gains a 25% Metal Part chance; Tree keeps dropping Wood; no new node kinds in MVP.
- Drops are guaranteed-quantity at MVP (no roll tables beyond the chance above).

### 3.4 Defense Structures (MVP set)
Built via `build_defense(hex_id, slot_x, slot_y, kind)` on an owned hex, using the same empty-slot rules (`require_empty_cell`) as Spec 022. One structure per slot; max 6 per hex (existing `MAX_OBJECTS_PER_HEX` budget shared).

| Kind | Cost | HP | Range (hex) | DPS | Notes |
|---|---|---|---|---|---|
| Bone Barricade | 6 Bone | 60 | — | — | Wall: mobs must destroy it to pass |
| Arrow Tower | 8 Wood + 4 Stone + 1 Metal Part | 80 | 2 | 10 | Single target, nearest mob |
| Ballista | 6 Wood + 4 Metal Part + 8 Bone | 60 | 3 | 22 | Slow single target, pierces 1 |

- Structures persist between raids; damage heals only via `repair_structure` (cost: 2 Wood per 10 HP, Phase 2 — MVP towers heal to full at raid end if the capital survived).

### 3.5 Mobs & Raids
- **Kinds:** `Scavenger` (30 HP, 4 DPS), `Bone Walker` (80 HP, 8 DPS), `Rust Husk` (140 HP, 12 DPS). Unlock by capital level: L1 = Scavenger, L2 += Bone Walker, L3 += Rust Husk.
- **Pressure:** `wave_size = ceil((2 + claimed_hexes/2) * 1.15^(level-1))`, capped at 12 concurrent mobs per capital; one wave at a time.
- **Timing:** a 30s `raid_tick` scheduler rolls waves. First raid only after `GRACE_WINDOW = 30 min` since capital placement; re-attack cooldown ≥ 10 min after the previous wave ends.
- **Spawning:** waves spawn at territory border hexes (owned hex adjacent to unowned/water); if the territory is a single hex, at ring distance 2. Positions distribute across 1–2 border points.
- **Movement/AI (MVP):** straight-line march toward the capital hex center, hex step per `mob_tick` (1s) — no pathfinding graph. Engagement priority: (1) any structure within on-hex adjacency → melee it, (2) towers only ever attack them (they don't retaliate at range in MVP), (3) capital in adjacency → attack capital.
- **Leash/despawn:** raid ends when all mobs die or 15 min elapse → surviving mobs despawn (no drop), wave marked ended.

### 3.6 Server Schedulers (new)
Following the existing scheduled-table pattern of `scheduler.rs`:

| Scheduled reducer | Interval | Body |
|---|---|---|
| `mob_tick` | 1 s | Move mobs, tower targeting/fire, damage application, death → drops |
| `raid_tick` | 30 s | Spawn waves per eligible capital, end stale raids, destruction cascade |

Existing schedulers (idle, plant growth, voice, market, eco) are untouched.

### 3.7 New Tables
Owned by `idlecore-server/src/types.rs` (schema additions, mirroring Spec 019 style):
- `capital` (PK id; owner, hex_id, hp, level, xp, grace_until, destroyed)
- `territory` (PK hex_id; owner_address, capital_id, claimed_at)
- `defense_structure` (PK id; capital_id, hex_id, slot_x/slot_y, kind, hp, built_at)
- `mob` (PK id; capital_id, wave_id, kind, hex_id, x, y, hp, state)
- `raid_wave` (PK id; capital_id, spawned_at, ended_at, mob_ids)
- scheduled: `scheduled_mob_tick`, `scheduled_raid_tick` (+ wrappers)
All new tables replicate automatically — the client already subscribes to all tables.

### 3.8 Reducers (new)
`place_capital(hex_id)` · `claim_land(hex_id)` · `build_defense(hex_id, slot_x, slot_y, kind)` · `remove_defense(structure_id)` (50% resource refund).

### 3.9 Client Changes
- **World view:** owner tint for claimed hexes; capital sprite + HP bar; tower sprites (Tiny Dungeon crate/tile placeholders); mob sprites (Tiny Dungeon monster tiles) — mobs glide between 1s server ticks like remote players do today.
- **Minimap:** territory borders + capital marker + red pulse on incoming waves.
- **Build mode:** with a structure item in the hotbar, E builds on the selected slot of an owned hex (mirrors Spec 022 bench placement flow); Capital Core in hand + E places the capital.
- **HUD:** capital HP + level, wave incoming banner with countdown, kill/drop toasts.
- No combat controls are added — towers are autonomous.

## 4. Non-Goals (this spec)
- PvP raiding between player capitals; co-op/alliance capitals (shared defense).
- Unclaim, capital relocation, structure upgrades/tiers, projectile visuals.
- True pathfinding (A*); mob ranged attacks; player weapons.
- Extending idle/farm/market systems — they are frozen for this spec.

## 5. Balance Invariants (Constitution 7/8 rules preserved)
- Existing plant-cycle net (+5 Gold, +15 XP) untouched; no defensive economy mutates it.
- Offline gains unchanged; **raids run regardless of the defender's online status** (user decision — offline capitals are raidable, subject only to the grace windows in §3.5).

## Acceptance Criteria
1. `place_capital` consumes the Capital Core, creates the capital row and grants the claim radius; illegal placements (Water, spacing, double placement) are rejected.
2. `claim_land` requires adjacency + radius + resources; two players cannot claim the same hex; a claimed hex appears tinted on world + minimap.
3. `build_defense` only succeeds on owned hexes with a free slot and consumes exact costs.
4. Waves spawn after grace only, at the border, sized by pressure, and never exceed 12 concurrent mobs per capital.
5. Mobs march toward the capital, melee structures in the way, and towers kill them within range; kills credit drops (Bone/Metal Part) to the defender and capital XP.
6. Capital at 0 HP triggers the destruction cascade (territory freed, structures removed, Capital Core returned, 24h grace).
7. Existing tests for idle/farm/market/eco pass without behavior changes.
8. Headless e2e: login → place capital → claim → build → force a scheduled wave → capital survives with ≥1 tower.

## Open Questions
- Which Tiny Dungeon tiles best read as "tower" vs "wall" before we commission art?
