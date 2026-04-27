# V0.2 Language-Service Direct API Tests

Date: 2026-04-27

Bead: `bd-bqm8.8.3`

## Scope

This bead hardens the direct `oxvba-languageservice` semantic query surface
selected by `V02_LANGUAGE_SERVICE_PRODUCT_MATRIX_2026-04-27.md`.

## Added Test

Added:

- `v02_direct_language_service_provider_exercises_product_matrix_queries`

The test exercises the public `LanguageServiceProvider` trait across:

- diagnostics,
- document symbols,
- workspace symbols,
- semantic classifications,
- completions,
- signature help,
- hover,
- go-to-definition,
- find-references,
- rename preparation,
- safe reference-update analysis,
- diagnostics-driven code actions.

## Existing Coverage Refreshed

The full crate suite also covers:

- semantic snapshot construction and symbol lookup,
- workspace open/change/close and reference-project loading,
- cross-module and cross-project navigation,
- imported-typelib rename blockers,
- host workspace sessions and embedded build/run snapshot handoff,
- local editor latency budget.

## Checks Run

- `cargo test -p oxvba-languageservice v02_direct_language_service_provider_exercises_product_matrix_queries -- --nocapture`
- `cargo test -p oxvba-languageservice -- --nocapture`

Result: `55 passed`.

## Result

`bd-bqm8.8.3` is complete for direct language-service semantic query tests. The
language-service lane remains in-progress pending LSP transport tests, host
guidance, and the final checklist.
