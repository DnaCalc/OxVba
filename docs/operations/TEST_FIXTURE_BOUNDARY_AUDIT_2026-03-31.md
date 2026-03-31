# Test Fixture Boundary Audit

Date: 2026-03-31

## Purpose

Record the production/test boundary cleanup after moving external real-library typelib resolution
onto the generic live COM path and restricting synthetic fixture support to explicit test-only
boundaries.

## Current Completed Cut

- `oxvba-com` normal builds no longer expose synthetic fixture typelib identity resolution as a
  silent production path.
- Synthetic fixture identity lookup now requires `cfg(test)` or the `fixture-typelibs` feature.
- Test-owning crates enable `fixture-typelibs` only through `dev-dependencies`.
- The synthetic `windows_test_dispatch` COM server is no longer compiled or re-exported in normal
  `oxvba-com` builds.
- `windows_client` synthetic activation and `windows_bridge` vtable fast-path are now gated behind
  the same fixture boundary instead of staying always-on.
- `oxvba-hal` no longer hard-codes `OxVba.TestDispatch` in the main `CreateObject` fallback path;
  it keys off fixture identity availability instead.
- Synthetic typelib metadata now lives under `crates/oxvba-com/src/fixtures/typelib_catalog.rs`
  instead of the main production catalog surface.
- The controlled COM server fixture now lives under
  `crates/oxvba-com/src/fixtures/windows_test_dispatch.rs`.
- The compiler no longer lowers `DispatchInvoke(CreateObject(...), "Name")` through an
  `OxVba.TestDispatch`-specific token optimization.
- The HAL no longer lowers fallback dynamic dispatch through fixture metadata in normal code flow.

## Closure Status

No additional production/test shortcut cleanup is open from this audit.

The remaining fixture hooks are now bounded and explicit:
- fixture identities and metadata require `cfg(test)` or `fixture-typelibs`
- fixture COM server code lives under the `fixtures/` boundary
- real external library behavior resolves through the generic live COM path
- tests that still use synthetic typelibs do so through the explicit fixture boundary rather than
  hidden production shortcuts

## Acceptance Bar For Closure

- No real external library behavior depends on fixture-owned routing.
- Synthetic fixture identity or metadata support is unavailable in normal production builds unless a
  bounded test-support feature is explicitly enabled.
- Remaining fixture hooks are visibly test-owned and documented as such.
