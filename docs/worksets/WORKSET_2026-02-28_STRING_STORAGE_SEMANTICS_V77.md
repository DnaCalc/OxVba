# WORKSET_2026-02-28_STRING_STORAGE_SEMANTICS_V77.md

## Purpose
Execute profile `v77` (`mvp-string-storage-semantics-v77`) in the `v67..v86` typing ladder.

## Scope
- Introduce a typed string sentinel constant path for `vbNullString` in current executable subset.
- Keep resolver/typecheck/emitter behavior aligned so `vbNullString` is consistently modeled as string-typed.
- Add regression coverage for string-vs-object assignment boundaries around `vbNullString`.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `conformance/tests/string_vbnullstring_basic.bas`
- `conformance/tests/string_vbnullstring_object_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V77.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-string-storage-semantics-v77
./scripts/run-matrix.ps1 -ProfileScope mvp-string-storage-semantics-v77 -OutputDir docs/evidence/profiles/v77
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v77` closes when FO-V77-* obligations are pass, `v77` matrix cells are green, and `vbNullString` typed sentinel handling is stable across resolver/typecheck/emitter and conformance fixtures.
