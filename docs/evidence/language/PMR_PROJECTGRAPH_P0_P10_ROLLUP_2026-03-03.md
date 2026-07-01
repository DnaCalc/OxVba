# PMR ProjectGraph Parser+Binder Integration Rollup (P0-P10)

Date: 2026-03-03
Workset: `docs/worksets/WORKSET_2026-03-03_PROJECTGRAPH_PARSER_BINDER_INTEGRATION_MASTER.md`
Status: completed through `P10` (with explicit deferred-oracle gates for host parity)

## Scope Outcome

Implemented a deterministic ProjectGraph compile path in current executable subset, preserving existing single-source API while adding project/module/reference modeling, PMR diagnostics, and host export registry surfaces.

## Phase Coverage Summary

| Phase | Outcome | Primary Evidence |
|---|---|---|
| `P0` | Baseline PMR clauses/requirements/workset anchored and parseable | PMR clause catalog + requirements CSV in repo |
| `P1` | Module header retention with deterministic diagnostics (`VB_*`, malformed attribute lines) | `module_unit_parses_header_attributes_and_option_private`; `module_unit_rejects_malformed_attribute_line` |
| `P2` | `ProjectManifest` + `compile_project(...)` entrypoint added; single-source `compile(...)` unchanged | `crates/oxvba-compiler/src/project.rs`; compiler project tests |
| `P3` | Project/module/reference identity checks and stable PMR codes | `compile_project_rejects_duplicate_module_names`; `compile_project_rejects_duplicate_reference_targets` |
| `P4` | Qualified name handling in executable subset (module-qualified + same-project-qualified) | Historical compiler subset refreshed by `scoping_visibility_vm3::cross_module_public_qualified_matches_oracle`, `scoping_visibility_vm3::valid_project_qualifier_should_match_oracle`, and `scoping_visibility_vm3::wrong_project_qualifier_should_be_rejected` in `bd-4ktq.38.3` |
| `P5` | `Option Private Module` module-kind legality + host-direct invocation export behavior | `compile_project_rejects_option_private_for_non_procedural_module`; `formal_pmr_project_manifest_option_private_module_preserves_host_export_entry` |
| `P6` | Class-related PMR diagnostics remain explicit, stable, and non-silent | `project_model_*_requires_class_graph.bas`; PMR diagnostics in resolver |
| `P7` | Host export registry for public procedural members | `compile_project_exports_public_procedures_including_option_private_modules_for_host_calls`; host `register_host_export` tests |
| `P8` | Reference-order shadowing modeled in ProjectGraph resolution subset | Historical CCT-037 oracle capture in `pmr_project_model_20260303T070427Z`; refreshed by `scoping_visibility_vm3::referenced_project_precedence_and_project_qualifier_are_explicit` in `bd-4ktq.38.2` |
| `P9` | Conformance/evidence synchronization and PMR coverage status updates | updated PMR clause catalog + requirements CSV + coverage index |
| `P10` | Oracle templates + deferred gate foldback notes for `CCT-037..CCT-041` | `docs/evidence/conformance/PMR_PROJECT_MODEL_ORACLE_TEMPLATES_V1.md`; updated `ODG-035..ODG-039` notes |

## Implemented Subset Boundaries

- Cross-project reference execution in compiler backend remains staged; explicit diagnostic: `PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED`.
- Project-qualified resolution currently supports current-project qualification in compile path; full host/project catalog parity remains deferred.
- Class/interface/event deep semantics (`WithEvents`, `Implements`, `RaiseEvent`) continue under stable PMR gates until class graph integration lands.

## Oracle Gate State

- `ODG-035..ODG-039` are now closed with oracle foldback evidence.
- `ODG-035..ODG-037`: matched and closed.
- `ODG-038..ODG-039`: closed with recorded divergences (`DIV-0003`, `DIV-0004`) and explicit follow-up queue entries.

## Validation Commands

Executed in this run:

```powershell
cargo test -p oxvba-compiler project::tests:: -- --nocapture
cargo test -p oxvba-host project::tests:: -- --nocapture
cargo test -p oxvba-host formal_pmr_project_manifest -- --nocapture
```

All passed after PMR guardrail and test-fixture updates.
