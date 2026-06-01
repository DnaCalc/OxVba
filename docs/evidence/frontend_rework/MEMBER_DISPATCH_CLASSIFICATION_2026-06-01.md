# Member Dispatch Classification Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_member_dispatch.rs`, a typed dispatch classification
surface for FE-7 migration. The 2026-06-01 continuation wired the early-bound project-member
classification into the production FE-7.1 project-symbol route used by `ModuleAwareBindPlan`.

The classifier distinguishes:

- early-bound project members from binder-owned project routes;
- imported COM members with optional dispatch ID;
- late-bound dispatch;
- default-member dispatch;
- host-provided globals.

## Checks

- `cargo test -p oxvba-compiler frontend_member_dispatch --quiet`
- `cargo test -p oxvba-compiler project_symbol_index_resolves_module_qualified_invocation_route --quiet`
- `cargo test -p oxvba-compiler compile_project_rejects_ambiguous_unqualified_duplicate_procedure_name_subset --quiet`
- `cargo fmt --check -p oxvba-compiler`

## Fresh-Eyes Review

- The first run established the dispatch classification vocabulary and tests, but that was
  scaffold-only and not enough for the reopened production gate.
- Early-bound project procedure dispatch now consumes `ProjectSymbolRoute` from FE-7.1 in
  production qualified/public procedure lookup. The classifier gates the route as
  `EarlyBoundProject { kind: Procedure }` before the lowering path maps the symbol route to the
  lowered procedure.
- This bead is not complete yet. Imported COM members, late-bound dispatch, default-member
  dispatch, and host-provided globals still need production route proof through this classifier or
  explicit handoff to narrower FE-7 beads before closure.
