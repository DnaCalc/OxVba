# WORKSET_2026-02-27_STDLIB_HOST_SENSITIVE_V52.md

## Purpose
Execute profile 52 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Host-sensitive subset: Shell, Environ, Dir with deterministic fallback semantics.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-host-sensitive-v52
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-host-sensitive-v52 -OutputDir docs/evidence/profiles/v52
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
52 closes when scope fixtures are green and FO-V52-* obligations are green (or logged under non-blocking formal policy).
