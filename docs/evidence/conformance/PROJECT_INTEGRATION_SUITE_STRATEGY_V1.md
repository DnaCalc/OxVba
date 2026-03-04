# Project Integration Suite Strategy v1

Status: `working-draft`
Date: `2026-03-03`

## Goal

Provide a deterministic, tracked, and extensible integration lane that validates OxVba end-to-end behavior for real project shapes (modules, references, host interfaces), not only single-file fixtures.

## Suite Model

Execution substrate:
- `oxvba-host` integration test harness (`crates/oxvba-host/tests/project_integration_suite.rs`).
- Data source: `conformance/integration/catalog.psv`.
- Fixture tree: `conformance/integration/projects/<CASE_ID>/...`.

Case classes:
- `active`: expected green behavior.
- `active-limit`: expected failure behavior (known limit; must stay stable).
- `deferred` / `planned`: tracked but not executed.

Complexity ladder:
- `L1`: single-module sanity.
- `L2`: multi-module intra-project behavior.
- `L3`: cross-project references and precedence/shadowing.
- `L4`: deterministic host interface integration and policy behavior.
- `L5`: challenging limits and explicit gate behavior.
- `L6`: deferred higher-scope integration topics.

## Determinism Contract

All active cases must be deterministic under declared runtime profile and policy.

Configuration per case includes:
- runtime profile,
- policy preset,
- optional policy overrides,
- unsupported-feature mode.

Host-sensitive cases use deterministic policy with explicit overrides (for example scripted UI virtualization).

## Execution and Evidence

Primary runner:
- `./scripts/run-project-integration-suite.ps1`

Outputs:
- `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_RUN_<timestamp>.md`
- `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_LOG_<timestamp>.txt`
- `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_LATEST.md`
- `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_LATEST.csv`

Validation guard:
- `./scripts/validate-project-integration-catalog.ps1`
- Included in `./scripts/meta-check.ps1`.

## Growth Plan

Near-term growth:
1. Add `jit` coverage for more active cases after first green baseline.
2. Add type-library/importlib and startup/entrypoint cases as PMR/HAL support matures.
3. Add statement-level file I/O integration cases when `Open`/`Print#`/`Input#` semantics land.
4. Add Excel-oracle paired project cases for resolved `CCT` topics.

Gate usage:
- Keep this lane required for active-case stability in routine correctness runs.
- Keep deferred/planned items non-blocking but continuously tracked.
