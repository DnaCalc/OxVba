# PMR Project-Model Fixture Matrix v1

Date: 2026-03-03
Scope: Workset `P9` fixture requirements

## Fixture Mapping

| Required Scenario | Executable Fixture Anchor | Lane |
|---|---|---|
| Duplicate module names | `project::tests::compile_project_rejects_duplicate_module_names` | compiler unit |
| Cross-module public call resolution | `engine::tests::formal_pmr_project_manifest_cross_module_call_executes` | host execution |
| Module-qualified resolution | `project::tests::compile_project_rewrites_module_qualified_calls_for_unique_names` | compiler unit |
| Project-qualified resolution (same project) | `project::tests::compile_project_rewrites_same_project_qualified_call` | compiler unit |
| Public Const/Public variable collision ambiguity and qualified access | `scoping_visibility_vm3::public_const_variable_collision_should_be_ambiguous`; `scoping_visibility_vm3::public_const_variable_collision_keeps_module_qualified_access`; `scoping_visibility_vm3::public_const_variable_collision_keeps_project_qualified_access`; oracle row `SCOPING-CONST-VAR-COLLISION` in `vm3_scoping_followup_oracle_20260701T1655Z` | vm3 differential + Excel oracle |
| Host direct invocation with `Option Private Module` | `engine::tests::formal_pmr_project_manifest_option_private_module_preserves_host_export_entry` | host execution |
| Visibility/module-kind legality (`Option Private Module`) | `project::tests::compile_project_rejects_option_private_for_non_procedural_module` | compiler unit |
| Reference-order shadowing | `project::tests::active_project_resolution_uses_reference_precedence_order_for_shadowing` | host project-graph unit |
| Host export enumeration eligibility | `project::tests::compile_project_exports_public_procedures_including_option_private_modules_for_host_calls`; `project::tests::host_export_registry_exposes_public_procedural_entries` | compiler + host unit |

## Notes

- These fixtures are executable and deterministic in local CI lanes (`cargo test`).
- Host-import header tolerance/rejection expectations are tracked in:
  - `docs/evidence/conformance/PMR_HOST_IMPORT_TOLERANCE_MATRIX_V1.md`
- Excel/VBA parity for `CCT-037..CCT-041` has oracle evidence captured (`pmr_project_model_20260303T070427Z`):
  - `CCT-037..CCT-039` matched and closed.
  - `CCT-040` original baseline divergence is closed locally after project-aware Implements compile/runtime closure; refreshed multi-interface oracle edge capture remains queued in `ODG-038`.
  - `CCT-041` remains open as recorded divergence (`DIV-0004`) for true instance-level reassignment/subscription semantics.
