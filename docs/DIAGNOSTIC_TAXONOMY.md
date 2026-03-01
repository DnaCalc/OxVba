# Diagnostic Taxonomy

This document consolidates the current user-facing compiler/typecheck diagnostic surface for typing-track profiles (`v67..v72`).

## Stability Notes
- Diagnostics are deterministic for fixed input and profile scope.
- Message text is intentionally explicit and currently treated as user-facing evidence for conformance fixtures.
- Future profiles may introduce stable diagnostic codes; this taxonomy is the baseline mapping for that migration.

## Categories

| Category | Representative Message Prefix | Trigger Examples | Current Source |
|---|---|---|---|
| Declaration discipline | `duplicate declaration:` | duplicate `Dim` declarations (scalar/array base) | `crates/oxvba-compiler/src/typecheck.rs` |
| Scope/name collision | `name collision between variable and procedure:` | variable declaration collides with procedure symbol | `crates/oxvba-compiler/src/typecheck.rs` |
| Label integrity | `duplicate label declaration:` / `gosub target label not found:` / `on error goto target label not found:` | duplicate label, missing `GoSub`/`On Error GoTo <label>` target | `crates/oxvba-compiler/src/typecheck.rs` |
| Declaration requirement | `use of undeclared variable:` | `Option Explicit` with undeclared symbol use | `crates/oxvba-compiler/src/typecheck.rs` |
| Assignment typing | `type mismatch in assignment:` | assignment not assignable under declared/default type | `crates/oxvba-compiler/src/typecheck.rs` |
| Call target resolution | `call to unknown procedure:` | unresolved call target that is not object-like late target | `crates/oxvba-compiler/src/typecheck.rs` |
| Call argument mapping | `procedure <name> expects between...` / `missing required argument` / `duplicate argument for parameter` / `positional argument cannot follow named argument` | arity and named-argument shape violations | `crates/oxvba-compiler/src/typecheck.rs` |
| Argument typing | `argument type mismatch for parameter` | argument type not assignable to parameter type | `crates/oxvba-compiler/src/typecheck.rs` |
| ByRef legality | `ByRef parameter <name> requires variable argument` / `ByRef parameter <name> requires exact type match` | non-variable ByRef argument, typed mismatch for non-Variant ByRef | `crates/oxvba-compiler/src/typecheck.rs` |
| Late-bound routing state | `late-bound default-member call is not yet executable:` | object-like call target classified as late/default-member call | `crates/oxvba-compiler/src/typecheck.rs` |
| Unsupported syntax/semantics | `unsupported statement:` | parsed-but-unsupported statement paths | `crates/oxvba-compiler/src/typecheck.rs` |

## Rollup Coverage (Track A)
- `v68`: declaration diagnostics expansion.
- `v69`: default typing + type-character precedence diagnostics.
- `v70`: typed ByRef legality + typed function-return diagnostics.
- `v71`: early/mixed/late call classification diagnostics.
- `v72`: this consolidated taxonomy artifact.
- `v157`: host execution phase classification for compile-time vs runtime diagnostics (`PhaseDiagnostic` in `crates/oxvba-host/src/engine.rs`), with compile-time precedence checks over runtime paths.

## Next Improvement Targets
1. Introduce stable diagnostic IDs without changing message semantics.
2. Add structured machine-readable diagnostics snapshot export for conformance lanes.
3. Split user-facing message text from internal invariant detail (keep deterministic ordering).
