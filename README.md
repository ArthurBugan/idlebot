# IdleBot — SDD Template

> Idle Tamagotchi × Voice Chat × Code Marketplace on a 3D Hex Grid

## Quick Start

```bash
cd /home/raspberry/idlebot
make spec NAME=001-your-feature
```

## Estrutura

```
idlebot/
├── crates/                   # Rust crates (Bevy client, SpacetimeDB server, etc.)
├── contracts/                # Solidity smart contracts (Polygon)
├── assets/                   # Low-poly assets (Phase 2+)
├── scripts/                  # Build and utility scripts
├── specs/                    # SDD specifications
│   ├── constitution.md
│   ├── README.md
│   └── 001-<slug>/
├── docs/decisions/           # ADRs
├── PROPOSAL.md               # Design specification completa
├── KANBAN.md                 # Progresso e tarefas
├── Cargo.toml                # Workspace root
├── Makefile
├── .gitignore
└── LICENSE
```

## Philosophy

> Specs são documentos vivos, refinados conforme você aprende.
> Força você a pensar antes de digitar. Dá ao future-you o *why* que o código nunca carrega.

## Stack

| Layer | Technology | Role |
|-------|-----------|------|
| Client | Bevy 0.15 (Rust) | 3D game rendering, input, voice |
| Backend | SpacetimeDB 2.7 | Real-time multiplayer, world state |
| Blockchain | Alloy + Polygon | Wallet auth, marketplace |
| Voice | str0m 0.21 | WebRTC voice chat |

## Workflow

```
[Human] → SPEC (WHAT) → PLAN (HOW) → TASKS (WHEN) → CODE (DO) → REVIEW → MERGE
```

## Regras Fundamentais

1. **Não há código sem spec** para trabalho não-trivial (≥20 linhas ou mudança em contrato público)
2. **Testes não são opcionais** para novo comportamento — ≥1 assert por requisito
3. **Secrets nunca no código** — `.env` (gitignored) local, secret manager em produção
4. **Falhe alto, não silenciosamente**
5. **Contratos públicos são imutáveis** sem major version bump + CHANGELOG
6. **Não pule fases** — spec → plan → tasks → implement → review

## Compatibilidade

Claude Code · Cursor · GitHub Copilot · Gemini CLI · Amazon Kiro

## Licença

MIT
