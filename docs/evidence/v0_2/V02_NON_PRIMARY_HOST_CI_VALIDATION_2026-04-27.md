# V0.2 Non-Primary Host CI Validation

Date: 2026-04-27

Bead: `bd-bqm8.9.2`

## Scope

This evidence closes the delivery bead for active Linux, macOS, and wasm32/WASI
validation jobs. Windows remains the primary host for native COM and Office
automation behavior; non-primary host validation is limited to portable build,
governance, and HAL conformance surfaces.

## Changes

- `.github/workflows/ci.yml` now runs on `workflow_dispatch`, `pull_request`,
  and `push` to `master`.
- `windows-ready`, `linux-ready`, and `macos-ready` run
  `./scripts/meta-check.ps1 -Fast -NoArtifacts`.
- `wasm-hal-ready` installs `wasm32-wasip1` and runs
  `./scripts/run-hal-conformance-wasm32.ps1 -SkipTests -OutputDir temp/no-artifacts/hal_wasm32_ci`.
- `scripts/run-hal-conformance-wasm32.ps1` enables
  `$PSNativeCommandUseErrorActionPreference = $true` so failed native commands
  fail the job instead of producing stale artifacts.
- Non-Windows COM/typelib support now returns deterministic unsupported errors
  on wasm/WASI where dynamic library loading and native typelib resolution are
  unavailable.
- The controlled self-object dispatch test compares stable COM compatibility
  identity instead of process-local wrapper pointer identity.

## Validation

- `cargo test -p oxvba-hal -- --nocapture`
  - Result: passed.
  - Coverage: 142 library tests, 0 `hal-conformance` unit tests, 0 doc tests.
- `./scripts/run-hal-conformance-wasm32.ps1 -SkipTests -OutputDir temp/no-artifacts/hal_wasm32_v02_9`
  - Result: passed.
  - Artifacts:
    - `temp/no-artifacts/hal_wasm32_v02_9/HAL_CONFORMANCE_1777280393.md`
    - `temp/no-artifacts/hal_wasm32_v02_9/HAL_CONFORMANCE_1777280393.jsonl`
- `cargo fmt --check`
  - Result: passed.
- `./scripts/check-governance.ps1`
  - Result: passed.
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Boundary

The wasm CI lane intentionally validates the HAL conformance binary build/run
path rather than full `cargo test -p oxvba-hal --target wasm32-wasip1`. The
current unit-test tree still includes Windows-native COM test helpers that are
not valid wasm/WASI runtime claims. That broader wasm unit-test split remains a
future hardening task, not a V0.2 product claim.
