# WORKSET_2026-02-28_WITH_BLOCK_V60.md

## Purpose
Execute profile `v60` (`mvp-lang-with-block-v60`) in the `v57..v66` ladder.

## Scope
- Implement `With ... End With` subset for member-style assignment/access lines.
- Support nested `With` rewriting for deterministic target aliasing.
- Validate behavior through compiler/host tests and conformance matrix.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `crates/oxvba-host/src/engine.rs`
- `conformance/tests/with_block_basic.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V60.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host --lib
./scripts/run-formal.ps1 -ProfileScope mvp-lang-with-block-v60
./scripts/run-matrix.ps1 -ProfileScope mvp-lang-with-block-v60 -OutputDir docs/evidence/profiles/v60
```

## Closure Signals
`v60` closes when FO-V60-* obligations are pass and `v60` VM/JIT matrix cells are green.
