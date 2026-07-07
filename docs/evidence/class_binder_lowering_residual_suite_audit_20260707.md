# Class Binder/Lowering Residual Suite Audit - 2026-07-07

Bead: `bd-h4oh.10.27`

Scope: source-to-Core/OxIR class semantics for the accepted project-class execution subset.
Imported COM activation, COM export packaging, and Windows COM interop are explicitly out of
scope.

## Coverage Matrix

| Requirement | Evidence |
| --- | --- |
| Hidden ByVal `Me` on class members and lifecycle hooks | OxIR verifier hidden-`Me` tests; member/lifecycle dispatch rows in `jit_project_objects` |
| Instance field access | `FieldGet`/`FieldSet` lowering and verifier tests; `jit_project_class_terminate_field_release_cascades_to_child` |
| Explicit `New` and cross-project construction | `jit_project_class_new_property_get_matches_vm3_without_fallback`, `jit_referenced_project_new_class_property_call_matches_vm3_without_fallback`, `cross_project` binder rows |
| Lazy `As New` locals and fields after `Nothing` | `jit_project_dim_as_new_*`, `jit_project_field_as_new_reinstantiates_after_set_nothing_without_fallback`, `dim_as_new_vm3` VM3 oracle rows |
| Predeclared class singleton construction/reset | `predeclared_instance_*`, `cross_project_predeclared_*`, `jit_predeclared_*` |
| Property Get/Let/Set grouping and accessor-kind preservation | `bind_roundtrip`, `cross_project_property_let_does_not_fallback_to_getter`, OxIR verifier accessor-kind tests |
| Default-member value contexts vs `Set` reference contexts | `cross_project_default_member_bare_let_get_preserves_object_reference`, `jit_project_*default_member*` |
| Object-valued properties and `Property Set` | `cross_project_default_member_property_set_assigned_object_is_byval_even_when_declared_byref`, `jit_project_object_default_member_property_set_matches_vm3_without_fallback` |
| Indexed property value-last rules, including named indexes | `cross_project_default_member_property_let_assigned_value_is_byval_even_when_declared_byref`, `cross_project_named_property_set_reorders_index_args_before_value` |
| Invalid Let/Set/member combinations | `cross_project_property_let_does_not_fallback_to_getter`, OxIR verifier `BadClassPropertySetter*`, bind diagnostics in `feature_coverage` |
| Project event class lowering interactions | `withevents_raise_event_routes_to_handler`, `raise_event_*` binder rows, `raiseevent_fanout_vm3` VM3/JIT rows |

## Residual Routing

No new binder/OxIR lowering gap was found for the current accepted project-class subset. The next
delivery bead, `bd-h4oh.10.28`, owns the execution parity sweep and will split any behavioral gap
that appears under VM3/JIT differential execution. `bd-h4oh.10.29` owns terminal docs and
unsupported diagnostics. COM-export/server work remains deferred to `bd-h4oh.15.1`.
