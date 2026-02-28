# WORKSET_2026-02-27_PROPERTY_PROCEDURES_V44.md

## Purpose
Execute profile `v44` (`mvp-lang-property-procedures-v44`) for property procedure subset support.

## Scope
- Parse top-level `Property Get`, `Property Let`, and `Property Set` procedures.
- Route assignment-form statements (`Name = expr`) to matching property `Let/Set` procedures when `Name` is not a declared variable.
- Keep lowering in existing call model via canonicalized procedure names.

## Implementation Notes
- Property procedures are internally lowered to canonical names (`property_get_<name>`, `property_let_<name>`, `property_set_<name>`).
- Routing is intentionally declaration-sensitive: local declared variables still use normal assignment semantics.
- `Property Get` is parse/dispatch-baseline only in this profile; expression-level property reads remain deferred.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-property-procedures-v44
./scripts/run-matrix.ps1 -ProfileScope mvp-lang-property-procedures-v44 -OutputDir docs/evidence/profiles/v44
```

## Completion Signal
`v44` closes when property subset fixtures are green and `FO-V44-*` obligations are green (or formally logged under non-blocking policy).
