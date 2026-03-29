# Language Services / Formalization Validation Walk - 2026-03-29

Status: `complete`
Scope: bead `bd-gm3.12.4`
Canonical matrix: `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`

## Purpose

Record the bounded verification pass for the language-services/formalization matrix rows without widening the canonical claims.

## Verified Rows

| Feature ID | Verified subset | Evidence checked | Result |
|---|---|---|---|
| `LSF-0001` | internal syntax/service surface only: syntax tree, semantic snapshot, workspace, provider trait wiring | `crates/oxvba-syntax/src/{lexer.rs,parser.rs,red.rs}`, `crates/oxvba-languageservice/src/{semantic.rs,service.rs,workspace.rs}` | supported as `in-progress` with bounded evidence |
| `LSF-0002` | scaffolded formal representation plus obligation registry and deferred formal cadence | `formal/lean/OxVba/{VarType,Coerce,Arithmetic,RefCount}.lean`, `docs/evidence/formal/{MANIFEST.md,INVENTORY.md,obligations.csv,latest_run.md}` | supported as `in-progress` with bounded evidence |

## Matrix Boundary

The canonical matrix remains intentionally narrow:
- `LSF-0001` records the service-surface inventory, not full LSP parity.
- `LSF-0002` records formalization scaffolding and registry coverage, not proof closure.

## Bounded Outcome

The checked evidence supports the current `in-progress` claims for `LSF-0001` and `LSF-0002`.
No additional row split was required for this bead. The broader executable language semantics remain in `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv`, while formal proof completion remains governed by the formal evidence registry and deferred lanes.
