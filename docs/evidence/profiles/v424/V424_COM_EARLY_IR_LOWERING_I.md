# V424 COM Early Binding - IR lowering I

## Scope
- Ladder: v407..v466
- Step: v424
- Workset: WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md

## Step Outcome
- Early-bound subset routes through existing CreateObject/DispatchInvoke IR/VM contracts, preserving module-aware and rewrite-bridge parity for the new fixture lane.

## Primary Artifacts
- crates/oxvba-compiler/src/project.rs (test: compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture)
- crates/oxvba-host/tests/com_client_end_to_end.rs

## Gate Signal
- v424 implementation objectives are captured and cross-linked.
