# Project Module Reference HAL Integration v1

Status: `working-draft`
Date: 2026-03-02

## 1. Purpose

Define how Project/Module/Reference semantics interact with HAL boundaries, without collapsing language semantics into platform adapters.

Principle:

- Project semantics remain compiler/language responsibilities.
- HAL provides host/environment capabilities required to realize host-specific project behaviors.

## 2. Why HAL Is Involved

MS-VBAL identifies host and library project mechanisms as implementation-defined (`SPEC-...-01231`, `...-01233`, `...-01240`, `...-01241`).

Those mechanisms include:

- discovering host projects,
- exposing referenced project/type-library metadata,
- open-host project extension workflows,
- persistent project transport and storage boundaries.

These are host-environment concerns and therefore HAL-adjacent.

Class-module note:

- class semantic correctness (lifecycle/property/diagnostics) remains in PMR/runtime,
- COM ABI realization for class/object host boundaries remains in HAL/host adapters.

## 3. Proposed HAL Capability Extensions (Planned)

These are planned additions, not yet implemented capabilities:

1. `ProjectCatalog`
- enumerate host-visible projects,
- classify project kind (`Source`, `Host`, `Library`),
- expose stable project identity metadata.

2. `ProjectReferenceProvider`
- provide ordered reference lists,
- resolve reference identity to binder-ready descriptors,
- surface deterministic failures for missing/invalid references.

3. `ProjectStorage`
- import/export project containers (MS-OVBA-aligned payloads),
- preserve module headers and reference order,
- report unsupported features via structured diagnostics.

4. `ClassActivationPolicy`
- expose host constraints that affect default-instance materialization for predeclared/global namespace classes.

### 3.1 Current v1 Snapshot (Implemented Subset)

Local deterministic scaffold now exists for type-library/importlib reference binding in the host project model:

- `ProjectReference.importlib_hint` (explicit importlib identity hint),
- `ProjectNode::set_reference_importlib(...)`,
- `ProjectNode::resolve_type_library_references(...)`,
- deterministic result taxonomy:
  - `PMR-I-TYPELIB-BOUND`,
  - `PMR-E-TYPELIB-IMPORTLIB-MISSING`,
  - `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`,
  - `PMR-E-TYPELIB-IMPORTLIB-AMBIGUOUS`.

This is intentionally host-neutral and does not yet claim registered-host/COM parity.

## 4. Contract Shape (Rust-level Draft)

```rust
trait ProjectCatalogHal {
    fn list_projects(&self) -> HalResult<Vec<ProjectDescriptor>>;
    fn get_project(&self, project_name: &str) -> HalResult<ProjectDescriptor>;
}

trait ProjectReferenceHal {
    fn list_references(&self, project_name: &str) -> HalResult<Vec<ProjectReferenceDescriptor>>;
    fn resolve_reference(&self, reference: &ProjectReferenceDescriptor) -> HalResult<ResolvedReference>;
}

trait ProjectStorageHal {
    fn load_project_package(&self, source: &ProjectStorageSource) -> HalResult<ProjectPackage>;
    fn save_project_package(&self, package: &ProjectPackage, target: &ProjectStorageTarget) -> HalResult<()>;
}
```

Design constraints:

- deterministic success/failure envelope (`HalResult`),
- stable error codes,
- capability and policy gating consistent with existing HAL model,
- no silent fallback for unsupported paths.

## 5. Error and Diagnostics Contract

Planned stable codes:

- `HAL-E-PROJ-NOT-FOUND`
- `HAL-E-PROJ-REF-UNRESOLVED`
- `HAL-E-PROJ-STORAGE-UNSUPPORTED`
- `HAL-E-PROJ-STORAGE-INVALID`

PMR-facing stable diagnostics for class/event compile-time legality (implemented subset):

- canonical source: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`
- generated list: `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`

Runtime event ordering/subscription parity and full event dispatch semantics remain tracked as conformance/divergence items (`CCT-040`, `CCT-041`, `DIV-0003`, `DIV-0004`).

Phase routing:

- metadata failures that prevent binding should surface as compile-time diagnostics.
- runtime host access failures should preserve structured runtime diagnostics with phase metadata.

## 6. Profile Expectations (Initial)

- Windows profile:
  - highest priority for host project/reference integration.
  - initial target for project catalog and reference provider realism.

- Linux/macOS profiles:
  - deterministic baseline first.
  - host catalog/storage features may be unsupported or partial by policy.

- wasm/null profiles:
  - deterministic unsupported floor for project host integration unless explicitly virtualized.

## 7. Verification Plan

Integrate PMR HAL clauses into both:

- PMR clause catalog (`PMR-HAL-*`),
- HAL clause governance (future `HAL-PROJ-*` family once capability IDs are added).

Required lane types:

- adapter contract tests,
- host integration phase-routing tests,
- storage roundtrip tests (when MS-OVBA mapping is available),
- oracle foldback for host-specific behavior claims.

## 8. Open Questions

- Should project catalog/references be compile-time only services or callable runtime services as well?
- How to version project-package schema while preserving deterministic roundtrip guarantees?
- Where to place policy controls for allowing host project mutation (`open host project`) in secure/CI modes?

## 9. Immediate Follow-up

1. Add PMR HAL capability placeholders to HAL design docs and uncertainty register.
2. Define descriptor payload schemas for project/ref metadata.
3. Add a deterministic mock adapter for `ProjectCatalog` and `ProjectReferenceProvider`.
4. Defer `ProjectStorage` parity claims until MS-OVBA section-level extraction is available.
5. Keep class semantic conformance lane executable independent of COM capability support per profile.
6. Evolve local importlib scaffold into a HAL-backed resolver interface with explicit policy and phase mapping.
7. Use `PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md` as the next contract refinement base.
