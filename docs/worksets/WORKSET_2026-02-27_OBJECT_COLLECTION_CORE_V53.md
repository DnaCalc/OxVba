# WORKSET_2026-02-27_OBJECT_COLLECTION_CORE_V53.md

## Purpose
Execute profile 53 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Collection-core subset: CollectionAdd, CollectionItem, CollectionRemove, CollectionCount.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-object-collection-core-v53
./scripts/run-matrix.ps1 -ProfileScope mvp-object-collection-core-v53 -OutputDir docs/evidence/profiles/v53
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
53 closes when scope fixtures are green and FO-V53-* obligations are green (or logged under non-blocking formal policy).
