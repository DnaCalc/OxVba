# Formal Hardening Pass: PMR + HAL + Declare (2026-03-03)

Status: `completed`
Scope: strengthen executable formalization discipline in code and verification pipeline.

## 1. Goals

1. Encode additional PMR pre/postconditions directly in implementation.
2. Harden type-library/importlib reference handling with explicit invariants.
3. Add Declare/dynamic-link descriptor contract checks at VM/HAL boundary.
4. Enforce PMR clause-catalog drift checks in standard meta pipeline.

## 2. Implemented Changes

## 2.1 PMR compile contract hardening

File: `crates/oxvba-compiler/src/project.rs`

- Added `validate_compiled_project_contract(...)` postcondition checks for:
  - sorted/unique `host_exports`,
  - sorted/unique `reference_visible_exports`,
  - `reference_visible_exports` subset-of `host_exports`,
  - no `Option Private Module` leakage in reference-visible surface,
  - exports map to active project + known procedural modules.
- Wired into `compile_project(...)` with deterministic internal contract failure surface:
  - `PMR-E-INTERNAL-CONTRACT` (via backend compile error message).
- Added deterministic compiler tests for:
  - unsorted export surface rejection,
  - subset contract rejection,
  - compile determinism for identical manifest input.

## 2.2 Typelib/importlib resolver invariants

File: `crates/oxvba-host/src/project.rs`

- Added explicit precondition:
  - `set_reference_importlib(...)` now rejects non-`TypeLibrary` references with
    `PMR-E-TYPELIB-KIND-MISMATCH`.
- Kept and exercised `PMR-E-REFERENCE-NOT-FOUND` for unknown targets.
- Hardened resolver behavior:
  - importlib matching normalized and whitespace-safe,
  - empty catalog importlib values ignored,
  - ambiguous candidate names sorted + deduplicated.
- Added postcondition debug assertions:
  - exactly one resolution record per type-library reference,
  - all type-library references transition out of `Unbound`.
- Added tests for:
  - kind mismatch rejection,
  - catalog-order determinism,
  - non-typelib reference state non-mutation.

## 2.3 Declare/HAL descriptor contract checks

Files:
- `crates/oxvba-hal/src/traits.rs`
- `crates/oxvba-vm/src/interpreter.rs`

- Added `DynLinkDescriptorView::contract_violation()` contract validator:
  - non-empty `declared_name/library/alias`,
  - `marshal_lane == "m0-deterministic"`,
  - `calling_convention == "platform-default"`,
  - `selection_policy` consistent with `ordinal_alias`.
- VM now validates descriptor contract before `invoke_descriptor(...)`.
- Violations route deterministically through HAL adapter-fault path.
- Added VM tests for:
  - empty-library descriptor violation,
  - ordinal/policy mismatch violation.

## 2.4 Verification pipeline hardening

Files:
- `scripts/check-pmr-clause-drift.ps1` (new)
- `scripts/meta-check.ps1` (updated)

- Added PMR clause drift checker mirroring HAL drift checker.
- `meta-check` now enforces both:
  - HAL clause catalog drift,
  - PMR clause catalog drift.

## 3. Validation

Executed and passing:

- `cargo test -p oxvba-compiler project::tests:: -- --nocapture`
- `cargo test -p oxvba-host project::tests:: -- --nocapture`
- `cargo test -p oxvba-vm declare_invoke_descriptor -- --nocapture`
- `./scripts/check-hal-clause-drift.ps1`
- `./scripts/check-pmr-clause-drift.ps1`
- `./scripts/meta-check.ps1`

## 4. Residual Formalization Gaps

1. PMR still uses source-rewrite bridge instead of module-aware IR bind.
2. Typelib resolution scaffold is host-local; HAL-backed resolver + oracle foldback remains open (`CCT-043` / `ODG-041`).
3. Descriptor contract is runtime-validated; compile-time descriptor contract proofs remain future work.
4. Kani/model-check lanes for PMR resolver and descriptor contract remain deferred to async formal lane expansion.
