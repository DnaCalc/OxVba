# Frontend V2 Gate Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.1`
Workset lane: FE-5.1 `frontend_v2` gate

## Outcome

Added an explicit compiler-level opt-in gate:

- `CompileOptions { frontend_v2: bool }` defaults to `false`;
- `compile_with_options(source, CompileOptions::default())` routes through the existing legacy
  compiler path;
- `compile_with_options(source, CompileOptions { frontend_v2: true })` routes through the temporary
  CST bridge before legacy lowering;
- tests prove the default path keeps the same emitted instruction sequence as `compile(source)`,
  the opt-in bridge compiles a supported assignment family, and syntax parse errors are reported
  before legacy lowering under the v2 route.
- after reopening, tests also prove a completed FE-4 bridge construct is gated correctly:
  colon-separated inline assignment remains rejected by the default legacy path, while
  `CompileOptions { frontend_v2: true }` accepts it through CST validation and bridge lowering.

No production caller is switched to frontend v2 by default.

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

The v2 route is intentionally a smoke path through the FE-4 bridge. It is not a full frontend v2
pipeline and should not become default until the semantic/differential harness proves the selected
constructs.

Reopen fresh-eyes review checked that the gate is not merely a boolean scaffold. The new inline
statement fixture proves a real behavior difference is available only behind the opt-in route:
legacy/default compile still reports the unsupported statement, while frontend v2 compiles the same
source through the CST bridge.

Residuals left for later beads:

- environment/CLI/project-manifest wiring can be layered on top of this compiler API when needed;
- semantic/differential harness work belongs to FE-5.2 and FE-5.3;
- frontend v2 default flip belongs to FE-10.
