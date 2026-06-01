# Query Integration Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reworked `crates/oxvba-compiler/src/frontend_query.rs` from a revision-counter scaffold into a
small executable query workspace over the real front-end facts.

The workspace now owns a source/module pair and lazily caches:

- parse summaries from `oxvba_syntax::parse`;
- bind/HIR/type-hook facts from `collect_type_hooks_from_source`;
- assignment/typecheck summaries and diagnostics from typed HIR assignment semantics;
- parser + binder/type diagnostics;
- `SemanticModel` values built from the same bound HIR and diagnostics used by compilation-facing
  front-end checks.

It also tracks per-layer recompute counts. Tests prove that:

- repeated SemanticModel queries reuse cached parse/bind/typecheck/diagnostic/model values;
- source edits invalidate parse and all downstream layers, but recomputation remains lazy until a
  query is requested;
- targeted typecheck invalidation recomputes typecheck/diagnostics while reusing parse and bind;
- parse errors stop binding while still producing parser diagnostics.

This remains an in-repo salsa-shaped query engine rather than an external `salsa` dependency. The
important FE-9.3 closure condition is now executable query/invalidation behavior over the actual
front-end data path; a later dependency migration can wrap this contract instead of replacing
scaffold-only counters.

## Checks

- `cargo test -p oxvba-compiler frontend_query --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo check -p oxvba-compiler --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The previous FE-9.3 artifact was not enough for the reopened workset: it had invalidation
  counters but no cached parse/bind/typecheck/diagnostic/SemanticModel query values. That would
  have allowed a false closure on data-structure-only work.
- The query database now reaches the real `oxvba-syntax` parser, HIR/type-hook builder,
  assignment diagnostics, and SemanticModel indexing path.
- This bead does not claim full IDE/language-service reconciliation. FE-9.4 remains responsible
  for replacing duplicate language-service logic with this shared query surface.
