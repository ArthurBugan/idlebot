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

**Anti-Cheat Protection:**
- Server validates elapsed time using server clock only (client timestamps ignored)
- Network latency compensated with ±2 second tolerance window
- Repeated rapid logins (within 5 min of last logout) trigger 90-day "new player" state (no idle gains)
- Idle gains calculated server-side via SpacetimeDB scheduled function (every 5 minutes)
- Maximum idle duration: 24 hours (no additional gain for longer periods)
- Decay function: `150 - (elapsed_hours - 12) * 7.5` for hours 12-24 (nonlinear scaling)

**Economy Note:** Idle gains are capped at 150 XP/75 Gold per 24h. This creates a soft cap on passive income and requires active gameplay for exponential growth.

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

**Conflict Resolution:**
- **Server-side locking:** All state-changing actions (planting, harvesting, cleaning) require server approval before execution
- **Action queue:** When multiple players attempt actions on the same hex simultaneously, actions queued by server timestamp
- **Lock timeout:** 2-second window for action resolution. If lock not acquired within 2s, action rejected with "Hex busy" message
- **Concurrent player limit:** Maximum 8 players per hex simultaneously. When exceeded, newest arrival gets queued notification
- **Plant ownership:** Each hex can only have ONE player's plant at a time. Harvesting yields go to the planter. Non-planter harvesters receive 0 Gold + 0 XP (no reward sharing)
- **Collision handling:** Two players harvesting the same plant? First to lock wins. Second player sees "Already harvested" error with 3-second cooldown

**Permissions & Access:**
- Players can interact with any hex except: another player's active plant, their own hex (passive income only), or hexes locked by server maintenance
- View-only: All hexes show public info (terrain, plant status, player count) regardless of ownership

### 3.4 Voice Chat

- **Radius:** 50 meters (one hex radius)
- **Type:** Within-hex channel — all players in the same hex hear each other
- **Audio:** Non-positional (no attenuation within hex, like a room)
- **Mechanics:** Voice channel auto-created when player enters a hex with others
- **Cleanup:** Channels destroyed when all players leave (5 min timeout)

**Technical Architecture:**
- **ICE/STUN/TURN:** Full ICE candidate gathering with STUN servers for NAT traversal
- **TURN relay required:** All players MUST connect through TURN relay servers for reliable connectivity (UDP blocking common in corporate/school networks)
- **TURN servers:** 3 regional TURN servers (EU, US-East, APAC) — hosted on Hetzner/Cloudflare
- **Adaptive bitrate:** 16kHz mono codec (Opus) at 32kbps adaptive based on network conditions
- **Packet loss handling:** 20ms packet loss tolerance with automatic bitrate reduction
- **Codec:** Opus v1.3.1 with 48kHz sample rate (best balance of quality vs bandwidth)
- **Network requirements:** Minimum 500kbps upload/download for 1:1 voice; 1mbps for 3+ players in same channel
- **Connection persistence:** 15-second reconnect window after packet loss. Auto-rejoin voice channel on reconnect without explicit user action
- **Echo cancellation:** WebRTC built-in AEC with server-side noise suppression

**Channel Limits:**
- Maximum 8 players per voice channel (prevents audio chaos)
- When exceeding 8 players, oldest 8 remain; newest gets priority queue notification
- Priority queue scrolls through waiting players every 30 seconds

**Emergency features:**
- Voice input mute button (always accessible)
- Emergency party invite (waits in queue, bypasses limits)
- Discord fallback channel for large groups (50+ players)

### 3.5 Economy

- **Gold:** Earned via idle gains + harvesting. Spent on planting, cleaning, publishing templates, vehicles, cosmetics, teleport.
- **USDT:** Premium currency. Used for marketplace template purchases via smart contract.
- **Eco Points:** Earned by cleaning pollution and planting trees. Affects eco rating of hexes.

**Economy Sinks (Inflation Control):**
- **Vehicle Maintenance:** 5 Gold/hour while owned (applied every 24h at midnight server time)
- **Template Listing Renewal:** 10 Gold every 7 days to keep a marketplace listing active
- **Teleport Cost Increase:** 100G base, scales by level: `100 * level^0.5` (diminishing returns on wealth)
- **Idle Gain Decay:** After 7 consecutive days without spending, gain multiplier reduces by 10%
- **Pollution Spread:** Neglected hexes (no player interaction for 48h) revert to Polluted state, requiring re-cleaning costs

**Economy Flow:**
```
Faucet (Sources):
├── Idle gains (max 75G/24h)
├── Harvesting (+15G per cycle, 5min cooldown)
├── Cleaning (+20G per hex)
└── Marketplace sales (variable, seller price)

Sinks (Removal):
├── Vehicle maintenance (5G/h)
├── Listing renewals (10G/7d)
├── Teleport costs (100-500G+ depending on level)
├── Planting costs (10G per plant)
└── Cosmetic purchases (50-500G)
```

**Inflation Target:** < 5% annual growth rate. Economy audited quarterly with sink adjustments.

---

## 4. Marketplace

### 4.1 Concept

A decentralized marketplace for AI agents, code templates, and content snippets.

### 4.2 How It Works

1. **Publish:** Player creates a listing with title, description, GitHub URL, and USDT price. Costs 50 Gold to publish.
2. **Listed:** Listing appears on the public marketplace.
3. **Purchase:** Buyer pays USDT via Polygon smart contract. Funds held in **escrow** (not released to seller immediately).
4. **Escrow Period:** 48-hour dispute window begins. Buyer has time to review the delivered content.
5. **Delivery:** Seller's GitHub repo becomes available to buyer. Listing marked as "pending delivery".
6. **Confirmation or Dispute:**
   - **No disputes:** After 48 hours, funds release to seller minus 5% platform fee. Listing marked as sold.
   - **Dispute filed:** Buyer reports issue (malicious code, misleading description, non-functional repo). Funds frozen in escrow.
7. **Dispute Resolution:**
   - **Automatic resolution:** If seller does not respond within 24 hours of dispute notification, buyer wins — funds returned minus 2% penalty fee.
   - **Manual resolution:** If both parties respond, community voting or admin intervention required. 3-day resolution window.
8. **Resolution Outcomes:**
   - **Buyer wins:** Full refund minus 2% platform fee. Listing removed.
   - **Seller wins:** Funds released minus 5% platform fee. Buyer banned from purchases for 7 days.
   - **Both penalized:** If both parties file counter-claims, both receive 50% refund minus 3% fee each.

**Dispute Triggers:**
- Malicious code detected (automated scan)
- Description mismatch with actual content
- Repository becomes private after sale
- Repository is empty or doesn't exist
- Seller fails to respond to dispute

**Automated Protection:**
- GitHub repo scanned for known malware signatures before delivery
- Warning system for repeat offenders (3 strikes → marketplace ban)
- Insurance pool: 2% of all marketplace transactions fund a dispute resolution pool

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
| Client        | Bevy 0.19 (Rust)       | 3D game rendering, input, voice |
| Backend       | SpacetimeDB 2.0        | Real-time multiplayer, world state, logic |
| Blockchain    | Alloy 2.2.0 + Polygon  | Wallet auth, marketplace, USDT |
| Voice         | str0m 0.21.0 + datachannel | WebRTC voice chat, proximity-based |
| Language      | Rust (2024 edition)    | All crates                     |

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

**Wallet Recovery:**
- **Seed phrase backup:** 24-word BIP39 seed phrase generated on wallet creation (exported to secure storage)
- **Social recovery:** 3-of-5 guardian system — players select 3 trusted friends as recovery guardians. Each guardian can sign a recovery transaction on behalf of lost wallet
- **Recovery timeline:** 48-hour waiting period after initiating social recovery (prevents quick exploits)
- **Loss scenarios handled:**
  - Lost private key → social recovery with guardians
  - Lost seed phrase → same as lost private key
  - Compromised wallet → immediate key rotation via guardians, funds transferred to new wallet
  - Hacked wallet → emergency freeze via admin panel (24h window), then key rotation

**Security measures:**
- Wallet connection signed with timestamp + nonce (prevents replay attacks)
- Session tokens expire after 24 hours of inactivity
- Multi-signature for large transactions (>1000 USDT requires 2/3 guardian approval)
- Transaction limits: 10,000 USDT per day without additional verification

**Wallet provider support:**
- MetaMask (primary)
- WalletConnect (mobile)
- Rabby (alternative desktop)
- Hardware wallets (Ledger/Trezor) via WalletConnect

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

| Last updated: 2026-07-26 |
| Status: Draft — waiting on Ferris review |
| **Software versions verified:** Bevy 0.19, SpacetimeDB 2.0, str0m 0.21.0, Alloy 2.2.0 |
