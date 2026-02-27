# WORKSET_2026-02-27_OPTIONAL_PARAMS_V37.md

## Purpose
Execute profile `v37` (`mvp-lang-optional-params-v37`) as the first step of the `v37..v56` ladder.

## Scope
- Parse procedure signatures with trailing `Optional` params and integer literal defaults.
- Allow omitted optional call arguments while preserving required-arg checks.
- Materialize omitted defaults during call lowering.
- Add conformance/formal evidence for omitted-vs-explicit behavior.

## Implementation Notes
- `Optional ByRef` is intentionally rejected in this subset.
- Required parameters after optional parameters are rejected in this subset.
- Default literals currently supported: signed integer values.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-optional-params-v37
```

## Completion Signal
`v37` closes when conformance tests for optional defaults/overrides pass on VM+JIT and `FO-V37-*` obligations are present in formal output.
