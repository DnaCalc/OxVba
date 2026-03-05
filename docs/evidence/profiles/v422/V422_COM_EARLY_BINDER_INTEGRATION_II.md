# V422 COM Early Binding - Binder integration II

## Scope
- Ladder: v407..v466
- Step: v422
- Workset: WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md

## Step Outcome
- Compiler rewrite path now lowers supported early-bound member invocations to DispatchInvoke token calls and emits explicit unsupported diagnostics for out-of-subset members/arity.

## Primary Artifacts
- crates/oxvba-compiler/src/project.rs
- crates/oxvba-compiler/src/project.rs (test: compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset)

## Gate Signal
- v422 implementation objectives are captured and cross-linked.
