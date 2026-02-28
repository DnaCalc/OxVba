# WORKSET_2026-02-28_KANI_HARNESS_EXPANSION_V58.md

## Purpose
Execute profile `v58` (`mvp-kani-harness-expansion-v58`) in the `v57..v66` ladder.

## Scope
- Expand bounded Kani harness coverage beyond VM/emitter:
  - syntax lexer,
  - syntax parser,
  - optimizer rewrite invariants.
- Keep formal failures non-blocking but visible in evidence.

## Implementation Targets
- `crates/oxvba-syntax/src/lexer.rs`
- `crates/oxvba-syntax/src/parser.rs`
- `crates/oxvba-compiler/src/optimize.rs`
- `docs/evidence/formal/obligations.csv`
- `docs/profile-status/PROFILE_STATUS_V58.md`

## Validation Commands
```powershell
cargo test -p oxvba-host --lib
./scripts/run-formal.ps1 -ProfileScope mvp-kani-harness-expansion-v58
./scripts/run-matrix.ps1 -ProfileScope mvp-kani-harness-expansion-v58 -OutputDir docs/evidence/profiles/v58
```

## Closure Signals
`v58` closes when:
- FO-V58-* obligations pass in the non-blocking formal report,
- `docs/evidence/profiles/v58/gate_report.md` is green,
- syntax/parser/optimizer Kani harness entries are present and tracked.
