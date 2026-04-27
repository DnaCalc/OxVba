# XLL Runtime Invocation Bridge

Date: 2026-04-27
Bead: `bd-xll1.3`

## Scope

Deliver the generated-source bridge from Excel/XLL function entry points to
OxVba runtime procedure invocation.

## Change

`crates/oxvba-build/src/xll.rs` now emits generated XLL source with:

- shared embedded `.oxb` session initialization via `OxBundle`,
  `oxvba_host::Engine`, and `ProjectRuntimeSession`,
- one exported `extern "system"` XLL function per `NativeExportDescriptor`,
- bounded XLOPER12 pointer argument conversion into `RuntimeValue` arguments,
- `Engine::invoke_procedure(session, module, procedure, &args)` dispatch using
  the descriptor's canonical module/procedure handoff,
- bounded `RuntimeValue` to XLOPER12-compatible return allocation, and
- focused source-generation regressions for the emitted exported wrapper,
  argument conversion, runtime invocation, and return bridge.

## Validation

Commands:

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib xll -- --nocapture
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib xll -- --nocapture`: pass, 2/2

## Remaining Boundary

This is still generated-source validation. The next XLL bead must publish the
bounded supported subset and validation matrix honestly, including that
Excel-loaded XLL registration/invocation parity is not yet proven by this
source-generation test alone.
