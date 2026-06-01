# Metadata Normalization Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reopened production-route update: the FE-5 diff harness metadata projection is now used to produce
field-level semantic metadata drift in `FrontendDiffReport`, not just an opaque
`metadata summary differs` boolean.

Stable projection fields include procedure identity, line/PC source maps, slot metadata, signature,
call sites, array/type/layout facts, value states, expression/operator semantics, coercions,
name bindings, object member bindings, and diagnostic projections via the FE-6.5 mapper.

The classifier now carries these field-level paths into bug/intentional-drift reasons, so a metadata
regression can point at stable semantic fields such as `procedures.main.return_slot` even when
bytecode layout or instruction shape drifts. It also has an explicit metadata-improvement policy for
documented cases where HIR source maps are more faithful than the legacy projection.

## Checks

- `cargo test -p oxvba-compiler frontend_diff_metadata_projection --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The projection is semantic, not byte-identical. It is intentionally suitable for classifying
  harmless bytecode drift while still catching metadata contract drift.
- This pass converts the projection from isolated coverage into an executable harness surface:
  `compare_legacy_to_frontend_v2` now returns stable metadata difference paths through
  `FrontendDiffReport::metadata_differences`.
- Source maps are represented through statement line numbers and entry PCs; FE-9.4 can extend this
  when language-service source maps move onto shared SemanticModel facts.
- Metadata improvements require fixture policy. Undocumented metadata drift still classifies as a
  bug; the call/coercion source-map row is accepted only because bytecode, diagnostics, call
  descriptors, and non-source-map metadata match.
