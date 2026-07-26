# IdleBot — Constitution

> **Princípios inegociáveis para o desenvolvimento do IdleBot.**
> Leia este arquivo antes de qualquer alteração.

## Regras Fundamentais

1. **Não há código sem spec** — Trabalho não-trivial (≥20 linhas ou mudança em contrato público) exige spec aprovado
2. **Testes não são opcionais** — ≥1 assert por requisito novo comportamento
3. **Secrets nunca no código** — `.env` (gitignored) local, secret manager em produção
4. **Falhe alto, não silenciosamente** — Use `anyhow`, `thiserror`, nunca `unwrap()` em código de produção
5. **Contratos públicos são imutáveis** — Sem major version bump + CHANGELOG
6. **Não pule fases** — spec → plan → tasks → implement → review

## Regras do IdleBot

7. **Econômia balanceada** — Net per plant cycle: +5 Gold, +15 XP (verificar após cada mudança)
8. **Offline gains seguros** — SpacetimeDB scheduled function, 5min interval, max 24h
9. **Proximidade de voz** — 50m radius, within-hex, non-positional audio
10. **Wallet como identidade** — Polygon signature auth, no password

## Stack Imutável (MVP)

- **Client:** Bevy 0.15 (Rust)
- **Backend:** SpacetimeDB 2.7 (self-hosted)
- **Blockchain:** Alloy + Polygon
- **Voice:** str0m 0.21 + datachannel
- **Language:** Rust 2021 edition

## Fluxo SDD

```
[Human] → SPEC (WHAT) → PLAN (HOW) → TASKS (WHEN) → CODE (DO) → REVIEW → MERGE
```

## Agentes

| Agente | Responsabilidade |
|--------|------------------|
| `leader` | Orquestrar fluxo, revisar specs |
| `spec_author` | Escrever specs de features |
| `architect` | Planos técnicos, decisões |
| `implementer` | Código de tarefas específicas |
| `reviewer` | Validar que tarefas estão completas |

## Onde Olhar

| Preciso de... | Arquivo |
|---|---|
| Escopo completo | `PROPOSAL.md` |
| Progresso | `KANBAN.md` |
| Status do projeto | `feature_list.json` |
| Decisões arquiteturais | `specs/decisions/` |
| Exemplo de spec | `specs/001-example/` (quando criado) |

## O Que NÃO Fazer

- Não invente APIs sem spec aprovado
- Não amplie escopo silenciosamente — atualize o spec
- Não pule linting/tests com flags de bypass
- Não escreva propriedades da struct como `#[serde(skip_serializing)]` sem razão clara
- Não use `unwrap()` em código de produção — use `?` ou `expect()` com mensagem clara

---

**Última atualização:** 2026-07-25  
**Status:** Vigente para MVP
