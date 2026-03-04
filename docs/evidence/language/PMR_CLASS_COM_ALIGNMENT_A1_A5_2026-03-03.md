# PMR Class/COM Alignment A1-A5 (2026-03-03)

## Scope

Execution pass for `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md` steps A1-A5.

## A1: Lock semantic object model contracts

Landed:

- PMR class semantic contract text in `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md` (lifecycle/property contract and deterministic PMR diagnostics).
- Host-side PMR model scaffold in `crates/oxvba-host/src/project.rs`:
  - `ProjectGraph`, `ProjectNode`, `ModuleNode`, `ProjectReference`,
  - precondition validation and stable PMR error codes (`ProjectModelError`).

## A2: Add executable non-interop conformance lane

Landed:

- Compiler diagnostics tests for deferred class/project semantics:
  - `compile_withevents_declaration_surfaces_project_model_diagnostic`
  - `compile_implements_directive_surfaces_project_model_diagnostic`
  - `compile_raiseevent_statement_surfaces_project_model_diagnostic`
- Resolver-level PMR diagnostics:
  - `PMR-E-WITHEVENTS-MODULE-KIND-UNRESOLVED`
  - `PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED`
  - `PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED`
- Conformance fixtures:
  - `conformance/tests/project_model_withevents_requires_class_graph.bas`
  - `conformance/tests/project_model_implements_requires_class_graph.bas`
  - `conformance/tests/project_model_raiseevent_requires_class_graph.bas`

## A3: Define COM-boundary contract for class semantics

Landed:

- PMR semantic-vs-adapter boundary section in `PROJECT_MODULE_REFERENCE_SPEC_V1.md`.
- HAL integration cross-reference updates in `PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`.
- Clause coverage expansion with `PMR-COM-001` and `PMR-COM-002`.

## A4: Integrate class model into Project/Module graph design

Landed:

- Project graph model now carries:
  - ordered module and reference sets,
  - class attribute metadata,
  - default-instance derivation hook,
  - symbol-collision qualification resolution scaffold.

## A5: Gate claims with compatibility tiers

Landed:

- PMR clause statuses updated across `implemented-verified` / `implemented-partial` / `specified-pending`.
- Executable guard:
  - `formal_pmr_a5_claim_tiers_have_stable_status_values` validates status vocabulary and rejects planned-only verification anchors on `implemented-verified` rows.

## Residual open work (expected)

- Full `WithEvents`/`Implements`/`RaiseEvent` runtime semantics remain open.
- Project binder/runtime integration for multi-module and multi-project resolution remains open.
- Full COM ABI parity remains deferred to HAL/interop lanes.
