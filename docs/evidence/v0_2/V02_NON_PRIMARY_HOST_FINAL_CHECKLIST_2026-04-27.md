# V0.2 Non-Primary Host Final Checklist

Date: 2026-04-27

Bead: `bd-bqm8.9.4`

Parent: `bd-bqm8.9`

## Scope

This checklist closes the non-primary host validation breadth lane for V0.2.
The lane covers active Linux, macOS, and wasm/WASI validation for portable
surfaces, plus deterministic product-truth boundaries for unsupported native
COM and Office automation outside the Windows-primary host.

## Validation

- `./scripts/meta-check.ps1 -Fast -NoArtifacts`
  - Result: passed.
  - Coverage: governance, profile scope, language coverage, drift checks,
    `cargo fmt --check`, `cargo clippy`, full workspace `cargo test`, and
    doc tests.
- `cargo test -p oxvba-host --lib -- --nocapture`
  - Result: passed.
  - Coverage: 926 host library tests.
- `cargo test -p oxvba-host --test imported_collection_newenum_regression -- --nocapture`
  - Result: passed.
  - Coverage: 8 imported `NewEnum` regression tests.
- `cargo test -p oxvba-hal -- --nocapture`
  - Result: passed.
  - Coverage: 142 HAL library tests.
- `./scripts/run-hal-conformance-wasm32.ps1 -SkipTests -OutputDir temp/no-artifacts/hal_wasm32_v02_9_final`
  - Result: passed.
  - Artifacts:
    - `temp/no-artifacts/hal_wasm32_v02_9_final/HAL_CONFORMANCE_1777286333.md`
    - `temp/no-artifacts/hal_wasm32_v02_9_final/HAL_CONFORMANCE_1777286333.jsonl`
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Linux, macOS, and Windows ready jobs run active fast validation. | Complete | `.github/workflows/ci.yml`; `bd-bqm8.9.2` evidence. |
| wasm/WASI has an active conformance executable lane. | Complete | `wasm-hal-ready`; local `HAL_CONFORMANCE_1777286333.*` artifacts. |
| Product claims separate Windows-primary COM and non-primary portable surfaces. | Complete | `V02_NON_PRIMARY_HOST_PRODUCT_MATRIX_2026-04-27.md`. |
| Non-Windows native COM and typelib behavior is deterministic unsupported behavior, not parity. | Complete | HAL/COM adapter tests and product matrix boundaries. |
| CI hardening and workspace validation drift exposed during the checklist is resolved. | Complete | Fast meta-check passed after clippy, coverage, VM, JIT, and HAL fixes. |

## Notes

The final checklist flushed supporting drift that was necessary for the active
non-primary host lane to stay credible: workspace clippy drift, language
coverage `derived-summary` validator drift, wasm COM stub warning drift,
host-backed dynamic-library diagnostic labels, CVErr-tag JIT fallback behavior,
WithEvents source identity matching without broadening global object equality,
legacy project-object handle compatibility for imported collection `NewEnum`,
and callback argument indexing for COM event subscription intrinsics.

## Boundary

The V0.2 non-primary host claim remains validation breadth, not parity with
Windows-native Office automation. wasm/WASI validation is the HAL conformance
binary build/run path; full wasm-target unit-test parity remains intentionally
out of scope because current unit tests include Windows-native COM helpers.

## Result

`bd-bqm8.9.4` is complete. Parent bead `bd-bqm8.9` is complete because all
child delivery, documentation, and final checklist beads are closed with active
validation evidence and product-truth boundaries aligned.
