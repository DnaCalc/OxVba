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
- Imported COM member dispatch now consumes resolved typelib member specs in production early-bound
  member rewriting. The classifier gates each resolved member as `ImportedCom` with the concrete
  dispatch id before rendering `__OxVbaEarlyInvoke`.
- The remaining dispatch categories are explicitly handed to narrower beads that own their
  executable semantics:
  - project/default-member reads and writes: FE-7.3;
  - source-class field/member state: FE-7.4;
  - host-provided globals and external reference roots: FE-7.6;
  - typed dynamic-dispatch intrinsics: FE-8.1.

## Closure Status

`bd-aprs.8.2` can close for dispatch classification after the production route proof above. The
classifier is no longer scaffold-only for the two dispatch classes this bead can safely migrate
without taking over later property/class/external-reference beads: early-bound project procedure
dispatch and imported COM member dispatch.

The narrower downstream beads must use this classification vocabulary and avoid reintroducing
string-only dispatch decisions.

## Continuation Checks

- `cargo test -p oxvba-compiler compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset --quiet`
- `cargo test -p oxvba-compiler known_typelib_default_member_token_and_spec_reads_external_metadata --quiet`
