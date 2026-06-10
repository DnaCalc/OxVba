# Error Codes

Authoritative catalog of OxVba error-code families as of 2026-06-10 (clean
pipeline: `oxvba-bind` → `oxvba-bundle` → `oxvba-vm2`).

## Scope

This file catalogs stable error and diagnostic code families that appear in
current source. It does not catalog HAL clause IDs (`HAL-TIME-002` etc.),
profile/workset identifiers, or generic Rust error strings with no stable
prefix.

## Status Register

| Family | Status | Authority | Notes |
|---|---|---|---|
| VBA runtime error numbers | implemented | `crates/oxvba-vm2/src/lib.rs` (`VmError`), `crates/oxvba-vm2/src/arith.rs` (`ArithError`) | The user-facing error model: numeric VBA codes (6 Overflow, 7 Out of memory, 9 Subscript out of range, 11 Division by zero, 13 Type mismatch, 28 Out of stack space, 94 Invalid use of Null, …) carried structurally, never matched from strings. |
| Bind diagnostics | implemented | `crates/oxvba-bind/src/error.rs` (`BindError`) | Compile-time failures: parse, unresolved names, invalid assignment, malformed/unsupported constructs. Enum-typed, not string-prefixed. |
| `HAL-E-*` | implemented | `crates/oxvba-hal/src/error.rs` | Stable HAL capability/policy/adapter/profile failures, strongly typed through `HalErrorKind`: `HAL-E-CAP-UNAVAILABLE`, `HAL-E-POLICY-DENIED`, `HAL-E-ADAPTER-FAULT`, `HAL-E-UNSUPPORTED-PROFILE`. |
| `COM-E-*` | implemented | `crates/oxvba-com/src/{windows_bridge,windows_invoke,windows_runtime_state}.rs`, `crates/oxvba-hal/src/adapters/standard/{com.rs,mod.rs}` | COM activation, typelib resolution, event subscription/callback, value transport, and object-lifecycle failures as stable string prefixes (e.g. `COM-E-EVENT-ADVISE-FAILED`, `COM-E-TYPELIB-IDENTITY-UNRESOLVED`, `COM-E-VALUE-TRANSPORT-UNSUPPORTED`, `COM-E-STATE-POISONED`). Not yet normalized behind a shared enum. |
| `BASPROJ` errors | implemented | `crates/oxvba-project/src/error.rs` (`BasProjError`) | Project-file loading/closure failures, enum-typed. |
| `VBP-E-*` | reserved | planning only | Do not document as implemented until source-level emission exists. |

## Removed With The Legacy Stack

`PMR-E-*`, `PMR-I-*`, and `BIND-E-*` were emitted by the deleted
`oxvba-compiler`/legacy host and no longer exist in source (see git history
and `docs/archive/`). Equivalent project-model and early-binding legality
checks now live in `oxvba-symbol`/`oxvba-bind` and surface as `BindError`;
re-introducing stable string codes for them is a future decision, not current
behavior.

## Non-Families Often Confused With Error Codes

Real identifiers that are not error-code families:
- `HAL-TIME-*`, `HAL-DES-*`, `HAL-GEN-*`, `HAL-DYN-*` (HAL conformance clause IDs)
- profile IDs such as `v506`
- evidence/workset/run IDs
