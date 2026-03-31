# Test Fixture Boundary Audit

Date: 2026-03-31

## Purpose

Record the remaining production/test boundary seams after moving external real-library typelib
resolution onto the generic live COM path and restricting synthetic fixture identity lookup to
test-only feature builds.

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

## Remaining Quick-Pass Audit Targets

1. `crates/oxvba-com/src/typelib_catalog.rs`
   Current issue: fixture metadata tables still live in the main production source file even
   though fixture identity lookup is now test-feature-only.
   Target: move synthetic typelib metadata tables into an explicitly test-support module or crate.

2. `crates/oxvba-com/src/windows_test_dispatch.rs`
   Current issue: controlled COM server fixture code still physically lives in the main COM crate
   even though it is no longer compiled into normal builds.
   Target: move it behind a narrower test-support boundary without breaking the registered-fixture
   harness.

3. `crates/oxvba-compiler/src/resolve.rs`
   Current issue: `known_dispatch_member_literal_token_for_object_arg` still contains
   `OxVba.TestDispatch`-specific compile-time optimization logic.
   Target: either move it behind the fixture feature or replace it with a more honest test-owned
   path.

4. `crates/oxvba-hal/src/adapters/standard/com.rs`
   Current issue: the direct `CreateObject` literal special-case is removed, but the fallback
   dynamic-member path still reaches for fixture metadata.
   Target: replace the remaining path with a fixture-owned adapter seam or gate it as test-only.

5. `crates/oxvba-project/src/load.rs`
   Current issue: `.basproj` test scenarios still rely on fixture typelib importlib names flowing
   through the production loader.
   Target: decide whether that path should use an injected test fixture provider or explicit
   synthetic referenced manifests in tests.

## Acceptance Bar For Closure

- No real external library behavior depends on fixture-owned routing.
- Synthetic fixture identity or metadata support is unavailable in normal production builds unless a
  bounded test-support feature is explicitly enabled.
- Remaining fixture hooks are visibly test-owned and documented as such.
