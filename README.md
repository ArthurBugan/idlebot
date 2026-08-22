# IdleBot

> Idle Tamagotchi × Voice Chat × Code Marketplace on a Hex Grid

## Overview

IdleBot is a multiplayer idle game where you manage a Tamagotchi-like character that grows XP and Gold even when you're offline. The game world is a shared hex grid (rendered as an isometric 2D world) where players meet, chat by voice, farm, clean pollution, and trade AI agents and code templates on an on-chain marketplace.

**Core loop:** Idle → Collect → Interact → Trade → Grow.

## Features

- 🎮 **Idle Progression** — XP and Gold accumulate offline (up to 24h, tiered brackets)
- 🌍 **Hex Grid World** — Shared multiplayer isometric world with terrain types (seeded, deterministic)
- 🎤 **Voice Chat** — Proximity-based voice channels (within hex) — server-side channel management; WebRTC audio layer (str0m) still pending
- 🌱 **Farming** — Plant, grow, and harvest resources
- 🚗 **Vehicles** — Bicycle, Scooter, Motorcycle, Boat, Airplane
- 💰 **Economy** — Gold, USDT, Eco Points (server-enforced ledger with sinks)
- 🛒 **Marketplace** — Trade AI agents and code templates with USDT escrow (Solidity/Polygon + Solana/Anchor contracts)
- 🔐 **Wallet Auth** — Wallet-signature login (no passwords); identity binding + rate limits enforced server-side

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Client | Bevy 0.19 (Rust), 2D isometric renderer |
| Backend | SpacetimeDB 2.7 (server) + 2.8 SDK (client) |
| Blockchain | Alloy 2.2 + Polygon (Solidity), Anchor (Solana) |
| Voice | Server-side hex voice channels (str0m 0.21 planned) |
| Language | Rust 2021 |

## Project Structure

```
idlebot/
├── crates/
│   ├── idlecore-core/          # Shared types: hex math, player, XP, economy, protocol, wallet auth
│   ├── idlecore-client/        # Bevy 2D isometric client: world, player, minimap, HUD, net, skins
│   ├── idlecore-server/        # SpacetimeDB server modules (authoritative game state)
│   └── idlecore-chain/         # Polygon wallet auth + transactions (Alloy)
├── contracts/
│   ├── solidity/               # Polygon: TemplateMarket.sol, Subscription.sol, USDTInterface.sol
│   ├── solana/                 # Anchor port: marketplace + subscriptions (SPL token USDT)
│   └── token/                  # USDT token interface helpers
├── assets/                     # (reserved for shared assets)
└── specs/                      # SDD specifications (001–021) + constitution
```

Client assets (isometric tile packs, 44 player skin sprites, fonts) live in `crates/idlecore-client/assets/`.

## Development

### Prerequisites

- Rust stable (2021 edition)
- SpacetimeDB CLI 2.x
- Polygon testnet wallet (chain integration) / Solana toolchain (Anchor contracts)

### Build

```bash
cargo build -p idlecore-core
cargo build -p idlecore-client
cargo build -p idlecore-server
```

### Run

```bash
# Start SpacetimeDB
spacetime start

# Run server module
cd crates/idlecore-server
cargo run

# Run client
cd crates/idlecore-client
cargo run
```

### Headless e2e smoke

```bash
cargo run -p idlecore-client --bin e2e
```

Logins, verifies row replication, and exercises teleport against a running module.

## Documentation

- **[PROPOSAL.md](PROPOSAL.md)** — Complete design specification
- **[specs/README.md](specs/README.md)** — Spec-driven development rules
- **[specs/constitution.md](specs/constitution.md)** — Development principles

## Roadmap

### Phase 1: Core Loop (MVP) — mostly done

- [x] Project structure
- [x] SpacetimeDB tables (11 core + 5 scheduler) + server modules
- [x] Hex grid generation + isometric rendering
- [x] Player spawn + WASD movement
- [x] Idle gains calculation (server scheduler, 5 min tick)
- [x] Basic interactions (plant/harvest/clean)
- [ ] Voice chat audio (channel management done; WebRTC/str0m pending)
- [x] Wallet login (identity binding + rate limits)

### Phase 2: Marketplace — in progress

- [x] Marketplace server module (list, buy, escrow, disputes, renewals)
- [x] Smart contracts authored (Solidity + Solana/Anchor)
- [ ] Contract deployment
- [ ] Marketplace UI polish
- [ ] USDT integration end-to-end

### Phase 3: Content & Polish

- [x] Vehicle system (buy, equip, speed)
- [x] Cosmetics (hat + aura layers)
- [x] Minimap
- [x] Teleport (with VFX + latency HUD)
- [ ] Asset polish (beyond isometric packs + skin sprites)

## License

MIT

---

**Status:** MVP Development
**Last Updated:** 2026-08-17
