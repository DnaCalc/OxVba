# Error Codes

Authoritative catalog of OxVba error-code families as of 2026-03-10.

## Scope

This file catalogs stable error and diagnostic code families that appear in current source or active specifications.

It does not catalog:
- HAL clause IDs such as `HAL-TIME-002`
- profile/workset identifiers
- generic Rust error strings with no stable code prefix

## Status Register

| Family | Status | Authority | Notes |
|---|---|---|---|
| `HAL-E-*` | implemented | `crates/oxvba-hal/src/error.rs` | Stable HAL capability/policy/adapter/profile failures. |
| `COM-E-*` | implemented | `crates/oxvba-hal/src/adapters/standard.rs`, `crates/oxvba-host/src/engine.rs` | COM activation, event subscription, callback, and object-lifecycle failures currently emitted as stable message prefixes rather than a centralized enum. |
| `PMR-E-*` | implemented | `crates/oxvba-compiler/src/project.rs`, `crates/oxvba-host/src/project.rs`, `crates/oxvba-host/src/engine.rs` | Project model/runtime diagnostics for module/reference/event-dispatch/project-graph flows. |
| `PMR-I-*` | implemented | `docs/DIAGNOSTIC_TAXONOMY.md` and generated diagnostics artifacts | Informational PMR diagnostics; not an error family, but part of the same registry surface. |
| `BIND-E-*` | implemented | `crates/oxvba-compiler/src/project.rs` | Early-bind/type-library subset diagnostics. |
| `VBP-E-*` | reserved | review/workset planning only | Reserved for possible future project packaging/build domain; not implemented in current source. |

## Implemented Families

### `HAL-E-*`

Source of truth:
- `crates/oxvba-hal/src/error.rs`

Implemented codes:
- `HAL-E-CAP-UNAVAILABLE`
- `HAL-E-POLICY-DENIED`
- `HAL-E-ADAPTER-FAULT`
- `HAL-E-UNSUPPORTED-PROFILE`

Notes:
- These are centralized and strongly typed through `HalErrorKind`.
- They are the only family in the current tree with a dedicated shared error-type mapping layer.

### `COM-E-*`

Primary sources:
- `crates/oxvba-hal/src/adapters/standard.rs`
- `crates/oxvba-host/src/engine.rs`

Implemented subfamilies currently present in source:
- `COM-E-OBJECT-MISSING`
- `COM-E-EVENT-CONNECTIONPOINT-MISSING`
- `COM-E-EVENT-ADVISE-FAILED`
- `COM-E-EVENT-PATH-UNSUPPORTED`
- `COM-E-EVENT-CALLBACK-MISSING`
- `COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`

Notes:
- These codes are stable string prefixes embedded in adapter/host diagnostics today.
- They are implemented, but not yet normalized behind a dedicated `ComErrorKind` or equivalent shared catalog type.
- `COM-E-*` should be treated as authoritative current behavior, not as proposal-only text.

### `PMR-E-*`

Primary sources:
- `crates/oxvba-compiler/src/project.rs`
- `crates/oxvba-host/src/project.rs`
- `crates/oxvba-host/src/engine.rs`

Implemented coverage includes:
- project/module/reference validation
- module header and attribute constraints
- `WithEvents` / `RaiseEvent` / `Implements` validation
- project qualification and cross-project execution constraints
- event-dispatch target and signature failures
- typelib/importlib resolution failures
- backend compile failures

Representative implemented codes:
- `PMR-E-PROJECT-NAME-INVALID`
- `PMR-E-MODULE-HEADER-INVALID`
- `PMR-E-REFERENCE-PROJECT-NOT-LOADED`
- `PMR-E-WITHEVENTS-MODULE-KIND`
- `PMR-E-RAISEEVENT-UNDECLARED`
- `PMR-E-EVENT-DISPATCH-TARGET-MISSING`
- `PMR-E-EVENT-DISPATCH-TARGET-AMBIGUOUS`
- `PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`
- `PMR-E-TYPELIB-IMPORTLIB-MISSING`
- `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`
- `PMR-E-TYPELIB-LIBID-UNRESOLVED`

Notes:
- `PMR-E-*` is already broader than pure project-manifest validation; it now includes runtime project-model/event-dispatch errors in the host layer.

### `PMR-I-*`

Primary sources:
- `docs/DIAGNOSTIC_TAXONOMY.md`
- generated PMR diagnostics artifacts

Current status:
- implemented informational family
- included here because it shares the PMR namespace and is part of the user-facing diagnostic register

Notes:
- `PMR-I-*` is not an error family and should not be counted as one in error-rate reporting.

### `BIND-E-*`

Source of truth:
- `crates/oxvba-compiler/src/project.rs`

Implemented codes:
- `BIND-E-TYPELIB-QUALIFIER-UNRESOLVED`
- `BIND-E-TYPELIB-CREATEOBJECT-UNSUPPORTED`
- `BIND-E-TYPELIB-MEMBER-UNSUPPORTED`
- `BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED`
- `BIND-E-TYPELIB-ARG-PARSE`
- `BIND-E-EVENT-ARITY-UNSUPPORTED`

Notes:
- This family is implemented in source even where some entries are currently emitted through free-form backend errors rather than a unified enum.

## Reserved / Proposal-Only Families

### `VBP-E-*`

Status:
- reserved, not implemented

Current basis:
- planning/review discussion only

Meaning:
- possible future family for VBA project/build/package-level diagnostics outside the current PMR/BIND/HAL/COM split

Constraint:
- do not document `VBP-E-*` as implemented until there is source-level emission or a committed spec that makes it normative.

## Non-Families Often Confused With Error Codes

These are real identifiers, but they are not error-code families:
- `HAL-TIME-*`
- `HAL-DES-*`
- `HAL-GEN-*`
- profile IDs such as `v506`
- evidence/workset/run IDs

Interpretation rule:
- if the identifier names a clause, gate, or conformance probe rather than a user/runtime diagnostic, it does not belong in the error-code family register.

## Maintenance Rule

When adding a new stable diagnostic family:
1. add or update the source-of-truth code/spec location
2. classify it here as `implemented`, `reserved`, or `proposal-only`
3. distinguish typed/shared families from string-prefix-only families
4. avoid claiming implementation from review text alone
