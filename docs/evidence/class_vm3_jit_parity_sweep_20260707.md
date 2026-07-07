# Class VM3/JIT Parity Sweep - 2026-07-07

Bead: `bd-h4oh.10.28`

Scope: accepted project-class execution subset for VM3/JIT parity. COM connection points,
COM export/server hosting, imported COM coclasses, and live Windows COM interop remain out of
scope.

## Sweep Coverage

| Area | Evidence |
| --- | --- |
| Construction and property reads | `jit_project_class_new_property_get_matches_vm3_without_fallback` |
| `As New` locals/fields and `Nothing` reset | `jit_project_dim_as_new_*`, `jit_project_field_as_new_reinstantiates_after_set_nothing_without_fallback` |
| Object identity, `Is`, `Nothing`, unset object errors | `jit_project_live_object_identity_and_set_assignment_match_vm3`, `jit_project_typeof_*`, unset-object rows |
| Method/property dispatch | named, optional, `ParamArray`, ByRef alias, indexed property, and object-valued property rows in `jit_project_objects` |
| Default members | scalar and object-valued default-member Get/Let/Set rows in `jit_project_objects` |
| Predeclared singletons | active and referenced predeclared rows in `jit_project_objects` |
| Lifecycle and termination timing | JIT lifecycle rows in `jit_project_objects`; VM3 lifecycle oracle rows in `class_lifecycle_vm3` |
| Cross-project classes | referenced `New`, property call, predeclared singleton, incompatible assignment diagnostics in `jit_project_objects` |
| Project events / `WithEvents` | VM3/JIT numeric rows in `raiseevent_fanout_vm3` |
| Handle balance / no fallback | differential helpers assert no unsupported diagnostics and zero handle balance where exposed |

## Checks

- `cargo fmt --check`
- `cargo test -p oxvba-jit -- --format terse`
- `cargo test -p oxvba-differential --test jit_project_objects -- --nocapture`
- `cargo test -p oxvba-differential --test raiseevent_fanout_vm3 -- --nocapture`
- `cargo test -p oxvba-differential --test class_lifecycle_vm3 -- --nocapture`

## Residual Routing

No in-scope VM3/JIT class execution gap was found during this sweep.

Terminal status language, unsupported diagnostic wording, and residual handoff remain owned by
`bd-h4oh.10.29`. COM export/server readiness remains deferred to `bd-h4oh.15.1`.
