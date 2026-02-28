# WORKSET_2026-02-28_STRING_MUTATION_SLICES_V79.md

## Purpose
Execute profile `v79` (`mvp-string-mutation-and-slices-v79`) in the `v67..v86` typing ladder.

## Scope
- Add executable subset support for `Mid` statement mutation (`Mid(target, start[, count]) = value`).
- Preserve existing slice intrinsic execution while adding explicit coverage for type-character forms (`Left$`, `Right$`, `Mid$`).
- Reconcile deferred formal gates scheduled at `v79` for `v77..v78`.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `conformance/tests/string_mid_statement_mutation.bas`
- `conformance/tests/string_slice_ops_dollar.bas`
- `conformance/golden/smoke.csv`
- `docs/evidence/formal/EXTENDED_TODO.md`
- `docs/profile-status/PROFILE_STATUS_V79.md`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-string-mutation-and-slices-v79
./scripts/run-matrix.ps1 -ProfileScope mvp-string-mutation-and-slices-v79 -OutputDir docs/evidence/profiles/v79
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v79` closes when FO-V79-* obligations are pass, string mutation/slice conformance fixtures are green, deferred gate reconciliation for `v77..v78` is recorded (`dg-folded` or explicit follow-up in extended todo), and strict async Kani run `v79-kani` is started as `DG-V79-001`.
