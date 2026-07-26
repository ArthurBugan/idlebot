# IdleBot — Kanban Board

## Columns

**Backlog** → **Todo** → **In Progress** → **In Review** → **Testing** → **Done**

---

## Backlog

### Phase 1: Core Loop (MVP)

#### [PB001] SpacetimeDB — Server Module Skeleton
- **Priority:** P0
- **Description:** Create the SpacetimeDB server module with `idlebot_module`. Set up the module declaration with all entrypoints, scheduled functions, pub/sub events, and the init function.
- **Definition of Done:** Module compiles, `#[module]` macro is set, all entrypoints declared.
- **Phase:** 1

#### [PB002] SpacetimeDB — Player Table & Schema
- **Priority:** P0
- **Description:** Define `player` table with all fields: address (primary key), position_x, position_y, hex_id, xp, gold, level, eco_points, last_seen, is_online, vehicle, cosmetics, templates, templates_limit. Add proper SpacetimeDB `#[table]` annotation with public access.
- **Definition of Done:** Table compiles, all fields typed correctly, primary key is address string.
- **Phase:** 1

#### [PB003] SpacetimeDB — HexTile Table & Schema
- **Priority:** P0
- **Description:** Define `hex_tile` table: hex_id (primary key), center_x, center_y, terrain (String), plant (Option<String>), is_polluted, eco_rating. Add proper annotations.
- **Definition of Done:** Table compiles, terrain stored as string, plant nullable.
- **Phase:** 1

#### [PB004] SpacetimeDB — VoiceChannel Table & Schema
- **Priority:** P1
- **Description:** Define `voice_channel` table: hex_id (primary key), players (String — JSON array), created_at, last_activity.
- **Definition of Done:** Table compiles, player list stored as JSON string.
- **Phase:** 1

#### [PB005] SpacetimeDB — MarketListing Table & Schema
- **Priority:** P2
- **Description:** Define `market_listing` table: listing_id (primary key), seller, title, github_url, description, price_usdt, published_at, sold.
- **Definition of Done:** Table compiles, price as f64.
- **Phase:** 2

#### [PB006] SpacetimeDB — Auth: Login/Logout Entrypoints
- **Priority:** P0
- **Description:** Implement `login(wallet_address, signature, nonce)` and `logout(wallet_address)` entrypoints. Login checks wallet signature against nonce, creates/updates player entry. Logout marks player offline.
- **Definition of Done:** Login creates new player with default values on first connect. Logout updates is_online=false.
- **Phase:** 1

#### [PB007] SpacetimeDB — Idle Gains Scheduled Function
- **Priority:** P0
- **Description:** Create `calculate_idle` scheduled function (every 300s). For each offline player, compute elapsed time since last_seen, apply idle gains table, update xp/gold, update last_seen.
- **Definition of Done:** Players gain XP/Gold based on offline duration tiers. Calculates correctly for all tiers.
- **Phase:** 1

#### [PB008] SpacetimeDB — Plant Update Scheduled Function
- **Priority:** P0
- **Description:** Create `update_plants` scheduled function (every 10s). For each hex_tile with a planted crop, check if grow_duration has elapsed. If ready, change stage to Ready.
- **Definition of Done:** Plants transition from Planted→Growing→Ready based on elapsed time.
- **Phase:** 1

#### [PB009] SpacetimeDB — Move Player Entrypoint
- **Priority:** P0
- **Description:** Implement `move_player(wallet_address, target_x, target_y)`. Validate player exists, update position, recalculate hex_id, publish hex_changed event.
- **Definition of Done:** Player position updates, hex_id recalculates, event published.
- **Phase:** 1

#### [PB010] SpacetimeDB — Interact Hex Entrypoint (Plant/Harvest/Clean/Clear)
- **Priority:** P0
- **Description:** Implement `interact_hex(wallet_address, hex_id, action, plant_type)`. Handle: Plant (costs 10G, creates plant entry), Harvest (gives 15G+10XP, removes plant), Clean (costs 20G, removes pollution), Clear (costs 15G, +5XP).
- **Definition of Done:** All 4 actions work correctly with proper cost/reward logic. Validation prevents invalid actions.
- **Phase:** 1

#### [PB011] SpacetimeDB — Teleport Entrypoint
- **Priority:** P1
- **Description:** Implement `teleport_player(wallet_address, target_hex_id)`. Cost: 100G. Validate gold, convert hex_id→coordinates, update position, publish event.
- **Definition of Done:** Teleport deducts gold, moves player, coordinates calculated from hex_id.
- **Phase:** 1

#### [PB012] SpacetimeDB — Buy Item Entrypoint
- **Priority:** P1
- **Description:** Implement `buy_item(wallet_address, item_type, item_name, cost)`. Validate gold, deduct cost, add item to player inventory (cosmetics or vehicle).
- **Definition of Done:** Items purchaseable, gold deducted, inventory updated.
- **Phase:** 1

#### [PB013] SpacetimeDB — Voice Channel Join/Leave Entrypoints
- **Priority:** P1
- **Description:** Implement `voice_join_hex(wallet_address, hex_id)` and `voice_leave_hex(wallet_address, hex_id)`. Join adds player to channel players array. Leave removes player, destroy channel if empty.
- **Definition of Done:** Players join/leave, channel auto-destroyed when empty.
- **Phase:** 1

#### [PB014] SpacetimeDB — Voice Cleanup Scheduled Function
- **Priority:** P1
- **Description:** Create `cleanup_voice_channels` (every 60s). Destroy voice channels where last_activity is more than 300s ago.
- **Definition of Done:** Inactive channels cleaned up.
- **Phase:** 1

#### [PB015] SpacetimeDB — Publish Template Entrypoint
- **Priority:** P2
- **Description:** Implement `publish_template(wallet_address, title, github_url, description, price_usdt)`. Cost: 50G. Create market_listing entry.
- **Definition of Done:** Listings created, gold deducted.
- **Phase:** 2

#### [PB016] SpacetimeDB — Complete Template Purchase (Blockchain Trigger)
- **Priority:** P2
- **Description:** Implement `complete_template_purchase(seller, buyer, listing_id, price_usdt)`. Mark listing sold, add template to buyer inventory, transfer gold to seller (minus 5% fee).
- **Definition of Done:** Purchase completes, templates transfer, fee deducted.
- **Phase:** 2

#### [PB017] SpacetimeDB — World Generation (Init)
- **Priority:** P0
- **Description:** Implement `init()` function to generate the initial world. Create hex tiles for the full grid (±64 axial range), distribute terrain types (Grass 50%, Forest 20%, Water 8%, City 10%, Desert 7%, Polluted 5%), set eco_rating.
- **Definition of Done:** ~12,480 hex tiles generated with correct terrain distribution.
- **Phase:** 1

#### [PB018] SpacetimeDB — Listing Cleanup Scheduled Function
- **Priority:** P2
- **Description:** Create `cleanup_old_listings` (every 3600s). Remove listings older than 30 days that are unsold.
- **Definition of Done:** Old listings removed.
- **Phase:** 2

#### [PB019] SpacetimeDB — Daily Login Reward System
- **Priority:** P1
- **Description:** Add daily login check in login entrypoint. If player last_seen is >24h ago, grant bonus: 50 XP + 25 Gold. Update last_seen to current time.
- **Definition of Done:** Returning players get bonus on first login after 24h+.
- **Phase:** 1

#### [PB020] SpacetimeDB — Scheduled Function Registration
- **Priority:** P0
- **Description:** Register all scheduled functions with correct intervals: update_plants (10s), calculate_idle (300s), cleanup_voice_channels (60s), cleanup_old_listings (3600s).
- **Definition of Done:** All 4 scheduled functions registered with correct intervals.
- **Phase:** 1

---

### Phase 2: Marketplace

#### [PB021] Blockchain — Subscription.sol Contract
- **Priority:** P2
- **Description:** Smart contract for player subscription management on Polygon. Track subscription status, expiry, active users.
- **Definition of Done:** Contract compiles, has subscribe/unsubscribe functions, subscription state tracked.
- **Phase:** 2

#### [PB022] Blockchain — TemplateMarket.sol Contract
- **Priority:** P2
- **Description:** Marketplace smart contract. Functions: publishListing, buyListing, cancelListing. Handles USDT payments, fee deduction (5%), listing state transitions.
- **Definition of Done:** Contract compiles, publish/buy/cancel work, USDT integration, fee logic.
- **Phase:** 2

#### [PB023] Blockchain — USDTInterface.sol
- **Priority:** P2
- **Description:** Interface for USDT (Polygon ERC-20). Defines transfer, balanceOf, approve functions for marketplace integration.
- **Definition of Done:** Interface compiles, covers required ERC-20 functions.
- **Phase:** 2

#### [PB024] Blockchain — idlebot-chain Crate: Wallet Auth
- **Priority:** P2
- **Description:** Implement wallet signature verification. Verify Polygon wallet signature against nonce. Returns Ok(()) on valid signature.
- **Definition of Done:** Valid signatures accepted, invalid rejected.
- **Phase:** 2

#### [PB025] Blockchain — idlebot-chain Crate: Transaction Sender
- **Priority:** P2
- **Description:** Implement transaction sending for marketplace purchases. Sign and broadcast USDT transfer tx to Polygon.
- **Definition of Done:** Transactions broadcast, receipt captured.
- **Phase:** 2

#### [PB026] Client — Hex Data Component & Lookup
- **Priority:** P0
- **Description:** Define `HexData` component with q, r, center_x, center_y, terrain enum, elevation. Create a `HashMap<(i32, i32), Entity>` for hex lookup by coords.
- **Definition of Done:** HexData component defined, lookup map exists as resource.
- **Phase:** 1

#### [PB027] Client — Procedural Hex Tile Mesh
- **Priority:** P0
- **Description:** Generate flat-top hex tile mesh programmatically. 12 vertices (top/bottom rings), 8 triangles (6 sides + top + bottom). Returns Bevy `Mesh`.
- **Definition of Done:** Hex mesh generated with correct vertex count and indices, compiles in Bevy 0.15.
- **Phase:** 1

#### [PB028] Client — World Spawn System
- **Priority:** P0
- **Description:** System that spawns all hex tiles from SpacetimeDB data. For each hex, create a Bevy entity with Transform, Mesh, Material (color by terrain), and HexData component.
- **Definition of Done:** World renders with colored hex tiles matching terrain type.
- **Phase:** 1

#### [PB029] Client — Player Placeholder Mesh (Orange Tetrahedron)
- **Priority:** P0
- **Description:** Create player placeholder: orange tetrahedron mesh (4 vertices, 4 triangles). 1.2 unit scale. Stored as asset.
- **Definition of Done:** Mesh generated, material applied (orange), renders correctly.
- **Phase:** 1

#### [PB030] Client — Player Spawn & Movement System
- **Priority:** P0
- **Description:** Spawn player entity at spawn point (center of map). WASD movement at 10 m/s (base speed). Apply vehicle speed multiplier. Update Transform based on input.
- **Definition of Done:** Player moves with WASD, speed modified by equipped vehicle.
- **Phase:** 1

#### [PB031] Client — Client ↔ Server Sync (SpacetimeDB Client)
- **Priority:** P0
- **Description:** Set up SpacetimeDB Rust client in Bevy app. Subscribe to player position updates, hex state changes, and pub/sub events. Sync entity transforms from server data.
- **Definition of Done:** Client connects to SpacetimeDB, player positions update in real-time when other players move.
- **Phase:** 1

#### [PB032] Client — Vehicle Placeholder Meshes
- **Priority:** P1
- **Description:** Create 5 vehicle placeholder meshes: Bicycle (thin frame + 2 wheels), Scooter (board + 2 small wheels), Motorcycle (fat body + 2 wheels), Boat (wide hull), Airplane (fuselage + wings). All distinct silhouettes, <60 tris each.
- **Definition of Done:** 5 meshes generated, each visually distinct from others.
- **Phase:** 1

#### [PB033] Client — Plant Placeholder Meshes
- **Priority:** P1
- **Description:** Create plant placeholder meshes for each type + stage combo. Wheat (0.3/0.6/0.9 height), Tomato (0.3/0.6/0.8), Sunflower (0.4/0.7/1.0), RareHerb (0.2/0.5/0.8). Color changes with stage (dull → vibrant).
- **Definition of Done:** 20 plant meshes (5 types × 4 stages including None), each with appropriate height and color.
- **Phase:** 1

#### [PB034] Client — Tree Placeholder Mesh (Reused)
- **Priority:** P1
- **Description:** Use existing `create_tree_mesh()` from procedural.rs (trunk + cone canopy). Add as asset. Color variations: normal green, forest dark green.
- **Definition of Done:** Tree mesh renders on Forest terrain hexes.
- **Phase:** 1

#### [PB035] Client — Pollution Marker Mesh
- **Priority:** P1
- **Description:** Create pollution marker: flat irregular disc (radius 8), dark purple, slight emissive. Renders on Polluted hex tiles.
- **Definition of Done:** Purple disc visible on polluted hexes, contrasts with terrain.
- **Phase:** 1

#### [PB036] Client — City Building Placeholder Meshes
- **Priority:** P1
- **Description:** Create 3 building placeholder meshes for City terrain: tall (gray box), medium (brown box), low (tan box). Varying heights for visual diversity.
- **Definition of Done:** 3 building meshes, render on City hexes.
- **Phase:** 1

#### [PB037] Client — Voice Channel Indicator (Glowing Ring)
- **Priority:** P1
- **Description:** Create voice channel indicator: glowing cyan ring under player(s) in active voice channel. Flat torus shape, emissive material.
- **Definition of Done:** Cyan ring visible under players in voice channel hex.
- **Phase:** 1

#### [PB038] Client — Camera & Viewport Setup
- **Priority:** P0
- **Description:** Configure main 3D camera (isometric-ish, looking down at angle). Configure minimap 2D camera (top-down orthographic).
- **Definition of Done:** Main camera shows game world, minimap shows overview.
- **Phase:** 1

#### [PB039] Client — Wallet Connection UI
- **Priority:** P2
- **Description:** Create a startup screen with "Connect Wallet" button. On click, trigger Polygon wallet connection flow. Display wallet address on success.
- **Definition of Done:** Wallet connects, address displayed, enters game on success.
- **Phase:** 2

#### [PB040] Client — Interaction UI (HUD)
- **Priority:** P1
- **Description:** Create HUD overlay: interaction buttons (Plant, Harvest, Clean, Clear) visible when player is on an interactable hex. Show available actions based on hex state.
- **Definition of Done:** HUD shows correct buttons based on hex terrain and plant state.
- **Phase:** 1

#### [PB041] Client — Minimap
- **Priority:** P1
- **Description:** 2D minimap rendering: top-down view of nearby hexes, player position marker, other players' positions. Click hex on minimap = request teleport.
- **Definition of Done:** Minimap renders, shows player + nearby players, clickable for teleport.
- **Phase:** 1

#### [PB042] Client — Global Map View
- **Priority:** P2
- **Description:** Global map overlay: full hex grid overview. Click any hex = teleport request (costs 100G). Zoom levels for navigation.
- **Definition of Done:** Full map visible, clickable, shows teleport cost.
- **Phase:** 2

#### [PB043] Client — Marketplace UI
- **Priority:** P2
- **Description:** Marketplace screen: list view of templates, search/filter, publish form (title, description, GitHub URL, price), buy button.
- **Definition of Done:** Can browse, publish, and buy templates through UI.
- **Phase:** 2

#### [PB044] Client — Voice Chat System Integration
- **Priority:** P1
- **Description:** Integrate str0m WebRTC for voice. Auto-create datachannel when entering hex with other players. Send/receive audio packets. Non-positional audio within hex.
- **Definition of Done:** Players in same hex can hear each other's voice.
- **Phase:** 1

#### [PB045] Client — Teleport Confirmation UI
- **Priority:** P1
- **Description:** When player requests teleport (via map click or UI), show confirmation dialog: "Teleport to [hex]? Cost: 100 Gold". Confirm/deny flow.
- **Definition of Done:** Confirmation dialog works, teleport executes on confirm.
- **Phase:** 1

---

### Phase 3: Content & Polish

#### [PB046] Client — Vehicle Equip & Movement Integration
- **Priority:** P1
- **Description:** When vehicle purchased, update player display mesh (show vehicle alongside/under player). Update movement speed multiplier.
- **Definition of Done:** Vehicle mesh appears, movement speed matches multiplier.
- **Phase:** 3

#### [PB047] Client — Cosmetic System (Hat/Aura/Trail)
- **Priority:** P2
- **Description:** Cosmetic system: player can equip Hat, Aura, Trail. Each cosmetic is a child entity attached to player. Visual only.
- **Definition of Done:** Cosmetics attach to player mesh, visible to self and others.
- **Phase:** 3

#### [PB048] Client — Player Animation (Idle Wiggle)
- **Priority:** P2
- **Description:** Add subtle idle animation to player: gentle bob/wiggle when not moving. Simple scale oscillation or rotation.
- **Definition of Done:** Player mesh has visible idle animation.
- **Phase:** 3

#### [PB049] Client — Plant Growth Animation
- **Priority:** P2
- **Description:** Smooth transition between plant growth stages: scale animation from Planted→Growing→Ready over a few seconds.
- **Definition of Done:** Plants animate smoothly between stages.
- **Phase:** 3

#### [PB050] Client — Pollution Cleanup VFX
- **Priority:** P2
- **Description:** Visual effect when cleaning pollution: green particles spawning and rising from hex, pollution disc fading out.
- **Definition of Done:** Cleanup action triggers particle VFX.
- **Phase:** 3

#### [PB051] Client — Low-Poly Asset Pipeline
- **Priority:** P2
- **Description:** Set up asset import pipeline: convert FBX/OBJ to glTF or use Bevy's built-in FBX loader. Import selected low-poly packs (terrain, forest, city, vehicles).
- **Definition of Done:** Low-poly assets import and render correctly in Bevy.
- **Phase:** 3

#### [PB052] Client — Terrain Textures & Materials
- **Priority:** P2
- **Description:** Replace flat color terrain materials with low-poly textures (grass texture, water animation, sand, etc.). Optional: vertex coloring for variety.
- **Definition of Done:** Terrain looks polished, not just flat colors.
- **Phase:** 3

#### [PB053] Client — Performance: LOD System
- **Priority:** P2
- **Description:** Implement Level of Detail for hex tiles: full mesh up close, simplified mesh at distance. Reduces draw calls for 12,480 hexes.
- **Definition of Done:** Distant hexes use lower-poly meshes, no visual pop-in.
- **Phase:** 3

#### [PB054] Client — Performance: Entity Culling
- **Priority:** P2
- **Description:** Cull entities outside camera frustum. Only render hexes and players within visible area.
- **Definition of Done:** Off-screen entities don't draw, performance improved.
- **Phase:** 3

---

## Todo

<!-- Tasks moved here from Backlog when ready to work on -->

## In Progress

<!-- Tasks currently being worked on -->

## In Review

<!-- Tasks completed, awaiting Ferris review -->

## Testing

<!-- Tasks verified, ready for QA -->

## Done

<!-- Completed tasks -->
