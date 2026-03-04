# Project Module Reference Typelib/Importlib HAL Draft v1

Status: `working-draft`  
Date: 2026-03-03  
Scope: deterministic reference-binding contract between PMR model and HAL-host integration

## 1. Purpose

Define a precise first contract for resolving `ReferenceKind::TypeLibrary` references using explicit `importlib` identity, while preserving OxVba priorities:

1. Robustness (deterministic outcomes and diagnostics),
2. Compatibility (traceability to MS-OAUT/MS-VBAL semantics),
3. Performance (stable pre-bind metadata suitable for cached lookup in future).

## 2. Source Anchors

- MS-OAUT extracted conformance anchor:
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0561` (importlib-based type definition location).
- PMR clause link:
  - `PMR-REF-002` (`PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.*`).

## 3. Current Implemented Subset

Implemented in `crates/oxvba-host/src/project.rs`:

- `ProjectReference.importlib_hint: Option<String>`
- `ProjectNode::set_reference_importlib(...)`
- `ProjectNode::resolve_type_library_references(...)`
- `TypeLibraryCatalogEntry`
- `TypeLibraryBindingStatus`

Deterministic status codes:

- `PMR-I-TYPELIB-BOUND`
- `PMR-E-TYPELIB-IMPORTLIB-MISSING`
- `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`
- `PMR-E-TYPELIB-IMPORTLIB-AMBIGUOUS`

This subset resolves by explicit importlib identity and updates `ReferenceBindingState` (`Bound`/`Failed`) for type-library references.

## 4. Contract Model (Draft)

### 4.1 Inputs

- Ordered PMR reference list.
- For `TypeLibrary` references:
  - logical reference name (project-facing),
  - explicit importlib hint (`importlib_hint`),
  - host-provided catalog entries (name/importlib/version).

### 4.2 Resolution Rule

For each `TypeLibrary` reference:

1. If `importlib_hint` missing -> `MissingImportLibHint`.
2. Match catalog entries by case-insensitive `importlib`.
3. 0 matches -> `ImportLibUnresolved`.
4. 1 match -> `Bound` with version metadata.
5. >1 matches -> `ImportLibAmbiguous` with deterministic sorted candidate names.

Non-type-library references are not processed by this resolver.

### 4.3 Determinism Requirements

- Input order of PMR references is preserved.
- Ambiguous candidate names are sorted for stable diagnostics.
- No silent fallback to reference name, registry default, or first-hit behavior.

## 5. HAL Interaction Shape (Next Step)

Planned host abstraction boundary:

```rust
trait ProjectReferenceHal {
    fn resolve_typelib_importlib(
        &self,
        project_name: &str,
        reference_name: &str,
        importlib_hint: &str,
    ) -> HalResult<ResolvedTypeLibrary>;
}
```

`ResolvedTypeLibrary` should carry:

- canonical identity,
- importlib token,
- version tuple,
- optional provenance metadata (registry/file/package).

## 6. Phase and Policy Mapping (Draft)

- Compile-time mode:
  - unresolved/ambiguous/missing importlib is surfaced before execution.
- Runtime mode:
  - pre-bound descriptors may still fail if host policy denies activation paths, but binding diagnostics remain stable.

Policy expectations:

- no host mutation required for pure type-library resolution,
- deterministic mode forbids interactive reference repair prompts.

## 7. Conformance Plan

Near-term local lanes (implemented):

- unique importlib bind succeeds,
- missing importlib hint fails deterministically,
- ambiguous importlib fails deterministically.

Deferred oracle lanes:

- registered host (Excel/VBA) parity with real typelib registration behavior,
- HAL-backed resolver across platform profiles with explicit unsupported handling on non-Windows lanes where appropriate.

Tracking:

- `CCT-043` (`in-progress`)
- `ODG-041` (still open, now with local scaffold landed)

## 8. Open Questions

1. Should PMR accept fallback from missing `importlib_hint` to reference-name lookup in non-strict profiles?
2. How should version conflicts be represented when multiple typelib versions share importlib identity?
3. Where should resolver caching live (host adapter vs PMR bind layer) for deterministic invalidation?

## 9. Immediate Next Actions

1. Add this contract as a linked extension in `PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`.
2. Add compile-time phase-routing tests once PMR compile pipeline consumes bound reference metadata.
3. Add host fixture harness for registered typelib parity probes and close `ODG-041`.

