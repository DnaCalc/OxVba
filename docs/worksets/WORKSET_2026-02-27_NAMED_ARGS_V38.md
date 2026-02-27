# WORKSET_2026-02-27_NAMED_ARGS_V38.md

## Purpose
Execute profile `v38` (`mvp-lang-named-args-v38`) for named call-argument semantics.

## Scope
- Parse call arguments in `name := expr` form.
- Bind named arguments by procedure-parameter name.
- Enforce deterministic call-shape rules:
  - no positional argument after named arguments,
  - no duplicate parameter assignment,
  - no unknown parameter names.

## Implementation Notes
- Positional args are still supported and can be combined with named args only when positional args come first.
- Named args may target required and optional parameters.
- Omitted optional parameters continue to materialize defaults from `v37`.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-named-args-v38
```

## Completion Signal
`v38` closes when named-arg conformance and `FO-V38-*` obligations are green (or formally logged per non-blocking policy).
