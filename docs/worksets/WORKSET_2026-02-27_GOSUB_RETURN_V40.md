# WORKSET_2026-02-27_GOSUB_RETURN_V40.md

## Purpose
Execute profile `v40` (`mvp-lang-gosub-return-v40`) for intra-procedure `GoSub`/`Return` semantics.

## Scope
- Parse label declarations and `GoSub <label>` statements.
- Parse `Return` as gosub return operation.
- Validate gosub labels are defined in the same procedure body.
- Emit gosub calls via label-patched call targets and runtime `Return`.

## Implementation Notes
- This profile uses a bounded subset:
  - labels are local to the containing procedure,
  - runtime branching uses existing `CallProc`/`Return` stack behavior.
- Missing labels are rejected during type-check.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-gosub-return-v40
```

## Completion Signal
`v40` closes when gosub conformance fixtures and `FO-V40-*` obligations are green (or formally logged under non-blocking policy).
