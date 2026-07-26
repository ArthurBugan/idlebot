# IdleBot — Spec-Driven Development

> **Especificações exequíveis, nunca código antes de spec.**

## Estrutura

```
specs/
├── constitution.md          ← Princípios inegociáveis (leia primeiro!)
├── README.md                ← Este arquivo
└── <NNN>-<slug>/           ← Um diretório por feature/spec
    ├── spec.md              ← O QUÊ e POR QUÊ
    ├── plan.md              ← O COMO (arquitetura)
    ├── tasks.md             ← Checklist de implementação
    └── review.md            ← Relatório de review (após impl)
```

## Numeração

`001-<slug>`, `002-<slug>`, ... **NUNCA** reutilizar números.

## Regras

- **Não há código sem spec** (para trabalho não-trivial)
- **Specs são documentos vivos** — atualize quando entendimento muda
- **Reference specs em commits e PRs**: `Refs: specs/NNN-slug/`
- **Não amplie escopo silenciosamente** — atualize o spec ou divida

## Exemplo

Veja `001-example/` para um exemplo completo (quando criado).
