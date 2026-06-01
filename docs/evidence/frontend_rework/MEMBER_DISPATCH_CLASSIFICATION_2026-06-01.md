# Member Dispatch Classification Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_member_dispatch.rs`, a typed dispatch classification
surface for FE-7 migration.

The classifier distinguishes:

- early-bound project members from binder-owned project routes;
- imported COM members with optional dispatch ID;
- late-bound dispatch;
- default-member dispatch;
- host-provided globals.

## Checks

- `cargo test -p oxvba-compiler frontend_member_dispatch --quiet`
- `cargo fmt --check -p oxvba-compiler`

## Fresh-Eyes Review

- This bead establishes the dispatch classification vocabulary and tests. It does not yet execute
  every route through the VM; those execution fixtures remain part of the later construct migration
  and corpus-runner expansion.
- The early-bound project path consumes `ProjectSymbolRoute` from FE-7.1, so member dispatch is now
  connected to binder-owned symbol tables rather than string rewrite discovery.
