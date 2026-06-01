# Operator Normalization Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_operator_normalization.rs`, a small operator
normalization/optimizer split surface over HIR binary operations.

Uniform HIR binary ops are accepted as the parser/binder shape; constant folding is represented as
a separate optimizer step.

## Checks

- `cargo test -p oxvba-compiler frontend_operator_normalization --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This does not yet delete legacy `AddConst`/`SubConst` paths. It establishes the FE-8 target shape:
  parser/binder produces uniform binary ops; optimizer handles constant transforms.
