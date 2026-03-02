# MS-VBAL Module/Project Requirements

## Purpose

Track the remaining MS-VBAL coverage that is centered on module/project semantics
rather than expression/statement runtime semantics.

Machine-readable source:
- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`

Normative source map:
- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/REFERENCE_SPEC_FORMAT_AND_CONFORMANCE.md`

## Current Language Status vs Spec

Current OxVba coverage artifacts indicate:

- Language coverage rows: `85 implemented`, `1 partial`, `1 planned`
  (`docs/evidence/language/COVERAGE_INDEX.csv`)
- Runtime/library rows: `14 implemented`, `3 partial`, `2 planned`
  (`docs/evidence/runtime/LIBRARY_CHECKLIST.csv`)
- Remaining non-implemented rows in those files are primarily HAL/interop/file-IO/UI.

Conclusion:
- Statement/expression language core is broadly implemented for the current executable subset.
- Full MS-VBAL scope is not complete yet because module/project semantics are still missing as first-class implementation surfaces.

## Requirement Set

Tracked requirement classes in this checklist:

- Project model and multi-module compilation.
- Module kind model (procedural/class/document/form).
- Module attributes and visibility rules.
- Cross-module/project-qualified name resolution.
- Class instancing/default-instance behavior.
- Document/form module integration points.
- Project-level references and storage roundtrip (MS-OVBA).

## Deferred Scope

Allowed deferrals in this phase:

- Forms runtime/userform behavior (`forms-deferred`).
- HAL-adjacent host binding and external automation edges (`hal-adjacent`).

These remain required scope, but not blockers for initial project-model bring-up.

## Recommended Next Focus

Next work should prioritize an early module/project architecture pass before adding
more statement-level language surface.

Reason:
- Core language surface is mostly closed for the current subset.
- Module/project semantics are structural prerequisites for full MS-VBAL fidelity.
- Delaying project-model work increases retrofit cost across parser/binder/runtime/host boundaries.
