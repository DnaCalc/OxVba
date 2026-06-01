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

The production module-aware project lowering route now builds this index during
`compile_project_with_strategy` and passes it into line binding. Module-qualified and
project-qualified procedure invocations can resolve through `ProjectSymbolRoute`/`SymbolId` before
falling back to the older `project.rs` name-resolution logic.

Unqualified active-project public procedure calls now also consume public candidates from
`ProjectSymbolTables`. Public lookup stores candidate routes instead of overwriting duplicate names,
so existing duplicate-name ambiguity behavior is preserved before reference-precedence fallback.

## Focused Fixtures

The tests verify:

- unqualified module and public symbol lookup;
- module-qualified procedure and field lookup;
- project-qualified lookup rejects mismatched project names;
- class member lookup remains separate from module member lookup.
- manifest-backed procedural module lookup, including `Option Private Module` behavior;
- manifest-backed class lookup using attribute names and keyword-like field identifiers such as
  `Name`.
- production resolver route proof for a module-qualified invocation resolved via
  `resolve_invocation_name_from_project_symbols`;
- compile-path coverage that the module-aware strategy still rewrites module-qualified calls.
- ambiguity-preservation coverage for duplicate unqualified active-project procedures;
- reference-precedence coverage for unqualified calls that fall through to referenced projects.

## Checks

- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_route_policy --quiet`
- `cargo test -p oxvba-compiler project_symbol_index_resolves_module_qualified_invocation_route --quiet`
- `cargo test -p oxvba-compiler compile_project_module_aware_rewrites_module_qualified_call_without_parentheses --quiet`
- `cargo test -p oxvba-compiler compile_project_rejects_ambiguous_unqualified_duplicate_procedure_name_subset --quiet`
- `cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_reference_precedence_fixture --quiet`
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

`bd-aprs.8.1` can close with an explicit scoped handoff.

The improved symbol index is now wired into the production module-aware project lowering path for
qualified procedure invocation lookup and unqualified active-project public procedure lookup.

Fresh-eyes review found that module/class field routes are indexed but are not yet the
authoritative production path for member reads/writes. That is not left as loose residual work:
member reads/writes, dispatch shape, property assignment, default members, and class field
construction are the scoped outcomes of FE-7.2, FE-7.3, and FE-7.4. Those beads must consume the
same `ProjectSymbolRoute`/`SymbolId` facts rather than reintroducing text-only lookup.

`project.rs` line lowering remains the compatibility lowering surface even where individual name
routes now come from the front-end table. The route policy therefore remains correct:
`FrontendConstruct::ProjectSemantics` is a tracked residual, not `V2Default`, until the rest of
FE-7 retires the member/property/class rewrite surfaces.

Next concrete implementation step: FE-7.2 should use the FE-7.1 project symbol index as its input
for member dispatch classification.
