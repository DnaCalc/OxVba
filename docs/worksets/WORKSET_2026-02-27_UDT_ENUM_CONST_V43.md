# WORKSET_2026-02-27_UDT_ENUM_CONST_V43.md

## Purpose
Execute profile `v43` (`mvp-lang-udt-enum-const-v43`) for module declaration semantics.

## Scope
- Parse and materialize module-level `Const <name> = <int>` declarations.
- Parse `Enum ... End Enum` and bind member constants (explicit + implicit value progression).
- Accept `Type ... End Type` declaration blocks in module parse flow (baseline subset).
- Keep runtime model slot-based; constants are lowered as deterministic procedure prelude assignments.

## Implementation Notes
- Constant lowering is deterministic (sorted by name) to keep slot ordering stable.
- This profile intentionally scopes UDT support to declaration-level acceptance; field storage/access is deferred.
- Call parsing now rejects trailing tokens after `)` to avoid misclassifying failed assignments as calls.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-udt-enum-const-v43
```

## Completion Signal
`v43` closes when new conformance fixtures are green and `FO-V43-*` obligations are green (or formally logged under non-blocking policy).
