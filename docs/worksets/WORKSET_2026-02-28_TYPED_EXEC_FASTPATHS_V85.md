# WORKSET_2026-02-28_TYPED_EXEC_FASTPATHS_V85.md

## Purpose
Execute profile `v85` (`mvp-typed-execution-fastpaths-v85`) in the `v67..v86` typing ladder.

## Scope
- Introduce typed hot-path VM execution helpers for core integer slot operations (`AddConst`, `SubConst`, `CopySlot`, comparisons, `IncSlot`).
- Keep semantic fallback to baseline instruction handlers whenever fast-path preconditions do not hold.
- Add typed fast-path parity checks (fast-path enabled vs disabled) and VM/JIT equivalence evidence for typed hot-loop corpus.

## Implementation Targets
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-vm/src/lib.rs`
- `crates/oxvba-host/src/engine.rs`
- `conformance/tests/*.bas`
- `conformance/golden/smoke.csv`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-typed-execution-fastpaths-v85
./scripts/run-matrix.ps1 -ProfileScope mvp-typed-execution-fastpaths-v85 -OutputDir docs/evidence/profiles/v85
./scripts/run-bench.ps1 -ProfileScope mvp-typed-execution-fastpaths-v85 -Iterations 1 -OutputPath docs/evidence/profiles/v85/benchmark_latest.md -OutputCsvPath docs/evidence/profiles/v85/benchmark_latest.csv
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v85` closes when FO-V85-* obligations are pass, matrix gate cells are green for profile scope, benchmark artifacts are recorded for `v85`, and strict async Kani run `v85-kani` is started and tracked as deferred gate `DG-V85-001`.
