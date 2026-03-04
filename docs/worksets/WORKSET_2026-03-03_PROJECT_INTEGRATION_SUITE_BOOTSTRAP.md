# WORKSET: Project Integration Suite Bootstrap (2026-03-03)

Workset ID: `WIS-2026-03-03-INTEGRATION-SUITE-V1`

## Objective

Create a tracked, extensible, deterministic integration suite for multi-module and multi-project VBA execution in OxVba, including host interface behavior and known-limit coverage.

## Scope

- Data-driven integration catalog and fixture hierarchy.
- Deterministic host-policy execution for host-sensitive features.
- Increasing complexity ladder (`L1..L6`).
- Active-pass and active-limit cases.
- Deferred/planned integration items linked to conformance/deferred-gate registers.
- Repeatable runner + evidence artifacts.

## Phase Plan

1. `I1` Catalog + fixture contract
- Define catalog schema (`catalog.psv`) and fixture naming rules.
- Encode case metadata: profile, policy preset/overrides, expected status/phase, deferred links.

2. `I2` Deterministic harness integration
- Add data-driven host integration test runner using `Engine::execute_project_with_snapshot_phased` + `ProjectManifest`.
- Support backend selection (`vm`, `jit`, `both`) and filtered runs.

3. `I3` Complexity ladder seeding
- Add initial `L1..L5` active cases:
  - baseline arithmetic,
  - multi-module project calls,
  - cross-project references/shadowing,
  - deterministic host intrinsic integration,
  - known-limit compile-time/project-model gates.

4. `I4` Tracking + governance
- Add catalog validation script.
- Add suite runner script that emits timestamped run artifacts.
- Integrate catalog validation into `meta-check`.

5. `I5` Deferred/uncertainty linkage
- Add explicit deferred/uncertain integration notes mapped to `ODG-*` and `CCT-*` topics.
- Keep deferred items non-blocking for active integration lane.

6. `I6` First execution + evidence
- Execute suite.
- Publish run artifact and baseline status for follow-up growth.

## Execution Policy

- Active cases are blocking for suite pass.
- Active-limit cases are blocking for expected-failure shape (phase + error token stability).
- Deferred/planned cases are tracked, not executed, and must carry `ODG`/`CCT` linkage.

## Extensibility Rules

- New integration cases must:
  - use unique `case_id`,
  - specify complexity level,
  - define deterministic expectations or explicit expected-limit behavior,
  - include traceability to conformance topics.
- Host-sensitive cases must declare policy overrides explicitly.
- Any new deferred case must link to at least one `ODG` or `CCT` item.
