# WORKSET_2026-02-27_STDLIB_DATE_TIME_CORE_V48.md

## Purpose
Execute profile 48 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Date/time intrinsic subset: DateSerial, TimeSerial, DateValue, TimeValue, DateAdd, DateDiff.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-date-time-core-v48
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-date-time-core-v48 -OutputDir docs/evidence/profiles/v48
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
48 closes when scope fixtures are green and FO-V48-* obligations are green (or logged under non-blocking formal policy).
