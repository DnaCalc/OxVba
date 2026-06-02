# Frontend V2 Gate Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.1`
Workset lane: FE-5.1 `frontend_v2` gate

## Outcome

Added an explicit compiler-level opt-in gate:

- `CompileOptions { frontend_v2: bool }` defaults to `false`;
- initially, `compile_with_options(source, CompileOptions::default())` routed through the existing
  legacy compiler path; later FE-8/FE-9 continuations made default compile HIR-first with legacy
  fallback only for unsupported residuals;
- initially, `compile_with_options(source, CompileOptions { frontend_v2: true })` routed through
  the temporary CST bridge before legacy lowering; current code routes through direct HIR production
  lowering and reports HIR `Unsupported` as a front-end error instead of falling back;
- tests prove the default path keeps the same emitted instruction sequence as `compile(source)`,
  the opt-in HIR route compiles a supported assignment family, and syntax parse errors are reported
  before lowering under the v2 route.
- after reopening, tests also prove a completed FE-4/HIR construct is routed correctly:
  colon-separated inline assignment remains rejected by the explicit legacy baseline, while both
  default HIR-first mode and `CompileOptions { frontend_v2: true }` accept it through HIR lowering.

The gate is no longer a bridge route. It remains stricter than default mode because default mode can
fall back for unsupported residuals, while explicit `frontend_v2` mode reports unsupported HIR
shapes.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-compiler compile_options_ --quiet`
  - First-run result: passed, 3 tests.
  - Reopen result: passed, 4 tests after adding the completed-construct gate check.
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - Reopen result: passed, 8 tests.
- `cargo fmt --check -p oxvba-compiler`
  - Reopen result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The important constraint is default behavior. The gate test compares the default `CompileOptions`
route against `compile(source)` by emitted instruction debug text, which catches accidental default
routing changes without requiring byte-for-byte package identity.

The v2 route began as a smoke path through the FE-4 bridge. Current code routes it through HIR
production lowering; terminal closure still requires the semantic/differential harness and route
audit to prove the selected constructs broadly enough to remove residual fallback.

Reopen fresh-eyes review checked that the gate is not merely a boolean scaffold. The new inline
statement fixture proves a real behavior difference is available only behind the opt-in route:
the explicit legacy baseline still reports the unsupported statement, while default HIR-first mode
and frontend v2 compile the same source through HIR.

Residuals left for later beads:

- environment/CLI/project-manifest wiring can be layered on top of this compiler API when needed;
- semantic/differential harness work belongs to FE-5.2 and FE-5.3;
- frontend v2 default flip belongs to FE-10.
