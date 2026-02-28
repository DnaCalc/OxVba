# WORKSET_2026-02-27_STDLIB_ERROR_SURFACE_V51.md

## Purpose
Execute profile 51 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Error-surface subset: Err.Raise statement form and CVErr conversion function.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-error-surface-v51
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-error-surface-v51 -OutputDir docs/evidence/profiles/v51
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
51 closes when scope fixtures are green and FO-V51-* obligations are green (or logged under non-blocking formal policy).
