# Per-Construct Route Policy Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_route_policy.rs`, a per-construct route policy.

Completed front-end construct families default to v2 in the policy. Production bytecode lowering
remains an explicit `LegacyResidual` because the lowering bridge is still staged.

## Checks

- `cargo test -p oxvba-compiler frontend_route_policy --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This avoids a false whole-compiler flip. The policy records the clean shape: completed construct
  families route v2 by default, and fallback is allowed only as named residual scope.
