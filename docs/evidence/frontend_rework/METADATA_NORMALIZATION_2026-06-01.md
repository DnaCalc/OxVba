# Metadata Normalization Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

The FE-5 diff harness metadata projection now has an explicit FE-8.4 coverage test:
`frontend_diff_metadata_projection_exposes_stable_descriptor_fields`.

Stable projection fields include procedure identity, line/PC source maps, slot metadata, signature,
call sites, array/type/layout facts, value states, expression/operator semantics, coercions,
name bindings, object member bindings, and diagnostic projections via the FE-6.5 mapper.

## Checks

- `cargo test -p oxvba-compiler frontend_diff_metadata_projection --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The projection is semantic, not byte-identical. It is intentionally suitable for classifying
  harmless bytecode drift while still catching metadata contract drift.
- Source maps are represented through statement line numbers and entry PCs; FE-9.4 can extend this
  when language-service source maps move onto shared SemanticModel facts.
