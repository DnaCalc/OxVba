# Formal Obligation Manifest

This manifest tracks profile-scoped formal obligations.

## Status Legend
- `pass`: obligation currently passing.
- `todo`: obligation currently failing or not executable; non-blocking at this stage.
- `skipped`: tooling unavailable or intentionally skipped.

## Source Of Truth
- Obligation index: `docs/evidence/formal/obligations.csv`
- Latest run report: `docs/evidence/formal/latest_run.md`
- Latest run csv: `docs/evidence/formal/latest_run.csv`

The obligation index is intentionally machine-readable and now the canonical registry for all profile obligations (`v2` through `v46`).

## Policy (current ladder run)
- Formal runs are required in-cycle for relevant changes.
- Formal failures are non-blocking during current ladder stage.
- Moderate in-cycle fix effort is expected; unresolved items move to the extended todo list.
