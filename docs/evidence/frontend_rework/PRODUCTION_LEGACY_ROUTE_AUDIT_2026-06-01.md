# Production Legacy Route Audit Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_legacy_route_audit.rs`, an executable audit report for
the FE-9 terminal route gate.

Current gate result: **not passed**.

The audit proves the good path and exposes the remaining production residuals:

- scoped procedure/local/assignment/arithmetic fixtures classify as `HirProduction`;
- simple same-module procedure call statement fixtures now reach `HirProduction`; the remaining
  call/coercion seed-row delta is a documented source-map metadata improvement, not a syntax-route
  residual or bytecode/call-descriptor bug;
- simple multiline `If ... Then ... End If` fixtures now reach `HirProduction`;
- simple front-checked `Do While ... Loop`, `Do Until`, and post-check loop fixtures now reach
  `HirProduction`;
- `While`/`Wend` fixtures now reach `HirProduction`;
- simple `For` range fixtures now reach `HirProduction`;
- simple single-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case` range fixtures now reach `HirProduction`;
- multi-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case Is` and `For Each` fixtures remain explicit `LegacyFallbackResidual` entries owned
  by FE-8.5 rather than being hidden under a generic "control flow" note;
- project compilation now selects `ModuleAwareBindPlan` unconditionally; the old
  `ProjectLoweringStrategy::RewriteBridge` path remains only as an internal parity-test strategy,
  not a production environment-selected path;
- `oxvba-languageservice` now uses compiler query/HIR facts for symbols, callable signatures,
  diagnostics, signature help, and the PtrSafe quick-fix diagnostic; `semantic.rs` now builds a
  legacy `BoundModule` only on the fallback path when HIR binding is unavailable.

This audit intentionally does not close FE-9.6. The workset terminal gate requires no scoped
production compile path to depend on legacy `parse_expr`/string-splitting, `project.rs`
source-text rewrite behavior, or language-service duplicate semantic surfaces. The project rewrite
bridge escape hatch has been removed, but the language-service residual remains.

## Reopened Owners

The audit result requires reopened delivery work rather than terminal closure:

- `bd-aprs.5.4` (`FE-4.4 CST-to-legacy bridge`) was reopened and then narrowed: the hidden bridge
  fallback was removed, so remaining unsupported constructs are owned by HIR/project delivery beads
  and outer route policy rather than the bridge itself;
- `bd-aprs.8.1` through `bd-aprs.8.6` (`FE-7.*`) have removed the production rewrite-bridge
  selector but still own broader replacement or quarantine of source-text lowering internals where
  those internals remain compatibility scaffolding;
- `bd-aprs.9.5` (`FE-8.5 Production HIR lowering`) for expanding production HIR lowering beyond
  the initial procedure/local/assignment/expression, simple same-module call, and simple multiline
  If/front-checked Do While/simple Select Case subset;
- `bd-aprs.10.4` (`FE-9.4 Language-service reconciliation`) for retiring the remaining internal
  language-service `BoundModule` fallback/diagnostic compatibility surface.

## Checks

- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Closing FE-9.6 would still be incorrect. The route audit no longer finds the project rewrite
  bridge as a production-selected path or signature help using `BoundModule`, but it still finds the
  fallback language-service compatibility residual.
- The scoped HIR production path is real, but the workset goal is broader than that subset.
- Procedure-call syntax, simple multiline If syntax, front-checked Do While syntax, and simple
  single-value Select Case syntax are no longer themselves route blockers. The call/coercion
  fixture now has matching bytecode/call descriptors. FE-8.5 still owns broader HIR lowering
  coverage, starting with the now-audited `Case Is` and `For Each` variants outside this narrow
  slice.
- The right next step is reopened delivery work on the owning beads, not another evidence-only
  closure pass.
