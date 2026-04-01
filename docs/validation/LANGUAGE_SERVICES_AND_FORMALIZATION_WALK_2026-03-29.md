# Language Services / Formalization Validation Walk - 2026-04-01

Status: `complete`
Scope: beads `bd-gm3.12.4`, `bd-ls1.6.1`
Canonical matrix: `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`

## Purpose

Record the validation-row split that promotes the language-service tranche-A surface from one bounded internal inventory row into explicit canonical rows while preserving the formalization row separately.

## Verified Rows

| Feature ID | Verified subset | Evidence checked | Result |
|---|---|---|---|
| `LSF-0001` | core syntax/service substrate only: syntax tree, semantic snapshot, diagnostics, core symbol/definition/reference queries | `crates/oxvba-syntax/src/{lexer.rs,parser.rs,red.rs}`, `crates/oxvba-languageservice/src/{semantic.rs,service.rs,workspace.rs}` | supported as `in-progress` with bounded evidence |
| `LSF-0100` | project-aware workspace loading, provenance/identity, invalidation, and interactive harness evidence | `crates/oxvba-languageservice/src/{workspace.rs,service.rs,span.rs}`, `cargo test -p oxvba-languageservice -- --nocapture` | supported as `implemented-subset` |
| `LSF-0101` | document symbols, workspace symbols, semantic classification | `crates/oxvba-languageservice/src/service.rs`, `cargo test -p oxvba-languageservice -- --nocapture` | supported as `implemented-subset` |
| `LSF-0102` | richer completion, signature-help, and hover context | `crates/oxvba-languageservice/src/service.rs`, `cargo test -p oxvba-languageservice -- --nocapture` | supported as `implemented-subset` |
| `LSF-0103` | rename preparation and safe reference-update analysis | `crates/oxvba-languageservice/src/service.rs`, `cargo test -p oxvba-languageservice -- --nocapture` | supported as `implemented-subset` |
| `LSF-0104` | diagnostics-driven code-action foundation | `crates/oxvba-languageservice/src/service.rs`, `cargo test -p oxvba-languageservice -- --nocapture` | supported as `implemented-subset` with bounded quick-fix families |
| `LSF-0105` | thin transport / embedding boundary | spec + tracker only | remains `in-progress` / planned boundary |
| `LSF-0002` | scaffolded formal representation plus obligation registry and deferred formal cadence | `formal/lean/OxVba/{VarType,Coerce,Arithmetic,RefCount}.lean`, `docs/evidence/formal/{MANIFEST.md,INVENTORY.md,obligations.csv,latest_run.md}` | supported as `in-progress` with bounded evidence |

## Matrix Boundary

The canonical matrix is now intentionally split:
- `LSF-0001` records the bounded service substrate, not the full tranche-A inventory.
- `LSF-0100` through `LSF-0104` record the implemented first-class tranche-A surfaces now present in the direct Rust API.
- `LSF-0105` records the still-open thin-transport / embedding boundary.
- `LSF-0002` continues to record formalization scaffolding and registry coverage, not proof closure.

## Bounded Outcome

The checked evidence supports the row split and the current `implemented-subset` / `in-progress` truth claims.
The language-service matrix no longer collapses symbols, semantic classification, richer editor queries, rename-preparation, and diagnostics-driven code-action planning into one generic internal row.
