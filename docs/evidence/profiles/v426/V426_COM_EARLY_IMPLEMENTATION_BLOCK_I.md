# V426 COM Early Binding - Design-to-code gate

## Scope
- Ladder: v407..v466
- Step: v426
- Workset: WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md

## Implementation closure summary
- PMR type-library references support deterministic identity hints (`importlib/libid/version/lcid`) and bind outcomes for unresolved/ambiguous identities.
- HAL exposes type-library resolve/load/invalidate surface through `TypeLibraryHal` with deterministic Windows subset behavior and deterministic unsupported floor elsewhere.
- Compiler project lowering supports constrained early-bound bridge:
  - typed external declaration rewriting,
  - `As New` selector initialization for known deterministic subset,
  - member-call lowering to deterministic `DispatchInvoke` token lanes.
- Runtime lane for this tranche intentionally reuses stable late-bound COM transport (`CreateObject` + `DispatchInvoke`) to preserve deterministic behavior.

## Verification evidence
- `cargo test -p oxvba-hal -p oxvba-host -p oxvba-compiler` -> PASS
- `./scripts/meta-check.ps1 -Fast` -> PASS

## Primary artifacts
- docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md
- docs/profile-status/PROFILE_STATUS_V417.md .. PROFILE_STATUS_V426.md
- crates/oxvba-compiler/src/project.rs
- crates/oxvba-host/src/project.rs
- crates/oxvba-hal/src/traits.rs
- crates/oxvba-hal/src/adapters/standard.rs

## Gate signal
- v426 implementation objectives are captured, implemented, and validated.
