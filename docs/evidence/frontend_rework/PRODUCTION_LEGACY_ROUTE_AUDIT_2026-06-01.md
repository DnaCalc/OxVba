# Production Legacy Route Audit Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_legacy_route_audit.rs`, an executable audit report for
the FE-9 terminal route gate.

Current bounded recorded-fixture route audit result: **passed for the fixture set in this file**.
This is not terminal workset closure. The 2026-06-02 workset rework adds `bd-aprs.10.7` for the
broader accepted grammar matrix, compiler fixture corpus, host project corpus, language-service
corpus, and selected Excel oracle route audit before terminal closure.

The audit proves the good path and exposes the remaining production residuals:

- scoped procedure/local/assignment/arithmetic fixtures classify as `HirProduction`;
- simple same-module procedure call statement fixtures now reach `HirProduction`; the remaining
  call/coercion seed-row delta is a documented source-map metadata improvement, not a syntax-route
  residual or bytecode/call-descriptor bug;
- same-module statement-form procedure calls with bare arguments now reach `HirProduction`;
- statement-form member calls with bare arguments now reach `HirProduction`;
- simple multiline `If ... Then ... End If` fixtures now reach `HirProduction`;
- multiline `If ... Else ... End If` and `If ... ElseIf ... Else ... End If` fixtures now reach
  `HirProduction`;
- single-line `If ... Then ... Else ...` fixtures now reach `HirProduction`;
- simple front-checked `Do While ... Loop`, `Do Until`, and post-check loop fixtures now reach
  `HirProduction`;
- `While`/`Wend` fixtures now reach `HirProduction`;
- simple `For` range fixtures now reach `HirProduction`;
- simple single-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case` range fixtures now reach `HirProduction`;
- multi-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case Is` fixtures now reach `HirProduction`;
- `For Each` fixtures now reach `HirProduction`;
- `Exit Do`, `Exit For`, and `Exit Sub` fixtures now reach `HirProduction`;
- basic non-label `On Error` and `Resume` fixtures now reach `HirProduction`;
- label-targeted `On Error GoTo` and `Resume` fixtures now reach `HirProduction`;
- identifier and numeric-label `GoTo` fixtures now reach `HirProduction`;
- `GoSub` / `Return` fixtures now reach `HirProduction`;
- `Erase` fixtures now reach `HirProduction`;
- `Event` declaration plus `RaiseEvent` fixtures now reach `HirProduction`;
- single-source `Implements` directive fixtures now reach `HirProduction`;
- explicit-receiver value-side dot-member read/call fixtures now reach `HirProduction`;
- simple explicit-receiver member assignment target fixtures now reach `HirProduction`;
- bang member assignment target fixtures now reach `HirProduction`;
- statement-form member calls with bare arguments now reach `HirProduction`;
- read-side `With` member fixtures now reach `HirProduction`;
- project compilation now selects `ModuleAwareBindPlan` unconditionally; the old
  `ProjectLoweringStrategy::RewriteBridge` path remains only as an internal parity-test strategy,
  not a production environment-selected path;
- `oxvba-languageservice` now uses compiler query/HIR facts for symbols, callable signatures,
  diagnostics, signature help, and the PtrSafe quick-fix diagnostic; `semantic.rs` no longer builds
  a legacy `BoundModule` fallback when HIR binding is unavailable.

This audit no longer finds the previously tracked scoped production route residuals. The broader
workset still remains open for unaudited language surfaces and full terminal evidence, but this
specific FE-9.6 route audit now passes for its recorded fixtures and static checks.

Continuation update: the audit's successful fixture routes are now aligned with the ordinary
lightweight compile API as well as the explicit frontend-v2 bridge. `compile()` and
`compile_with_runtime_metadata()` try HIR production first for eligible completed constructs; the
legacy resolver path is now an explicit comparison helper plus unsupported-residual fallback rather
than the first path for completed single-source fixtures.

## Reopened Owners

The audit result records completed reopened delivery work and remaining broader workset scope:

- `bd-aprs.5.4` (`FE-4.4 CST-to-legacy bridge`) was reopened and then narrowed: the hidden bridge
  fallback was removed, so remaining unsupported constructs are owned by HIR/project delivery beads
  and outer route policy rather than the bridge itself;
- `bd-aprs.8.1` through `bd-aprs.8.6` (`FE-7.*`) have removed the production rewrite-bridge
  selector but still own broader replacement or quarantine of source-text lowering internals where
  those internals remain compatibility scaffolding;
- `bd-aprs.9.5` (`FE-8.5 Production HIR lowering`) for expanding production HIR lowering beyond
  the initial subset; the route audit fixtures now cover procedure calls and representative
  control-flow families through HIR production;
- `bd-aprs.10.4` (`FE-9.4 Language-service reconciliation`) retired the remaining internal
  language-service `BoundModule` fallback/diagnostic compatibility surface.

## Checks

- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_uses_hir_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The FE-9.6 audit fixture set now passes, but the workset goal is broader than that subset.
- The audit previously proved HIR reachability but not the plain `compile()` entry point. The
  lightweight compile path now has an executable route check for completed constructs, while project
  compile and broader unsupported surfaces remain separate workset scope.
- Procedure-call syntax, including same-module statement-form procedure calls with bare arguments,
  multiline and single-line If/ElseIf syntax, front-checked Do While syntax, basic Exit and
  error-control statements, identifier/numeric-label `GoTo`, `GoSub` / `Return`, `Erase`,
  one-dimensional, two-dimensional, and explicit static lower-bound dynamic-array runtime `ReDim`,
  read/write dynamic-array element access, initial fixed-array element aliasing and fixed-array
  `ReDim` alias rematerialization, local multidimensional dynamic/fixed element access,
  simple function declarations with declared return
  slots, and simple single-value Select Case syntax, plus basic `RaiseEvent` and single- or
  multi-declarator literal `Const`, `Event` declarations paired with `RaiseEvent`, single-source
  `Implements` directives, explicit-receiver value-side dot-member read/call syntax, statement-form
  member calls with bare arguments, simple dot/bang member assignment targets, and read-side `With`
  member syntax are no longer themselves route blockers. The call/coercion fixture now has matching
  bytecode/call descriptors. FE-8.5 still owns broader HIR lowering coverage for language surfaces
  outside this route-audited subset, but the audited fixtures in this file now classify as
  `HirProduction`.
- The next step is broader terminal evidence and expansion of the route-audit fixture set, not
  claiming complete compiler front-end replacement from this audit alone.
