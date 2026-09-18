# Architecture Decision Records (ADRs)

This directory captures significant architectural decisions for credd. Each ADR follows the format in `template.md`.

## Index

| ADR | Title | Status | Date | Supersedes |
|-----|-------|--------|------|------------|
| 0001 | Approval Protocol Wire Format | Proposed | — | — |
| 0002 | Derivative Credential Trait | Proposed | — | — |
| 0003 | SPIFFE Integration Strategy | Proposed | — | — |
| 0004 | Policy Language & Evaluation | Proposed | — | — |
| 0005 | Seal Backend Abstraction | Proposed | — | — |

## Process

1. Copy `template.md` to `NNNN-short-title.md`
2. Fill in all sections
3. Submit as PR for review
4. On merge, update this index and set status to `Accepted`
5. If superseded, mark old ADR `Superseded` and link from new ADR

## Scope

ADRs are required for decisions that:
- Affect multiple crates or cross-cutting concerns
- Define wire formats, protocols, or data schemas
- Choose external dependencies with architectural impact
- Establish security boundaries or trust models
- Define operational procedures (bootstrapping, rotation, disaster recovery)

Decisions local to a single crate or purely implementation details belong in code comments with PRD/DESIGN back-links, not ADRs.