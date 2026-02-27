# WORKSET_2026-02-27_ON_ERROR_GOTO_LABEL_V41.md

## Purpose
Execute profile `v41` (`mvp-lang-on-error-goto-label-v41`) to complete label-targeted error handler transfer semantics.

## Scope
- Parse `On Error GoTo <label>` statements.
- Validate handler labels exist in procedure scope.
- Emit label-target patching for error handler instructions.
- Extend VM runtime with handler-target error mode.

## Implementation Notes
- `On Error GoTo 0` clears both resume-next and goto-label modes.
- `On Error Resume Next` clears goto-label mode.
- Runtime error dispatch order in this subset:
  1. `Resume Next` mode
  2. `GoTo label` handler
  3. default fail

## Validation Commands
```powershell
cargo test -p oxvba-vm
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-on-error-goto-label-v41
```

## Completion Signal
`v41` closes when label-handler conformance fixtures and `FO-V41-*` obligations are green (or formally logged under non-blocking policy).
