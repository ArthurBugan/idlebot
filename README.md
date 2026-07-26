# IdleBot

> Idle Tamagotchi × Voice Chat × Code Marketplace on a 3D Hex Grid

## Overview

IdleBot is a multiplayer idle game where you manage a Tamagotchi-like character that grows XP and Gold even when you're offline. The game world is a shared 3D hex grid where players meet, chat by voice, farm, clean pollution, and trade AI agents and code templates on an on-chain marketplace.

**Core loop:** Idle → Collect → Interact → Trade → Grow.

## Features

- 🎮 **Idle Progression** — XP and Gold accumulate offline (up to 24h)
- 🌍 **3D Hex Grid World** — Shared multiplayer environment with terrain types
- 🎤 **Voice Chat** — Proximity-based voice channels (within hex)
- 🌱 **Farming** — Plant, grow, and harvest resources
- 🚗 **Vehicles** — Bicycle, Scooter, Motorcycle, Boat, Airplane
- 💰 **Economy** — Gold, USDT, Eco Points
- 🛒 **Marketplace** — Trade AI agents and code templates via Polygon
- 🔐 **Wallet Auth** — Polygon signature login (no passwords)

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Client | Bevy 0.15 (Rust) |
| Backend | SpacetimeDB 2.7 |
| Blockchain | Alloy + Polygon |
| Voice | str0m 0.21 |
| Language | Rust 2021 |

## Project Structure

```
idlebot/
├── crates/
│   ├── idlebot-core/          # Identity, XP, gold, level calc
│   ├── idlebot-client/        # Bevy 3D rendering, input, voice
│   ├── idlebot-server/        # SpacetimeDB server modules
│   └── idlebot-chain/         # Polygon wallet auth, marketplace
├── contracts/                 # Solidity smart contracts
├── assets/                    # Low-poly assets (Phase 2+)
├── scripts/                   # Build and utility scripts
└── specs/                     # SDD specifications
```

## Development

### Prerequisites

- Rust 1.70+ (2021 edition)
- SpacetimeDB CLI
- Polygon testnet wallet

### Build

```bash
cd crates/idlebot-core
cargo build

cd crates/idlebot-client
cargo build

cd crates/idlebot-server
cargo build
```

### Run

```bash
# Start SpacetimeDB
spacetime start

# Run server module
cd crates/idlebot-server
cargo run

# Run client
cd crates/idlebot-client
cargo run
```

## Documentation

- **[PROPOSAL.md](PROPOSAL.md)** — Complete design specification
- **[KANBAN.md](KANBAN.md)** — Task tracking and progress
- **[specs/constitution.md](specs/constitution.md)** — Development principles

## Roadmap

### Phase 1: Core Loop (MVP) ✅ In Progress

- [x] Project structure
- [ ] SpacetimeDB tables + server modules
- [ ] Hex grid generation + rendering
- [ ] Player spawn + WASD movement
- [ ] Idle gains calculation
- [ ] Basic interactions (plant/harvest/clean)
- [ ] Voice chat
- [ ] Wallet login

### Phase 2: Marketplace

- [ ] Smart contract deployment
- [ ] Marketplace UI
- [ ] USDT integration

### Phase 3: Content & Polish

- [ ] Vehicle system
- [ ] Cosmetics
- [ ] Minimap + global map
- [ ] Teleport
- [ ] Asset polish

## License

MIT

---

**Status:** MVP Development  
**Last Updated:** 2026-07-25
