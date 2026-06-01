# Production HIR Lowering Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_hir_lowering.rs`, a scoped production HIR lowering path.
For the currently supported HIR surface, source is parsed and bound into typed HIR, lowered from HIR
facts into the current bound module shape, then passed through the existing typecheck, optimizer, and
bytecode/metadata emitter. This means the frontend-v2 syntax bridge now tries real HIR production
lowering before falling back to the older CST/legacy bridge.

The initial production scope is intentionally narrow and explicit:

- procedure declarations,
- local and parameter frame slots with declared scalar/object types,
- `Dim` metadata line projection,
- implicit/explicit `Let` and `Set` assignments,
- literals, names, unary expressions, and binary arithmetic/comparison/logical expressions, and
- typed structural `Null`/`Nothing` literals.

Unsupported constructs are rejected from the HIR production path before lowering and continue through
the tracked fallback path. This prevents silent partial lowering for calls, member/index/new
expressions, control flow, error handling, `ReDim`, `With`, events, declarations, and other surfaces
not yet implemented in HIR production lowering.

## Checks

- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler frontend_diff_v2_smoke_matches_legacy_for_supported_assignment --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This bead does not remove the fallback bridge; FE-9 default-route and audit beads must decide which
  construct families are flipped and which residuals remain tracked.
- Call-site descriptors, object/member bindings, and writebacks remain out of the initial HIR
  production scope. They stay on the tracked fallback route until HIR supports those syntax and
  semantic forms; the production guard rejects those constructs before HIR lowering.
- The first attempt let HIR production lowering silently ignore call statements. The production guard
  now rejects unsupported syntax kinds up front so scoped HIR lowering is not allowed to compile a
  partial program.
- The simple assignment parity check initially exposed metadata drift in assignment intent and
  declaration line numbers; HIR lowering now preserves implicit assignment intent and projects local
  declaration source lines into procedure metadata.
