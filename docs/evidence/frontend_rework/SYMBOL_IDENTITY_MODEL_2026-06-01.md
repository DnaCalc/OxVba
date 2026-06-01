# Symbol Identity Model Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_symbols.rs`, a compiler-owned symbol identity model for
the upcoming HIR binder and SemanticModel work.

The model defines:

- typed IDs: `SymbolId`, `ScopeId`, and `InternedNameId`;
- scoped lookup with parent walking and nearest-scope resolution;
- namespace separation for project, module, library, type, procedure, member, parameter, and
  local symbols;
- case-insensitive interning that preserves the first source spelling and stores a folded key;
- source provenance through module name plus byte-span-style `FrontendSourceSpan`;
- duplicate detection within the same scope/namespace/name, ignoring case.

Lookup and resolution are immutable operations. Unknown names do not mutate the interner, which
keeps SemanticModel query behavior predictable.

Reopened update: the model now has an executable CST-fed collection route:
`build_symbol_model_from_source(module_name, source)`.

That route parses with `oxvba-syntax`, rejects recovered syntax errors before binding, declares a
module symbol/scope, and collects procedure, parameter, local, declare, event, type, and enum
symbols from CST nodes with source spans. This is still the FE-6.1 symbol identity slice, not the
full production binder, but symbol identities are no longer only hand-constructed test data.

## Checks

- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_semantic_model --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The model is intentionally independent of the legacy `resolve.rs` string tables. FE-6.2 can build
  HIR arenas on this without forcing a high-risk resolver rewrite in the same bead.
- Namespace separation is explicit. A module symbol and a library symbol can share a folded name
  without colliding, while duplicates in the same namespace and scope are rejected.
- Source provenance is stored on every symbol now, before diagnostics and SemanticModel queries
  depend on it.
- The new CST collection tests prove case-insensitive lookup, bracketed identifier normalization,
  duplicate detection, and byte-span provenance using real parser output.
- The route intentionally covers declarations needed to seed FE-6.2/FE-6.3. It does not yet bind
  expression references, member/default-member semantics, or production lowering; those remain
  owned by later FE-6/FE-7 beads.
- Non-ASCII/VBA locale-specific case folding is not solved here; the current compiler already
  operates on ASCII-oriented identifier handling. If broader identifier folding becomes required,
  the change is localized to `fold_identifier`.
