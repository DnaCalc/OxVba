# Project Module Reference Conformance v1

Status: `working-draft`
Date: 2026-03-02

## 1. Purpose

Define executable verification lanes for Project/Module/Reference semantics and map them to clause IDs in:

- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`

## 2. Suite Layers

1. Parser and header layer (`oxvba-syntax` + compiler front-end)
- module header attribute parsing and validation.
- module kind grammar checks.

2. Project graph and binder layer (`oxvba-compiler`)
- cross-module/project name resolution.
- visibility and qualification diagnostics.
- reference precedence behavior.

3. Host integration layer (`oxvba-host`)
- project manifest loading,
- phase-classified diagnostics,
- host project/reference interaction policy.

4. HAL-adjacent lane (`oxvba-hal` + host)
- host project discovery/injection contracts,
- reference binding provider contracts,
- structured host failure propagation.

5. Oracle differential lane (deferred)
- empirical parity checks against real VBA hosts for high-semantic ambiguity paths.

## 3. Planned Lanes

## Lane A: Static semantics core

Target clauses:

- `PMR-PROJ-001..004`
- `PMR-MOD-001..006`
- `PMR-VIS-001..004`
- `PMR-CLS-001..006`

Expected artifacts:

- `docs/evidence/language/pmr_static_semantics_<timestamp>.md`
- failing fixture list with clause IDs.

Current executable subset (A1/A2):

- `project_graph_rejects_invalid_project_name`
- `project_node_rejects_duplicate_module_name_case_insensitive`
- `source_project_class_attribute_constraints_are_enforced`
- `duplicate_public_unqualified_should_be_ambiguous`
- `duplicate_public_unqualified_members_are_ambiguous_before_library_fallback`
- `module_name_public_member_collision_should_be_rejected`
- `public_const_variable_collision_should_be_ambiguous`
- `public_const_variable_collision_keeps_module_qualified_access`
- `public_const_variable_collision_keeps_project_qualified_access`
- `public_udt_enum_collision_should_be_ambiguous`
- `public_udt_enum_collision_keeps_module_qualified_udt_type`
- `public_udt_enum_collision_keeps_project_qualified_udt_type`
- `source_option_private_module_is_project_private`
- `option_private_module_hides_referenced_project_export`
- `option_private_module_hides_referenced_project_qualified_export`
- `option_private_module_allows_same_project_access`
- `option_private_module_keeps_public_referenced_module_visible`
- `referenced_project_precedence_and_project_qualifier_are_explicit`
- `active_project_member_shadows_referenced_project_member`
- `wrong_referenced_project_qualifier_should_be_rejected`
- `duplicate_referenced_project_global_name_should_be_ambiguous`
- `duplicate_referenced_project_global_name_blocks_later_reference_fallback`

## Lane B: Multi-module resolution

Target clauses:

- `PMR-NAME-001..003`
- `PMR-PROJ-003`

Expected probes:

- two-module and three-module symbol collision + qualification cases,
- reference-precedence shadowing cases,
- private-module accessibility boundaries.

Current executable subset:

- `source_option_private_module_is_project_private`
- `duplicate_public_unqualified_should_be_ambiguous`
- `duplicate_public_unqualified_members_are_ambiguous_before_library_fallback`
- `module_name_public_member_collision_should_be_rejected`
- `public_const_variable_collision_should_be_ambiguous`
- `public_const_variable_collision_keeps_module_qualified_access`
- `public_const_variable_collision_keeps_project_qualified_access`
- `public_udt_enum_collision_should_be_ambiguous`
- `public_udt_enum_collision_keeps_module_qualified_udt_type`
- `public_udt_enum_collision_keeps_project_qualified_udt_type`
- `option_private_module_hides_referenced_project_export`
- `option_private_module_hides_referenced_project_qualified_export`
- `option_private_module_allows_same_project_access`
- `option_private_module_keeps_public_referenced_module_visible`
- `referenced_project_precedence_and_project_qualifier_are_explicit`
- `active_project_member_shadows_referenced_project_member`
- `wrong_referenced_project_qualifier_should_be_rejected`
- `duplicate_referenced_project_global_name_should_be_ambiguous`
- `duplicate_referenced_project_global_name_blocks_later_reference_fallback`

## Lane C: Reference and automation bridge

Target clauses:

- `PMR-REF-001..004`

Expected probes:

- reference graph ordering and target selection,
- importlib hint resolution and deterministic missing/unresolved/ambiguous diagnostics,
- OAUT dispatch packing obligations for reference-backed calls,
- deterministic error routing for bad reference bindings.

Current executable subset (C1):

- `type_library_resolution_binds_unique_importlib_entry`
- `type_library_resolution_requires_importlib_hint`
- `type_library_resolution_reports_ambiguous_importlib`
- `source_option_private_module_is_project_private`
- `option_private_module_hides_referenced_project_export`
- `option_private_module_hides_referenced_project_qualified_export`
- `option_private_module_keeps_public_referenced_module_visible`
- `cross_project_fixture_baseline_uses_two_active_modules_and_reference`
- `cross_project_module_qualified_reference_call_matches_current_baseline`
- `referenced_project_precedence_and_project_qualifier_are_explicit`
- `active_project_member_shadows_referenced_project_member`
- `wrong_referenced_project_qualifier_should_be_rejected`
- `duplicate_referenced_project_global_name_should_be_ambiguous`
- `duplicate_referenced_project_global_name_blocks_later_reference_fallback`

## Lane D: Class/event integration

Target clauses:

- `PMR-CLS-002`, `PMR-CLS-007`, `PMR-CLS-008`

Expected probes:

- `WithEvents` prefix binding,
- `RaiseEvent` legality,
- class-instancing constraints at project boundary.

Current executable subset (A2):

- project-aware legality and coverage diagnostics:
  - canonical list is generated from `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`:
    - `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`
  - covered by compiler tests such as:
    - `compile_project_rejects_withevents_in_procedural_module`
    - `compile_project_rejects_implements_missing_member_coverage`
    - `compile_project_rejects_raiseevent_undeclared_event`
- vm3 scoping/event-source checks:
  - `active_project_withevents_source_routes_to_handler`
  - `referenced_project_withevents_source_routes_to_active_project_handler`
  - `withevents_handler_prefix_mismatch_does_not_route`
  - `withevents_in_procedural_module_should_be_rejected`
  - `private_referenced_project_withevents_declaration_should_be_rejected`
  - `private_referenced_project_withevents_source_should_be_rejected`
- non-interop class runtime checks via host tests:
  - `formal_v44_property_let_routes_assignment_byref`
  - `formal_v44_property_set_routes_assignment_byref`
  - `formal_v54_class_initialize_runs_before_main`
  - `formal_v54_class_terminate_runs_after_main`

Remaining deferred portion:
- runtime class-event dispatch ordering and subscription graph semantics beyond
  the handler-prefix/source-visibility subset (`WithEvents` reassignment
  ordering and broader `RaiseEvent` subscriber dispatch).

## Lane E: Storage and roundtrip

Target clauses:

- `PMR-OVBA-001`, `PMR-OVBA-002`

Expected probes:

- ingest and emit of project metadata,
- module header attribute preservation,
- reference list ordering preservation.

Note: blocked on MS-OVBA section-level extraction quality improvements.

## Lane F: HAL-adjacent integration

Target clauses:

- `PMR-HAL-001`, `PMR-HAL-002`

Expected probes:

- host project catalog provider behavior,
- reference provider failure taxonomy,
- compile-time/runtime phase routing under host policy modes.

## 4. Source Mapping Discipline

Each new PMR fixture must record:

- clause ID(s),
- Foundation source anchor(s),
- expected deterministic result,
- whether oracle evidence is required for closure.

## 5. Deferred Oracle Policy

High-semantic PMR areas should be tracked in:

- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

No PMR parity claim should be promoted to fully compatible without either:

- direct canonical MUST-level source closure, or
- oracle foldback evidence for implementation-defined behaviors.

## 6. Class/COM Alignment Staging

Class-module compatibility work should follow:

- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`

Execution stance:

- semantic class behavior (`Initialize`/`Terminate`, `Property Get/Let/Set`, `Implements`, `WithEvents`) is near-term required,
- full COM ABI and rich automation parity remains explicitly deferred and tracked through existing deferred-oracle gates.

## 7. Immediate Command Skeleton

Initial PMR lane command placeholders:

```powershell
cargo test -p oxvba-compiler pmr_
cargo test -p oxvba-host pmr_
cargo test -p oxvba-compiler compile_project_rejects_withevents_in_procedural_module
cargo test -p oxvba-compiler compile_project_rejects_implements_in_non_class_module
cargo test -p oxvba-compiler compile_project_rejects_raiseevent_undeclared_event
```

And evidence collation via existing profile gate scaffolding once PMR tests land.
