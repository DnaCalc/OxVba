# External References Binding Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_external_references.rs`, a descriptor-backed external
reference binding surface for typelib, project, and native/host-injected references.

The model records reference symbols, descriptor identities, member bindings, and stable unresolved
reference diagnostics.

## Checks

- `cargo test -p oxvba-compiler frontend_external_references --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Reference kind is normalized into one binder-owned route model instead of dependency-specific
  routing branches.
- Descriptor identity is carried with the symbol route, so later lowering can consume package-owned
  descriptor facts.
- This bead supplies the route contract; imported-reference execution fixtures still need the
  higher-level corpus runner path to exercise runtime behavior.
