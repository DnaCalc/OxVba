# WORKSET_2026-02-28_STRING_COMPARE_SEARCH_V78.md

## Purpose
Execute profile `v78` (`mvp-string-compare-search-v78`) in the `v67..v86` typing ladder.

## Scope
- Wire `Option Compare` mode capture into bound-module state (`Binary`, `Text`, `Database`).
- Extend string compare/search execution subset with `InStrRev` and `Like` lowering/runtime support.
- Ensure compare/search intrinsics use mode-aware instruction metadata and remain deterministic in current numeric-string subset.
- Add mode-scoped conformance fixtures and regression tests for compare/search paths.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `crates/oxvba-compiler/src/optimize.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `conformance/tests/string_compare_option_binary.bas`
- `conformance/tests/string_compare_option_text.bas`
- `conformance/tests/stdlib_advanced_instrrev_like.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V78.md`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-string-compare-search-v78
./scripts/run-matrix.ps1 -ProfileScope mvp-string-compare-search-v78 -OutputDir docs/evidence/profiles/v78
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v78` closes when FO-V78-* obligations are pass, compare/search conformance fixtures are green (including mode-scoped `Option Compare` snapshots), and strict async Kani is started and tracked as deferred gate `DG-V78-001`.
