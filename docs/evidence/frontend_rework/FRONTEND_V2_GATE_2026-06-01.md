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

No production caller is switched to frontend v2 by default.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-compiler compile_options_ --quiet`
  - Result: passed, 3 tests.
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - Result: passed, 2 tests.
- `cargo fmt --check -p oxvba-compiler`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The important constraint is default behavior. The gate test compares the default `CompileOptions`
route against `compile(source)` by emitted instruction debug text, which catches accidental default
routing changes without requiring byte-for-byte package identity.

The v2 route is intentionally a smoke path through the FE-4 bridge. It is not a full frontend v2
pipeline and should not become default until the semantic/differential harness proves the selected
constructs.

Residuals left for later beads:

- environment/CLI/project-manifest wiring can be layered on top of this compiler API when needed;
- semantic/differential harness work belongs to FE-5.2 and FE-5.3;
- frontend v2 default flip belongs to FE-10.
