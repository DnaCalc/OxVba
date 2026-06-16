# Error Codes

Authoritative catalog of OxVba error-code families as of 2026-06-16 (clean
pipeline: `oxvba-bind` → `oxvba-bundle` → `oxvba-vm2`). Structured diagnostics
are carried through `oxvba-diagnostics`.

## Scope

This file catalogs stable error and diagnostic code families that appear in
current source. It does not catalog HAL clause IDs (`HAL-TIME-002` etc.),
profile/workset identifiers, or generic Rust error strings with no stable
prefix.

## Status Register

| Family | Status | Authority | Notes |
|---|---|---|---|
| `SYN-E-*` | implemented | `crates/oxvba-syntax/src/parser.rs` (`ParseError::to_diagnostic`) | Syntax parse diagnostics with byte offsets and, when source text is available, line/column/snippet rendering. |
| `SYM-E-*` | implemented | `crates/oxvba-symbol/src/model.rs` (`SymbolModelError::to_diagnostic`) | Symbol-model failures such as duplicate declarations, unknown internal scopes, and preprocessing failures. Parse failures preserved by the symbol layer keep their `SYN-E-PARSE` code. |
| `BIND-E-*` | implemented | `crates/oxvba-bind/src/error.rs` (`BindError::to_diagnostic`) | Binder failures: unresolved names, invalid assignments, malformed CST shapes, unsupported lowered constructs, and symbol-model handoff failures. |
| `BUND-E-*` | implemented | `crates/oxvba-host/src/engine.rs` (`linearize_diagnostic`) | Defensive Core IR linearization failures. These indicate an internal compiler bug, not invalid user VBA. |
| `PROJ-E-*` | implemented | `crates/oxvba-project/src/error.rs` (`BasProjError::to_diagnostic`) | `.basproj`/`.vbp` project loading, discovery, entry-point, reference, export, and COM-server project failures. |
| `RUN-E-*` | implemented | `crates/oxvba-vm2/src/lib.rs`, `crates/oxvba-host/src/engine.rs` | Runtime infrastructure diagnostics such as VM link failures and unavailable JIT execution. VBA runtime errors still carry the numeric VBA error number structurally. |
| `HOST-E-*` | implemented | `crates/oxvba-cli/src/main.rs` | CLI/host orchestration failures such as runner bootstrap resolution. |
| VBA runtime error numbers | implemented | `crates/oxvba-vm2/src/lib.rs` (`VmError`), `crates/oxvba-vm2/src/arith.rs` (`ArithError`) | The user-facing error model: numeric VBA codes (6 Overflow, 7 Out of memory, 9 Subscript out of range, 11 Division by zero, 13 Type mismatch, 28 Out of stack space, 94 Invalid use of Null, …) carried structurally, never matched from strings. |
| `HAL-E-*` | implemented | `crates/oxvba-hal/src/error.rs`, `crates/oxvba-hal/src/project.rs` | Stable HAL capability/policy/adapter/profile/project-boundary failures. Core factories emit `HAL-E-CAP-UNAVAILABLE`, `HAL-E-POLICY-DENIED`, `HAL-E-ADAPTER-FAULT`, `HAL-E-UNSUPPORTED-PROFILE`; project catalog/reference callbacks also emit stable `HAL-E-PROJ-*` codes. |
| `COM-E-*` | implemented | `crates/oxvba-com/src/{windows_bridge,windows_invoke,windows_runtime_state}.rs`, `crates/oxvba-hal/src/adapters/standard/{com.rs,mod.rs}` | COM activation, typelib resolution, event subscription/callback, value transport, object lifecycle, and dispatch failures. Existing string prefixes are preserved as structured diagnostic codes where they cross the COM/VM/host boundary. |
| `VBP-E-*` | reserved | planning only | Do not document as implemented until source-level emission exists. |

## Removed With The Legacy Stack

`PMR-E-*`, `PMR-I-*`, and the old legacy-stack `BIND-E-*` meanings were emitted by the deleted
`oxvba-compiler`/legacy host and no longer exist in source (see git history
and `docs/archive/`). Equivalent project-model and early-binding legality
checks now live in `oxvba-symbol`/`oxvba-bind`. New clean-stack `BIND-E-*`
codes are owned by `oxvba-bind` and must not be treated as resurrection of the
legacy PMR diagnostic catalog.

## Non-Families Often Confused With Error Codes

Real identifiers that are not error-code families:
- `HAL-TIME-*`, `HAL-DES-*`, `HAL-GEN-*`, `HAL-DYN-*` (HAL conformance clause IDs)
- profile IDs such as `v506`
- evidence/workset/run IDs
