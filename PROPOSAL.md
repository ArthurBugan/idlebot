# IdleBot — Design Specification

> **Idle Tamagotchi × Voice Chat × Code Marketplace on a 3D Hex Grid**

---

## 1. Concept

IdleBot is a multiplayer idle game where you manage a Tamagotchi-like character that grows XP and Gold even when you're offline. The game world is a shared 3D hex grid where players meet, chat by voice, farm, clean pollution, and trade AI agents and code templates on an on-chain marketplace.

**Core loop:** Idle → Collect → Interact → Trade → Grow.

---

## 2. Gameplay Systems

### 2.1 Character (Tamagotchi)

- Represented as a small 3D avatar (placeholder: orange tetrahedron)
- Has: XP, Gold, Level, Eco Points, Position
- Progression: `level = calculate_level(total_xp)` — formula `100 * level²`
- Cosmetics: Hat, Aura, Trail, Vehicle Skin (purchased with Gold or USDT)
- Vehicle: Bicycle, Scooter, Motorcycle, Boat, or Airplane

### 2.2 Idle Gains

Offline XP and Gold accumulate based on elapsed time:

| Offline Duration | XP Gained | Gold Gained |
|-----------------|-----------|-------------|
| < 1 hour        | 10        | 5           |
| 1–6 hours       | 60        | 30          |
| 6–12 hours      | 100       | 50          |
| 12–24 hours     | 150       | 75          |
| Max: 24 hours   | 150       | 75          |

Gains are calculated server-side via a SpacetimeDB scheduled function (every 5 minutes).

### 2.3 Actions (On-World)

| Action          | Cost (Gold) | Reward       | XP  | Notes                           |
|-----------------|-------------|--------------|-----|---------------------------------|
| Plant           | 10          | —            | 5   | Requires empty hex              |
| Harvest         | 0           | 15 Gold      | 10  | Requires mature plant           |
| Clean Pollution | 20          | 20 Gold      | 15  | Requires polluted hex           |
| Clear Terrain   | 15          | —            | 5   | Removes obstacle                |
| Publish Template| 50          | Listing on market | — | Requires GitHub URL            |

**Economy note:** Planting costs 10G, harvesting returns 15G — a net +5G per cycle plus 15 XP. Planting drives the idle loop forward.

### 2.4 Plant Growth

- Plants go through stages: Planted → Growing → Ready
- Growth time varies by type (Wheat fast, Tree slow, RareHerb longest)
- Server checks every 10 seconds for ready plants
- Harvesting gives XP + Gold reward

### 2.5 Vehicles

| Vehicle       | Speed Multiplier | Gold Cost |
|---------------|-----------------|-----------|
| None          | 1.0x            | 0         |
| Bicycle       | 2.0x            | 500       |
| Scooter       | 3.0x            | 1,000     |
| Motorcycle    | 5.0x            | 2,500     |
| Boat          | 4.0x            | 2,000     |
| Airplane      | 10.0x           | 10,000    |

All vehicles are electric (thematic consistency with conservation). Speed affects movement speed on the hex grid.

### 2.6 Teleport

- Cost: 100 Gold
- Teleport to any hex visible on the map
- Required for traversing large distances on foot

---

## 3. World

### 3.1 Structure

- **Type:** 3D, shared, hexagonal grid
- **Scale:** 1:10,000 (game coordinates × 10,000 = approximate real-world meters)
- **Hex radius:** 10 meters (game units)
- **Map size:** ~64 hexes radius in each direction (axial coords)
- **Total hexes:** ~12,480 (with axial constraint `|s| <= map_radius`)

### 3.2 Terrain Types

| Terrain    | Probability | Eco Rating | Description               |
|------------|-------------|------------|---------------------------|
| Grass      | 50%         | 50         | Default, farmable         |
| Forest     | 20%         | 50         | Natural, eco-positive     |
| Water      | 8%          | 20         | Non-farmable              |
| City       | 10%         | 20         | Urban, buildings visible  |
| Desert     | 7%          | 20         | Dry, low eco              |
| Polluted   | 5%          | 10         | Needs cleaning            |

### 3.3 Interaction

- Players interact with hexes by proximity (within the same hex)
- Clicking a hex on the minimap or global map triggers teleport
- Hexes are visible as flat-top 3D tiles with color based on terrain type
- Plants grow on top of hexes, pollution shows as dark markers

### 3.4 Voice Chat

- **Radius:** 50 meters (one hex radius)
- **Type:** Within-hex channel — all players in the same hex hear each other
- **Audio:** Non-positional (no attenuation within hex, like a room)
- **Mechanics:** Voice channel auto-created when player enters a hex with others
- **Cleanup:** Channels destroyed when all players leave (5 min timeout)

### 3.5 Economy

- **Gold:** Earned via idle gains + harvesting. Spent on planting, cleaning, publishing templates, vehicles, cosmetics, teleport.
- **USDT:** Premium currency. Used for marketplace template purchases via smart contract.
- **Eco Points:** Earned by cleaning pollution and planting trees. Affects eco rating of hexes.

---

## 4. Marketplace

### 4.1 Concept

A decentralized marketplace for AI agents, code templates, and content snippets.

### 4.2 How It Works

1. **Publish:** Player creates a listing with title, description, GitHub URL, and USDT price. Costs 50 Gold to publish.
2. **Listed:** Listing appears on the public marketplace.
3. **Purchase:** Buyer pays USDT via Polygon smart contract.
4. **Delivery:** Seller's GitHub repo becomes available to buyer. Listing marked as sold.

### 4.3 Smart Contract (Polygon)

- **Subscription.sol:** Manages player subscriptions
- **TemplateMarket.sol:** Marketplace logic (list, buy, sell, transfer)
- **USDTInterface.sol:** USDT token interaction

### 4.4 Content Types

- GitHub repositories (any content: agents, code, templates, snippets)
- Authors can customize description and price
- Listings expire after 30 days if not sold

### 4.5 Fees

- Platform fee: 5% of sale price (configurable in GameConfig)

---

## 5. Technical Architecture

### 5.1 Stack

| Layer         | Technology              | Role                           |
|---------------|------------------------|--------------------------------|
| Client        | Bevy 0.15 (Rust)       | 3D game rendering, input, voice |
| Backend       | SpacetimeDB 2.7        | Real-time multiplayer, world state, logic |
| Blockchain    | Alloy + Polygon        | Wallet auth, marketplace, USDT |
| Voice         | str0m 0.21 + datachannel | WebRTC voice chat, proximity-based |
| Language      | Rust (2021 edition)    | All crates                     |

### 5.2 SpacetimeDB

- **Self-hosted** on Raspberry Pi (or any VPS)
- **Server modules** with validated logic (not client-side entropia)
- Tables: `player`, `hex_tile`, `voice_channel`, `market_listing`
- Scheduled functions: plant updates (10s), idle gains (5min), voice cleanup (1min), listing cleanup (1hr)
- Pub/Sub events for real-time client updates

### 5.3 Bevy Client

- 3D hex grid rendering
- WASD movement (10 m/s base speed, modified by vehicle)
- Minimap (2D overlay) and global map view
- Voice chat UI (proximity indicator)
- Interaction UI (plant, harvest, clean buttons)
- Marketplace UI (browse, list, buy)
- Teleport UI (map click → confirm → pay Gold)

### 5.4 Wallet Authentication

- Player connects wallet (Polygon network)
- Signature-based login to SpacetimeDB
- No password — wallet is identity

### 5.5 Asset Strategy

- **Phase 1:** Procedural placeholders (colored primitives, no external assets)
- **Phase 2:** Import low-poly assets (see `lowpoly_assets/` pack list)
- **Phase 3:** Polish textures, animations, VFX

---

## 6. Progression Roadmap

### Phase 1: Core Loop (MVP)

- [ ] SpacetimeDB tables + server modules
- [ ] Hex grid generation + rendering (procedural)
- [ ] Player spawn + WASD movement
- [ ] Idle gains calculation (offline)
- [ ] Basic interaction (plant/harvest/clean)
- [ ] Voice chat within hex (proximity)
- [ ] Wallet login

### Phase 2: Marketplace

- [ ] Smart contract deployment (Polygon)
- [ ] Marketplace UI (list, browse, buy)
- [ ] USDT integration
- [ ] Template delivery via GitHub

### Phase 3: Content & Polish

- [ ] Vehicle system (buy, equip, speed)
- [ ] Cosmetic system (hat, aura, trail)
- [ ] Minimap + global map
- [ ] Teleport mechanic
- [ ] Low-poly asset import
- [ ] Animation + VFX
- [ ] Subscription tier system

---

## 7. Decisions

| Question | Decision |
|----------|----------|
| Concurrency | 100 concurrent players (SpacetimeDB single-node is enough) |
| Voice fallback | Voice only — no text in voice channels |
| Template moderation | None — free publishing |
| Level cap | None — infinite progression |
| Daily login rewards | Yes — MVP scope |
| Seasonal events | Backlog |
| Timeline | Exploratory — no target date |

---

## 8. Backlog (Future)

- Seasonal events (e.g. "Spring Cleanup" with double XP)
- Subscription tiers
- Text fallback in voice chat
- Template quality verification / community voting
- Public testnet deployment

---

## 9. Appendix

### 9.1 Hex Grid Math

- Axial coordinates: `(q, r, s)` where `q + r + s = 0`
- Hex center: `x = hex_radius * sqrt(3) * (q + r/2)`, `y = hex_radius * 1.5 * r`
- Hex ID: `(q << 32) | r` (stored as u64)

### 9.2 Level Formula

```rust
pub fn xp_for_next_level(level: u32) -> u64 {
    100 * (level as u64).pow(2)
}

pub fn calculate_level(total_xp: u64) -> u32 {
    let mut level = 1u32;
    let mut xp_needed = 100u64;
    let mut remaining = total_xp;
    while remaining >= xp_needed {
        remaining -= xp_needed;
        level += 1;
        xp_needed = Self::xp_for_next_level(level);
    }
    level
}
```

### 9.3 Economy Summary

| Action        | Gold Cost | Gold Reward | Net   | XP  |
|---------------|-----------|-------------|-------|-----|
| Plant         | 10        | 0           | -10   | 5   |
| Harvest       | 0         | 15          | +15   | 10  |
| Clean         | 20        | 20          | 0     | 15  |
| Publish       | 50        | 0           | -50   | 0   |
| Teleport      | 100       | 0           | -100  | 0   |

**Net per plant cycle:** +5 Gold, +15 XP (after planting + harvesting)

---

**Last updated:** 2026-07-25  
**Status:** Draft — waiting on Ferris review
