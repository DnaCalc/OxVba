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
- explicit `ByVal` / `ByRef` parameter mechanism projection for lowered procedures,
- `Dim` metadata line projection,
- implicit/explicit `Let` and `Set` assignments,
- simple multiline `If ... Then ... End If` statements without `ElseIf`,
- literals, names, unary expressions, and binary arithmetic/comparison/logical expressions, and
- typed structural `Null`/`Nothing` literals,
- same-module procedure call statements whose targets bind to procedure symbols and whose arguments
  lower through the supported expression surface.

Unsupported constructs are rejected from the HIR production path before lowering and continue through
the tracked fallback path. This prevents silent partial lowering for member/index/new expressions,
unsupported control flow, error handling, `ReDim`, `With`, events, declarations, and other surfaces
not yet implemented in HIR production lowering.

## Reopened Continuation

The second FE-8.5 slice removes the procedure-call syntax residual that the route audit exposed
after the hidden CST bridge fallback was removed:

- `CallStmt` lowers into a HIR expression statement instead of falling through recursive statement
  collection.
- `CallExpr` and parser-shaped `IndexExpr` call forms lower into `HirExprKind::Call`.
- HIR production lowering emits `BoundStmt::Call` for same-module procedure targets.
- Lowered procedure parameters now preserve explicit `ByVal` / `ByRef` source mechanisms, so call
  descriptors no longer report `ByVal` parameters as omitted/default `ByRef`.

The call/coercion seed row no longer exposes a bytecode or call-descriptor bug after this slice.
`conformance/tests/call_coercion_mixed_variant_to_long.bas` now matches diagnostics, bytecode, call
site metadata, and coercion descriptors. The only remaining delta is source-map metadata for the
second procedure: HIR reports the actual `Sub Use` line after the blank line, while the legacy
projection maps the procedure one line early. The diff classifier now records that as a documented
metadata improvement instead of a bug.

This is still not blanket FE-8.5 closure. Broader HIR production lowering remains open for language
surfaces outside this simple same-module call subset, especially optional/default arguments,
ParamArray, member/index dispatch, control flow, and project/class paths owned jointly with FE-7.

## Control-Flow Continuation

The third FE-8.5 slice removes the simplest control-flow route residual:

- multiline `IfStmt` nodes lower into `HirStmtKind::If` with CST-backed condition, then-body, and
  else-body fields;
- production HIR lowering converts that HIR statement into `BoundStmt::IfCond`;
- HIR production bytecode emission now reaches `Instruction::JumpIfZero` for the simple
  `If x = 0 Then ... End If` fixture; and
- the route audit classifies the simple If fixture as `HirProduction`.

This is intentionally not full control-flow closure. `ElseIf`, `Do`/`Loop`, `For`, `For Each`,
`While`, `Select Case`, labels, `GoTo`/`GoSub`, and error-control constructs remain tracked FE-8.5
residuals until each has HIR shape, lowering tests, bytecode/metadata parity or documented
improvement classification, and route-audit coverage.

## Checks

- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
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
  production scope beyond the simple same-module call route above. Broader argument binding,
  optional/default, ParamArray, member dispatch, and writeback semantics remain open FE-8.5/FE-7
  delivery work.
- The first attempt let HIR production lowering silently ignore call statements. The production guard
  now rejects unsupported syntax kinds up front, and call statements are covered by direct HIR
  lowering tests so scoped HIR lowering is not allowed to compile a partial program.
- The simple assignment parity check initially exposed metadata drift in assignment intent and
  declaration line numbers; HIR lowering now preserves implicit assignment intent and projects local
  declaration source lines into procedure metadata.
- A corpus bookkeeping error briefly attached the FE-8.5 call/coercion rationale to
  `examples/basic/arithmetic.bas`. The corpus test now asserts that the single bug row is
  gone and that `conformance_call_coercion_mixed_variant_to_long` is an intentional metadata
  improvement, so equivalent arithmetic cannot mask a call/coercion residual.
