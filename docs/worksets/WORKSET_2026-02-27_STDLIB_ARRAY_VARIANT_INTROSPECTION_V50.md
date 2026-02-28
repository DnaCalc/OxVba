# WORKSET_2026-02-27_STDLIB_ARRAY_VARIANT_INTROSPECTION_V50.md

## Purpose
Execute profile 50 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Array/variant introspection subset: Array, LBound, UBound, IsArray, VarType, TypeName, IsNumeric, IsDate, IsObject.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-array-variant-introspection-v50
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-array-variant-introspection-v50 -OutputDir docs/evidence/profiles/v50
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
50 closes when scope fixtures are green and FO-V50-* obligations are green (or logged under non-blocking formal policy).
