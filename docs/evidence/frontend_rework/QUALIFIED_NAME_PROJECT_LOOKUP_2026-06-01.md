# Qualified Name and Project Lookup Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_project_symbols.rs`, a binder-owned project symbol table
for qualified name resolution. The 2026-06-01 continuation upgraded this from a hand-seeded table
to a manifest-backed project index built from real `ProjectManifest` module units.

The table covers:

- project symbols;
- module symbols;
- class/type symbols;
- public symbols;
- module procedure and field routes;
- class member routes;
- one-, two-, and project-qualified three-part lookup.

Resolution is case-insensitive and uses `SymbolId` routes rather than source-text rewrites.

The manifest index now records:

- `VB_Name` module/class names instead of assuming the storage filename is authoritative;
- procedural public procedure routes for unqualified lookup;
- `Option Private Module` exclusion from unqualified public lookup;
- class/document/form type routes;
- class field and property routes from CST-collected module-scope declarations.

## Focused Fixtures

The tests verify:

- unqualified module and public symbol lookup;
- module-qualified procedure and field lookup;
- project-qualified lookup rejects mismatched project names;
- class member lookup remains separate from module member lookup.
- manifest-backed procedural module lookup, including `Option Private Module` behavior;
- manifest-backed class lookup using attribute names and keyword-like field identifiers such as
  `Name`.

## Checks

- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_route_policy --quiet`
- `cargo fmt --check -p oxvba-compiler`

## Fresh-Eyes Review

- A bad initial shortcut that accepted any three-part qualified name was corrected. The table now
  stores the folded project name and requires it to match for project-qualified lookup.
- A second gap was found during the continuation: class field declarations such as
  `Public Name As String` were not collected because declaration-name extraction only accepted
  plain identifier tokens. The collector now accepts visibility-prefixed field declarations and
  keyword-like declaration names in declaration nodes.
- The route type carries `SymbolId` plus project symbol kind, so later lowering can reason about
  module/procedure/field/public distinctions without reparsing text.

## Current Closure Status

`bd-aprs.8.1` should remain open after this evidence update.

The improved symbol index is real binder infrastructure, but fresh-eyes review confirmed that the
production project lowering path still uses `project.rs` procedure metadata and line lowering as
the source of truth for qualified invocation rewriting. The route policy was corrected so
`FrontendConstruct::ProjectSemantics` is now a tracked residual rather than claimed as `V2Default`.

Next concrete implementation step: wire `build_project_symbol_index_from_manifest` into the
module-aware project lowering/binding path, and make qualified procedure/module/class/field
resolution consume `ProjectSymbolRoute`/`SymbolId` facts. Only then can this bead close under the
reopened production-migration criterion.
