# WORKSET_2026-02-27_STDLIB_CONVERSION_CORE_V45.md

## Purpose
Execute profile `v45` (`mvp-stdlib-conversion-core-v45`) for intrinsic conversion function baseline.

## Scope
- Add expression parsing support for `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, and `Str` wrappers.
- Lower conversion wrappers into current integer expression model for deterministic execution.
- Add conformance/formal evidence for simple and nested conversion chains.

## Implementation Notes
- Current runtime remains integer-slot based; conversion wrappers are currently identity lowering in int-domain subset.
- This profile establishes parser/runtime integration points for later richer variant/string/date conversion semantics.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-conversion-core-v45
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-conversion-core-v45 -OutputDir docs/evidence/profiles/v45
./scripts/meta-check.ps1 -Fast -Conformance -Formal
```

## Completion Signal
`v45` closes when conversion subset fixtures are green and `FO-V45-*` obligations are green (or formally logged under non-blocking policy).
