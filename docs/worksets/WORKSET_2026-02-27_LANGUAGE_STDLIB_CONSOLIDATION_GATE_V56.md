# WORKSET_2026-02-27_LANGUAGE_STDLIB_CONSOLIDATION_GATE_V56.md

## Purpose
Execute profile 56 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Consolidation gate across language + stdlib + host/interop subset with default profile switch to v56.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-language-stdlib-consolidation-gate-v56
./scripts/run-matrix.ps1 -ProfileScope mvp-language-stdlib-consolidation-gate-v56 -OutputDir docs/evidence/profiles/v56
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
56 closes when scope fixtures are green and FO-V56-* obligations are green (or logged under non-blocking formal policy).
