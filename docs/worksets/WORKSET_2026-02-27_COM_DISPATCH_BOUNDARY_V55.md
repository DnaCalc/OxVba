# WORKSET_2026-02-27_COM_DISPATCH_BOUNDARY_V55.md

## Purpose
Execute profile 55 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Dispatch-boundary subset: CreateObject and DispatchInvoke intrinsics.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-com-dispatch-boundary-v55
./scripts/run-matrix.ps1 -ProfileScope mvp-com-dispatch-boundary-v55 -OutputDir docs/evidence/profiles/v55
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
55 closes when scope fixtures are green and FO-V55-* obligations are green (or logged under non-blocking formal policy).
