# WORKSET: MS-VBAL Module/Project Probe

## Objective

Establish the first implementation slice for full MS-VBAL project/module coverage in OxVba.

## Why This Next

- Core statement/expression language coverage is largely closed for the current subset.
- Full MS-VBAL scope requires project/module semantics not yet modeled as first-class runtime/compiler artifacts.
- Starting project-model work now reduces future rework in binder/host/runtime boundaries.

## Scope (This Probe Workset)

In scope:
- Define internal project graph model:
  - project identity
  - module identity and module kind (`procedural`, `class`, `document`, `form`)
  - module attribute bag (initial parse/retain only)
- Introduce multi-module compile entry API (without changing HAL behavior yet).
- Implement cross-module symbol table skeleton and duplicate module-name diagnostics.
- Add conformance fixtures for:
  - duplicate module names in one project
  - cross-module public procedure call resolution
  - project-qualified/module-qualified references (initial deterministic subset)

Out of scope:
- Forms runtime execution semantics.
- Full document-module host event wiring.
- Full COM reference/type-library import.
- MS-OVBA file ingest/emit.

## Inputs

- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`
- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`

## Deliverables

1. Project model types and parser/binder plumbing (initial slice).
2. New conformance fixtures + golden updates for module/project subset.
3. Coverage/index updates:
   - `docs/evidence/language/COVERAGE_INDEX.csv` (new project-model rows)
   - `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv` status promotion
4. Profile status/workset evidence for this probe slice.

## Exit Criteria

1. OxVba can compile a project consisting of multiple standard modules with deterministic symbol resolution in the supported subset.
2. Duplicate module names are diagnosed.
3. At least one project-qualified and one module-qualified resolution path has executable conformance evidence.
