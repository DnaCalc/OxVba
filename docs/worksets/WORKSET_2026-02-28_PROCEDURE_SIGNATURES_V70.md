# WORKSET_2026-02-28_PROCEDURE_SIGNATURES_V70.md

## Purpose
Execute profile `v70` (`mvp-typing-procedure-signatures-v70`) in the `v67..v86` typing ladder.

## Scope
- Expand typed procedure signature handling with function return-type metadata.
- Apply return-type precedence for function/property-get signatures: `As <type>` > type character > `Def*` > `Variant`.
- Enforce stricter typed `ByRef` legality: temporary/non-variable rejection and exact typed match for non-Variant `ByRef` parameters.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `conformance/tests/byref_typed_mismatch_error.bas`
- `conformance/tests/function_return_explicit_as_precedence_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V70.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-procedure-signatures-v70
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-procedure-signatures-v70 -OutputDir docs/evidence/profiles/v70
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v70` closes when FO-V70-* obligations are pass, `v70` matrix cells are green, and typed procedure signature/ByRef legality fixtures are green.
