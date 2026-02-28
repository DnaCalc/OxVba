# WORKSET_2026-02-28_OPTION_EXPLICIT_DIAGNOSTICS_V68.md

## Purpose
Execute profile `v68` (`mvp-typing-option-explicit-diagnostics-v68`) in the `v67..v86` typing ladder.

## Scope
- Extend declaration diagnostics beyond undeclared-variable checks:
  - duplicate declaration detection,
  - duplicate label declaration detection,
  - declaration/procedure name-collision diagnostics.
- Keep `Option Explicit` undeclared-use diagnostics stable.
- Add conformance fixtures for new diagnostic paths.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `conformance/tests/duplicate_dim_error.bas`
- `conformance/tests/duplicate_label_error.bas`
- `conformance/tests/declaration_collision_proc_name_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V68.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-option-explicit-diagnostics-v68
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-option-explicit-diagnostics-v68 -OutputDir docs/evidence/profiles/v68
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v68` closes when FO-V68-* obligations are pass, `v68` matrix cells are green, and diagnostics for duplicate declarations/labels/collisions are covered by compile+conformance tests.
