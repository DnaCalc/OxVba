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
- procedure call statement fixtures still classify as HIR-unsupported residuals; the syntax bridge
  now returns `Unsupported`, but the outer default policy may still fall back to legacy compilation;
- `project.rs` still contains the `ProjectLoweringStrategy::RewriteBridge` /
  `rewrite_module_source` source-text rewrite production surface for project/class/COM/
  default-member semantics;
- `oxvba-languageservice` now prefers compiler query/HIR facts for symbols and diagnostics, but
  `SemanticSnapshot` still retains legacy `BoundModule` compatibility for signature help and
  older workspace features.

This audit intentionally does not close FE-9.6. The workset terminal gate requires no scoped
production compile path to depend on legacy `parse_expr`/string-splitting or `project.rs`
source-text rewrite behavior. That condition is false today.

## Reopened Owners

The audit result requires reopened delivery work rather than terminal closure:

- `bd-aprs.5.4` (`FE-4.4 CST-to-legacy bridge`) was reopened and then narrowed: the hidden bridge
  fallback was removed, so remaining unsupported constructs are owned by HIR/project delivery beads
  and outer route policy rather than the bridge itself;
- `bd-aprs.8.1` through `bd-aprs.8.6` (`FE-7.*`) for project/class/member/property/event/external
  semantics that still rely on `project.rs` rewrite glue;
- `bd-aprs.9.5` (`FE-8.5 Production HIR lowering`) for expanding HIR production lowering beyond
  the initial procedure/local/assignment/expression subset;
- `bd-aprs.10.4` (`FE-9.4 Language-service reconciliation`) for retiring the remaining
  language-service `BoundModule` compatibility surface.

## Checks

- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Closing FE-9.6 would be incorrect. The route audit finds explicit legacy fallback and static
  production residuals.
- The scoped HIR production path is real, but the workset goal is broader than that subset.
- The right next step is reopened delivery work on the owning beads, not another evidence-only
  closure pass.
