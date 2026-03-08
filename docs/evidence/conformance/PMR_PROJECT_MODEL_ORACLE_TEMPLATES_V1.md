# PMR Project-Model Oracle Templates v1

Date: 2026-03-03
Scope: `CCT-037..CCT-041`
Status: template + executable runner + initial capture complete

## Purpose

Provide concrete Excel/VBA probe templates for Project/Module/Reference behaviors that are implemented in deterministic subset form in OxVba but still require host-oracle foldback before parity claims.

## Topics

| Topic | Probe Focus | Workbook Shape | Expected Capture |
|---|---|---|---|
| `CCT-037` | Reference precedence and shadowing | 3 projects (`SourceA`, `LibFirst`, `LibSecond`) with conflicting `Public Function Compute()` names and reordered references | Winner project/module per reference order; diagnostics for ambiguous/non-visible paths |
| `CCT-038` | `Option Private Module` cross-project visibility | 2 projects with paired modules (`PublicMod`, `PrivateMod`) toggling `Option Private Module` | Accessibility matrix (`call ok` vs compile/runtime diagnostic) from referencing project |
| `CCT-039` | Header attribute defaults + legality | Class and procedural modules varying `VB_Name`, `VB_PredeclaredId`, `VB_GlobalNamespace`, `VB_Creatable`, `VB_Exposed` | Attribute default table and legality outcomes by project/module kind |
| `CCT-040` | `Implements` coverage + prefix rules | Class module implementing one and multiple interfaces, with complete/incomplete members | Compile-time diagnostics, required prefix form, member coverage obligations |
| `CCT-041` | `WithEvents` legality + handler ordering | Class/procedural module matrix with re-assignment and `RaiseEvent` paths | Legality diagnostics + event ordering traces under reassignment |

## Template Harness Layout

For each topic, use:

1. `Workbook` containing project set (`.xlsm` with separate VBProjects or equivalent manual setup).
2. `Driver` module exposing deterministic runner `Sub Probe_<topic>()` that writes outcomes to worksheet cells.
3. `Capture` worksheet with columns:
   - `case_id`
   - `project_order`
   - `module_flags`
   - `expected_vba_observed`
   - `oxvba_observed`
   - `match`
   - `notes`
4. Exported module text snapshot under `docs/evidence/conformance/oracle_captures/<topic>/`.

## Foldback Contract

After each oracle run:

1. Attach captured workbook/module artifacts under `docs/evidence/conformance/oracle_captures/<topic>/`.
2. Update `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` evidence column for matching `ODG-0xx`.
3. Reconcile `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv` and `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv` statuses.
4. Record residual mismatches in `docs/evidence/conformance/IMPLEMENTATION_DEFINED.md` if behavior is implementation-defined.

Automated runner:

- `scripts/run-pmr-project-model-oracle.ps1`
- `scripts/excel-dialog-guardian.ps1` (spawned by default by the PMR runner to auto-handle macro/add-in security dialogs in unattended runs)

## Current Local Coverage Anchors

- `crates/oxvba-compiler/src/project.rs` (`compile_project`, header/visibility diagnostics, qualification rewrites)
- `crates/oxvba-host/src/project.rs` (`ProjectGraph` invariants, reference-order symbol resolution, host export eligibility)
- `crates/oxvba-host/src/engine.rs` (`formal_pmr_project_manifest_*` execution fixtures)

This template remains non-blocking by policy until oracle captures are completed.

Initial capture status:

- Completed run: `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T070427Z/summary.md`
- Gate outcomes:
  - `CCT-037..CCT-039` matched.
  - `CCT-040` original divergence is closed locally for baseline shape; refreshed edge-matrix oracle foldback remains open (`ODG-038`).
  - `CCT-041` divergence remains open (`DIV-0004`) for true instance-level reassignment/subscription semantics.
