# MS-VBAL Module/Project Requirements

## Purpose

Track and prioritize full Project/Module/Reference coverage for the MS-VBAL scope,
with explicit cross-reference to MS-OAUT (automation/reference binding) and
MS-OVBA (project/module storage).

Machine-readable source:
- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`

Formal PMR spec set:
- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`
- `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`

Canonical source map:
- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/REFERENCE_SPEC_FORMAT_AND_CONFORMANCE.md`

Primary extracted source runs:
- MS-VBAL: `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`
- MS-OAUT: `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`
- MS-OVBA: `../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/`

## Current Language Status vs Spec

Current OxVba coverage artifacts indicate:

- Language coverage rows: `85 implemented`, `1 partial`, `1 planned`
  (`docs/evidence/language/COVERAGE_INDEX.csv`)
- Runtime/library rows: `14 implemented`, `3 partial`, `2 planned`
  (`docs/evidence/runtime/LIBRARY_CHECKLIST.csv`)
- Remaining non-implemented rows in those files are primarily HAL/interop/file-IO/UI.

Conclusion:
- Statement/expression language core is broadly implemented for the current executable subset.
- Full MS-VBAL scope is not complete yet because project/module/reference semantics are only partially executable and still rely on deferred-oracle foldback for parity claims.
- Project/model support now includes executable `ProjectManifest` compilation and `ProjectGraph` reference-order resolution in deterministic subset form, with stable PMR diagnostics.

## Requirement Set

Tracked requirement classes in this checklist:

- Project model and multi-module compilation.
- Module kind model (procedural/class/document/form/extension).
- Module attributes and visibility rules.
- Cross-module/project-qualified name resolution.
- Class instancing/default-instance and Implements/WithEvents constraints.
- Project references, precedence, and automation dispatch boundary constraints.
- Host-facing exported procedural entrypoints (for example Excel-UDF-style discovery/invocation of `Public` module procedures).
- Project storage/roundtrip obligations (MS-OVBA).

## Deferred Scope

Allowed deferrals in this phase:

- Forms runtime/userform behavior (`forms-deferred`).
- HAL-adjacent host binding and external automation edges (`hal-adjacent`).

These remain required scope, but not blockers for initial project-model bring-up.

## Source Quality Note

The current MS-OVBA extracted run (`20260301-ms-ovba-pass01`) contains only the
landing-page level items and does not yet provide section-level conformance
anchors for normative section 2 obligations. This gap is explicitly tracked in
the requirements CSV and PMR clause catalog as a source-extraction blocker for
storage parity claims.

## Recommended Next Focus

Next work should prioritize deeper ProjectGraph binder/runtime integration and class-event/interface semantics
before expanding peripheral language surface.

Reason:
- Core language surface is mostly closed for the current subset.
- Project/module/reference semantics are structural prerequisites for full MS-VBAL fidelity and now have an executable baseline to extend.
- Delaying project-model work increases retrofit cost across parser/binder/runtime/host/HAL boundaries.

## A1-A5 Execution Note (2026-03-03)

Completed in this pass:

- A1: formal class semantic contract tightened and mapped to PMR clauses.
- A2: executable non-interop subset expanded with class lifecycle/property evidence and stable PMR diagnostics for deferred class/project features.
- A3: COM-boundary split documented as semantic-vs-adapter responsibility.
- A4: host-side ProjectGraph scaffold implemented with deterministic pre/postcondition validation.
- A5: PMR tiered claim statuses updated and executable status-vocabulary check added.

## P0-P10 Workset Execution Note (2026-03-03)

ProjectGraph parser+binder integration master workset (`P0..P10`) is now completed in deterministic subset form, including:

- compiler-facing `ProjectManifest` + `compile_project(...)`,
- header retention/validation diagnostics (`PMR-E-MODULE-HEADER-*`),
- qualification rewrite subset (module-qualified and same-project-qualified),
- host export eligibility registry (`MODPROJ-039` partial),
- reference-order project graph symbol resolution subset,
- deferred-oracle template setup for `CCT-037..CCT-041`.

Evidence rollup:
- `docs/evidence/language/PMR_PROJECTGRAPH_P0_P10_ROLLUP_2026-03-03.md`
