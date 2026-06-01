# Qualified Name and Project Lookup Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_project_symbols.rs`, a binder-owned project symbol table
for qualified name resolution.

The table covers:

- project symbols;
- module symbols;
- class/type symbols;
- public symbols;
- module procedure and field routes;
- class member routes;
- one-, two-, and project-qualified three-part lookup.

Resolution is case-insensitive and uses `SymbolId` routes rather than source-text rewrites.

## Focused Fixtures

The tests verify:

- unqualified module and public symbol lookup;
- module-qualified procedure and field lookup;
- project-qualified lookup rejects mismatched project names;
- class member lookup remains separate from module member lookup.

## Checks

- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo fmt --check -p oxvba-compiler`

## Fresh-Eyes Review

- A bad initial shortcut that accepted any three-part qualified name was corrected. The table now
  stores the folded project name and requires it to match for project-qualified lookup.
- This bead introduces the binder-owned table and tests; it does not delete legacy `project.rs`
  rewrites yet. Later FE-7 beads can migrate constructs to consume these routes one family at a
  time.
- The route type carries `SymbolId` plus project symbol kind, so later lowering can reason about
  module/procedure/field/public distinctions without reparsing text.
