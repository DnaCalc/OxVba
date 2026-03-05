# V423 COM Early Binding - As New integration

## Scope
- Ladder: v407..v466
- Step: v423
- Workset: WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md

## Step Outcome
- As New external declarations lower to deterministic CreateObject selector initialization for known early-bindable types.

## Primary Artifacts
- crates/oxvba-compiler/src/project.rs
- crates/oxvba-compiler/src/project.rs (test: compile_project_rewrites_as_new_external_type_to_createobject_selector)

## Gate Signal
- v423 implementation objectives are captured and cross-linked.
