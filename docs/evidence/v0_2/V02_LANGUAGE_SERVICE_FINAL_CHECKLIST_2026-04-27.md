# V0.2 Language-Service Final Checklist

Date: 2026-04-27

Bead: `bd-bqm8.8.6`

Parent: `bd-bqm8.8`

## Scope

This checklist closes the V0.2 language-service roundout lane for OxIde,
`oxvba-lsp`, and the VS Code alternate-editor path.

## Completed Evidence Chain

- `bd-bqm8.8.1`: child-bead rollout evidence in
  `V02_LANGUAGE_SERVICE_ROLLOUT_2026-04-27.md`.
- `bd-bqm8.8.2`: product matrix evidence in
  `V02_LANGUAGE_SERVICE_PRODUCT_MATRIX_2026-04-27.md`.
- `bd-bqm8.8.3`: direct API tests evidence in
  `V02_LANGUAGE_SERVICE_DIRECT_API_TESTS_2026-04-27.md`.
- `bd-bqm8.8.4`: LSP transport tests evidence in
  `V02_LANGUAGE_SERVICE_LSP_TRANSPORT_TESTS_2026-04-27.md`.
- `bd-bqm8.8.5`: OxIde and VS Code host-consumption guidance evidence in
  `V02_LANGUAGE_SERVICE_HOST_CONSUMPTION_GUIDANCE_2026-04-27.md`.

## Matrix Result

Supported-active rows have executable evidence:

- `LS-V02-001` through `LS-V02-008`: direct language-service API rows.
- `LS-V02-009` through `LS-V02-011`: LSP transport rows.

Supported-guidance rows have published host-consumption guidance:

- `LS-V02-012`: OxIde direct host path.
- `LS-V02-013`: VS Code alternate-editor path through `oxvba-lsp`.

Unsupported/out-of-scope rows remain explicit:

- `LS-V02-014`: LSP-owned project authoring is unsupported in V0.2.
- `LS-V02-015`: multi-root LSP/workspace semantics are unsupported in V0.2.
- `LS-V02-016`: full VS Code extension package, full VBIDE parity,
  designer/forms editing, and complete refactoring parity are out of scope.

## Checks Run

- `cargo test -p oxvba-languageservice -- --nocapture`
- `cargo test -p oxvba-lsp -- --nocapture`
- `rg "LS-V02-|unsupported-v02|out-of-scope-v02|bd-bqm8\\.8" docs/evidence/v0_2/V02_LANGUAGE_SERVICE_PRODUCT_MATRIX_2026-04-27.md docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md docs/LANGUAGE_SERVICE_SHOWCASE.md docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md`
- `./scripts/check-governance.ps1`
- `git diff --check`

Results:

- `oxvba-languageservice`: `55 passed`.
- `oxvba-lsp`: `14 passed` across lib/main tests.
- Governance passed.
- Diff whitespace check passed.

Known non-blocking warning:

- `oxvba-lsp` still emits existing `tower_lsp::lsp_types` deprecation warnings
  for `DocumentSymbol::deprecated` and `SymbolInformation::deprecated`.

## Result

`bd-bqm8.8.6` is complete. Parent lane `bd-bqm8.8` is complete for the V0.2
language-service roundout because direct API tests, LSP transport tests,
host-consumption guidance, governance, and residual unsupported boundaries are
all documented and validated.
