# PMR Module-Aware Bind Execution (v287) - 2026-03-03

Status: `completed` for the bind-migration tranche in this workset.

## Scope Executed
- `PMR-FUP-001`: moved `compile_project` active path from rewrite-bridge-only flow to module-aware bind-plan lowering (with bridge fallback retained).
- `PMR-FUP-003` setup from prior step remains in place (formal lane scaffolding + obligations/deferred gate).
- `PMR-FUP-002` remains deferred by policy.

## Code Changes

File: `crates/oxvba-compiler/src/project.rs`

1. Added strategy-based lowering selection:
- `ProjectLoweringStrategy::{ModuleAwareBindPlan, RewriteBridge}`
- `compile_project(...)` now routes through:
  - `compile_project_with_strategy(...)`
  - `selected_project_lowering_strategy()`
- env override:
  - default: `ModuleAwareBindPlan`
  - fallback: `OXVBA_PMR_LOWERING=rewrite-bridge`

2. Added module-aware lowerer:
- `lower_project_source(...)`
- `lower_module_source(...)`
- `lower_module_source_module_aware(...)`
- `build_line_bind_plan(...)`

3. Added explicit bind-plan helpers:
- `InvocationBinding`
- `LineBindPlan`
- `bind_invocation_targets(...)`
- `apply_invocation_bindings(...)`
- `rewrite_call_statement_target_if_present(...)`
- `call_statement_name_span(...)`

4. Kept rewrite bridge intact as explicit fallback:
- `rewrite_module_source(...)` still available.
- `rewrite_invocation_targets(...)` now uses shared bind helpers and supports bare `Call <target>` rewrites too.

## Behavioral Outcomes

1. `compile_project` primary path is now bind-plan based and deterministic.
2. Bridge path remains available for controlled fallback and parity checks.
3. Module-qualified bare `Call` forms (no parentheses) are now lowered in PMR project compile path.

## New/Updated Tests

File: `crates/oxvba-compiler/src/project.rs`

- `compile_project_module_aware_matches_rewrite_bridge_for_shared_fixture`
- `compile_project_module_aware_rewrites_module_qualified_call_without_parentheses`

## Validation

- `cargo fmt --all` -> pass
- `cargo test -p oxvba-compiler project::tests:: -- --nocapture` -> pass
- `./scripts/meta-check.ps1 -Fast` -> pass

## Deferred

- `PMR-FUP-002` (typelib/importlib HAL-backed resolver parity + COM/oracle foldback, `CCT-043` / `ODG-041`) is unchanged and intentionally deferred.
