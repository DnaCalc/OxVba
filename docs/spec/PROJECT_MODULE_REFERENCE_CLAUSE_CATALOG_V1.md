# Project Module Reference Clause Catalog v1

Status: `working-draft`
Date: 2026-03-02
Applies to: planned ProjectGraph integration across `oxvba-compiler` + `oxvba-host`

## 1. Purpose

Stable clause IDs for Project/Module/Reference semantics, with explicit verification mapping.

Machine-readable companion:

- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`

## 2. Status Vocabulary

- `implemented-verified`: implemented and covered by executable checks.
- `implemented-partial`: implemented subset, incomplete coverage.
- `specified-pending`: specified target, not implemented yet.

## 3. Clause Set

| Clause ID | Domain | Clause | Status | Verification Anchor |
|---|---|---|---|---|
| `PMR-GEN-001` | global | Project graph operations must be deterministic for identical manifest input. | implemented-verified | `project_graph_operations_are_deterministic_for_equal_inputs` |
| `PMR-GEN-002` | global | MUST-level source constraints produce explicit diagnostics, never silent fallback. | implemented-partial | `scanner_rejects_withevents_in_standard_modules`; `withevents_in_procedural_module_should_be_rejected`; `compile_implements_directive_in_single_module_subset_succeeds`; `compile_raiseevent_statement_in_single_module_subset_succeeds`; `compile_project_rejects_implements_in_non_class_module`; `compile_project_rejects_raiseevent_in_non_class_module`; `compile_project_rejects_raiseevent_undeclared_event`; `module_unit_rejects_malformed_attribute_line`; `compile_project_rejects_option_private_for_non_procedural_module` |
| `PMR-PROJ-001` | project | Project name must be valid identifier (`CONF-...-0035`). | implemented-verified | `project_graph_rejects_invalid_project_name` |
| `PMR-PROJ-002` | project | Referenced project names in one project must be distinct (`CONF-...-0038`). | implemented-verified | `references_are_precedence_ordered_and_case_insensitive_unique` |
| `PMR-PROJ-003` | project | Reference order is preserved and used as precedence (`SPEC-...-01230`). | implemented-verified | `references_are_precedence_ordered_and_case_insensitive_unique` |
| `PMR-PROJ-004` | project | Project kind model includes Source/Host/Library (`SPEC-...-01234`). | implemented-verified | `ProjectKind` + `project_graph_rejects_duplicate_project_name_case_insensitive` |
| `PMR-PROJ-005` | project | Host project public entities must be visible to source projects as specified (`SPEC-...-01239`). | specified-pending | host project integration tests (planned) |
| `PMR-PROJ-006` | project | Open-host extension behavior is explicit and profile-governed (`SPEC-...-01240/01241`). | specified-pending | HAL-adjacent contract checks (planned) |
| `PMR-MOD-001` | module | Module kinds include at least procedural + class (`CONF-...-0039`). | implemented-partial | `ModuleKind` modeling + project-node validation tests |
| `PMR-MOD-002` | module | Every module in a project has a distinct module name (`CONF-...-0041`). | implemented-verified | `project_node_rejects_duplicate_module_name_case_insensitive` |
| `PMR-MOD-003` | module | Module name source is `VB_Name` attribute (`SPEC-...-01285`). | implemented-verified | `module_unit_uses_vb_name_as_semantic_identity_when_file_name_differs`; `load_basproj_uses_vb_name_as_semantic_identity_while_preserving_include_path` |
| `PMR-MOD-004` | module | Module name max length is 31 (`SPEC-...-01286`). | implemented-verified | `project_node_rejects_module_name_over_31_chars` |
| `PMR-MOD-005` | module | Source-project modules require `VB_GlobalNamespace=False` and `VB_Creatable=False` (`CONF-...-0042`). | implemented-verified | `source_project_class_attribute_constraints_are_enforced` |
| `PMR-MOD-006` | module | Class header supports `VB_PredeclaredId` + `VB_Exposed` attributes (`SPEC-...-01266/01267`). | implemented-verified | `module_unit_parses_header_attributes_and_option_private` |
| `PMR-MOD-007` | module | Extension module name must match extensible module name (`CONF-...-0043`). | implemented | `host_extension_attach_requires_registered_extensible_target`; `extensible_module_registration_is_case_insensitive`; `host_extension_oracle_20260326T144800Z` |
| `PMR-VIS-001` | visibility | `Option Private Module` grammar and semantics are enforced (`SPEC-...-01366..01369`). | implemented-partial | `source_option_private_module_is_project_private`; `option_private_module_hides_referenced_project_export`; `option_private_module_hides_referenced_project_qualified_export`; `option_private_module_allows_same_project_access`; historical CCT-038 host-direct oracle |
| `PMR-VIS-002` | visibility | Colliding public variable names require module qualification (`CONF-...-0053`). | implemented-partial | `public_const_variable_collision_should_be_ambiguous`; `public_const_variable_collision_keeps_module_qualified_access`; `public_const_variable_collision_keeps_project_qualified_access`; `module_name_public_member_collision_should_be_rejected` |
| `PMR-VIS-003` | visibility | Colliding public procedure names require module/project qualification (`CONF-...-0106`). | implemented-partial | `duplicate_public_unqualified_should_be_ambiguous`; `duplicate_public_unqualified_members_are_ambiguous_before_library_fallback`; `module_name_public_member_collision_should_be_rejected` |
| `PMR-VIS-004` | visibility | Private-module members are inaccessible from referencing projects (`SPEC-...-01368`). | implemented-verified | `option_private_module_hides_referenced_project_export`; `option_private_module_hides_referenced_project_qualified_export`; `option_private_module_allows_same_project_access`; `option_private_module_keeps_public_referenced_module_visible` |
| `PMR-NAME-001` | name-resolution | Cross-module collision rules for module-level declarations are enforced (`CONF-...-0131/0132/0136`). | implemented-partial | existing module-local collision tests; `duplicate_public_unqualified_should_be_ambiguous`; `module_name_public_member_collision_should_be_rejected`; `public_const_variable_collision_should_be_ambiguous`; `public_const_variable_collision_keeps_module_qualified_access`; `public_const_variable_collision_keeps_project_qualified_access` |
| `PMR-NAME-002` | name-resolution | Public UDT/Enum naming conflicts across project/module/library spaces are diagnosed (`CONF-...-0078/0079/0083/0084`). | implemented-partial | `public_udt_enum_collision_should_be_ambiguous`; `public_udt_enum_collision_keeps_module_qualified_udt_type`; `public_udt_enum_collision_keeps_project_qualified_udt_type` |
| `PMR-NAME-003` | name-resolution | Project/module/reference namespace precedence is deterministic and documented. | implemented-partial | `cross_module_public_qualified_matches_oracle`; `valid_project_qualifier_should_match_oracle`; `wrong_project_qualifier_should_be_rejected`; `cross_project_fixture_baseline_uses_two_active_modules_and_reference`; `cross_project_module_qualified_reference_call_matches_current_baseline`; `referenced_project_precedence_and_project_qualifier_are_explicit`; `active_project_member_shadows_referenced_project_member`; `wrong_referenced_project_qualifier_should_be_rejected`; `duplicate_referenced_project_global_name_should_be_ambiguous`; `duplicate_referenced_project_global_name_blocks_later_reference_fallback` |
| `PMR-CLS-001` | class | `WithEvents` declarations are prohibited in procedural module declaration lists (`CONF-...-0056`). | implemented-verified | `scanner_rejects_withevents_in_standard_modules`; `withevents_in_procedural_module_should_be_rejected`; `active_project_withevents_source_routes_to_handler` |
| `PMR-CLS-002` | class | Event-handler prefix binding from `WithEvents` variable declarations is enforced (`CONF-...-0140`). | implemented-verified | `active_project_withevents_source_routes_to_handler`; `referenced_project_withevents_source_routes_to_active_project_handler`; `withevents_handler_prefix_mismatch_does_not_route`; `private_referenced_project_withevents_declaration_should_be_rejected`; `private_referenced_project_withevents_source_should_be_rejected` |
| `PMR-CLS-003` | class | Implements directive cannot occur in extension modules (`CONF-...-0095`). | implemented-verified | `compile_project_rejects_implements_in_non_class_module` |
| `PMR-CLS-004` | class | Implemented interface class names in same class module are pairwise distinct (`CONF-...-0096`). | specified-pending | implements diagnostics tests (planned) |
| `PMR-CLS-005` | class | Implements requires implemented-name declarations for all public interface members (`CONF-...-0097/0098`). | implemented-verified | `compile_project_rejects_implements_missing_member_coverage`; `compile_project_class_module_allows_implements_subset_without_gate_diagnostic`; `compile_project_allows_implements_interface_from_referenced_project` |
| `PMR-CLS-006` | class | Implements method name prefix binding is enforced (`CONF-...-0143`). | implemented-partial | `compile_project_rejects_implements_missing_member_coverage` |
| `PMR-CLS-007` | class | `RaiseEvent` statements are valid only inside class modules and declared events (`CONF-...-0176/0177`). | implemented-verified | `compile_project_rejects_raiseevent_in_non_class_module`; `compile_project_rejects_raiseevent_undeclared_event`; `compile_project_allows_raiseevent_for_declared_event` |
| `PMR-CLS-008` | class | Class instancing restrictions for auto-object paths are enforced (`CONF-...-0065`). | implemented-partial | `class_default_instance_flag_is_derived_from_attributes` (metadata subset) |
| `PMR-REF-001` | references | Project references expose public entities from referenced projects (`SPEC-...-01232`). | implemented-partial | `cross_project_fixture_baseline_uses_two_active_modules_and_reference`; `cross_project_module_qualified_reference_call_matches_current_baseline`; `referenced_project_precedence_and_project_qualifier_are_explicit`; `active_project_member_shadows_referenced_project_member`; `wrong_referenced_project_qualifier_should_be_rejected`; `duplicate_referenced_project_global_name_should_be_ambiguous`; `duplicate_referenced_project_global_name_blocks_later_reference_fallback` |
| `PMR-REF-002` | references | Importlib/type-library reference identification rules are explicit and deterministic (`CONF-MS-OAUT-0561`). | implemented-partial | `type_library_resolution_binds_unique_importlib_entry`; `type_library_resolution_requires_importlib_hint`; `type_library_resolution_reports_ambiguous_importlib` |
| `PMR-REF-003` | references | OAUT `GetIDsOfNames` case-insensitive requirement is preserved for reference-bound dispatch (`CONF-MS-OAUT-0599`). | implemented-partial | existing dispatch subset tests |
| `PMR-REF-004` | references | OAUT `Invoke` argument packing and out-parameter obligations are preserved (`CONF-MS-OAUT-0614..0623`). | implemented-partial | deterministic dispatch subset + HAL marshal tests |
| `PMR-COM-001` | class-com | Class runtime semantics remain executable without COM activation (`Class_Initialize`/`Class_Terminate`, property routes). | implemented-verified | `formal_v44_property_*`; `formal_v54_class_*` host tests |
| `PMR-COM-002` | class-com | Class/object COM-boundary execution is currently deterministic subset only; no full parity claim. | implemented-partial | `formal_v55_createobject_dispatch_subset`; dispatch subset conformance fixtures |
| `PMR-OVBA-001` | storage | MS-OVBA section 2 obligations must be mapped into clause IDs before storage parity claims. | specified-pending | Foundation extraction follow-up (planned) |
| `PMR-OVBA-002` | storage | Project/module/reference storage roundtrip is deterministic and loss-aware (header attrs + references + module text). | specified-pending | storage roundtrip test suite (planned) |
| `PMR-HAL-001` | hal-adjacent | Host-project discovery and open-host extension hooks are HAL-profile-governed. | specified-pending | HAL integration conformance lane (planned) |
| `PMR-HAL-002` | hal-adjacent | Reference binding failures from host/HAL are structured and phase-classified (compile/runtime policy aware). | specified-pending | host phase diagnostics tests (planned) |

## 4. Notes

- `CONF-MS-OAUT-*` is shorthand for full IDs in `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`.
- Clauses marked `implemented-partial` refer to currently shipped single-module or deterministic dispatch subsets and do not imply full project-model implementation.

