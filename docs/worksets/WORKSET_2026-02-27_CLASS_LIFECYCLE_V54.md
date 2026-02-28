# WORKSET_2026-02-27_CLASS_LIFECYCLE_V54.md

## Purpose
Execute profile 54 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Class lifecycle subset: Class_Initialize and Class_Terminate entry wiring.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-class-lifecycle-v54
./scripts/run-matrix.ps1 -ProfileScope mvp-class-lifecycle-v54 -OutputDir docs/evidence/profiles/v54
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
54 closes when scope fixtures are green and FO-V54-* obligations are green (or logged under non-blocking formal policy).
