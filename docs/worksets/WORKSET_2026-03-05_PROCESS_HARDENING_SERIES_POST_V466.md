# Workset: Process Hardening Series (Post-v466)

Date: 2026-03-05  
Scope: operational hardening after COM early-binding ladder closure (`v466`)  
Status: completed (2026-03-05)

## Objectives

1. Eliminate accidental evidence churn during validation cycles.
2. Enforce cleaner commit hygiene (code/spec vs evidence refresh).
3. Prevent profile-scope drift (editing historical profile artifacts unintentionally).
4. Stabilize run identity across conformance/perf cycles.
5. Keep evidence storage bounded without losing useful history.
6. Strengthen machine-readable gate outputs for automation.
7. Improve formal-lane observability with structured, low-noise output.

## Hardening Ladder

### HN1 - No-Artifact Validation Default
- Deliverables:
  - `meta-check` support for `-NoArtifacts`.
  - matrix/formal/conformance lanes support no-artifact redirection to `temp/no-artifacts/...`.
- Evidence:
  - dry run with `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal -NoArtifacts`.
- Exit:
  - no tracked `LATEST` files change in no-artifact mode.

### HN2 - Stable Run-ID Discipline
- Deliverables:
  - shared run-id resolver with lock-aware reuse window.
  - conformance/perf runners accept explicit `-RunId` and support deterministic run grouping.
- Evidence:
  - repeated same-cycle runs reuse run-id without duplicate timestamped churn.
- Exit:
  - run-id consistency visible in lane summaries.

### HN3 - Commit Scope Split Guard
- Deliverables:
  - staged-scope checker (`code/spec` vs `docs/evidence` mixed-set detection).
  - operator guidance in `OPERATIONS.md` and testing docs.
- Evidence:
  - scripted failure on mixed staged set (unless explicit override).
- Exit:
  - default workflow supports two-commit closure pattern.

### HN4 - Profile Artifact Scope Guard
- Deliverables:
  - staged/working guard that blocks profile artifact edits outside active ladder range by default.
- Evidence:
  - synthetic test: changing `docs/evidence/profiles/v<old>/...` fails under active newer ladder.
- Exit:
  - accidental historical profile mutations are stopped early.

### HN5 - Evidence Retention and Prune
- Deliverables:
  - prune utility implementing `LATEST + N` retention for timestamped evidence runs.
  - documented housekeeping policy.
- Evidence:
  - dry-run + live-run logs with explicit deleted/would-delete counts.
- Exit:
  - repeatable evidence-size control workflow in place.

### HN6 - Gate Manifest Formalization
- Deliverables:
  - machine-readable `gate.json` emitted alongside integrated gate reports.
  - markdown gate report generated from the same manifest data.
- Evidence:
  - integrated gate output contains `gate.json` + `integrated_gate.md`.
- Exit:
  - gate consumption can be automated without parsing markdown.

### HN7 - Formal Lane Structured Output and Quiet Mode
- Deliverables:
  - formal lane JSONL output (`latest_run.jsonl`).
  - quieter default reporting with optional verbose failure surfacing.
- Evidence:
  - formal run artifacts include markdown, csv, and jsonl records.
- Exit:
  - long formal runs are monitorable with reduced terminal noise.

### HN8 - Adoption and Backfill Hygiene
- Deliverables:
  - update script/docs inventories for new options and guardrails.
  - optional targeted backfill plan for historical gate manifests (non-blocking).
- Evidence:
  - docs references are current and command examples are executable.
- Exit:
  - operators can reliably use hardened flow without implicit tribal knowledge.

## Deferred/Optional Follow-ups

1. Add CI lane that runs `meta-check -NoArtifacts` and both commit-scope guards.
2. Add pre-commit hook template wiring `check-staged-commit-scope` and `validate-profile-artifact-scope`.
3. Backfill `gate.json` for selected historical terminal gates (`v386`, `v406`, `v466`) for downstream tooling consistency.
4. Add retention policy exceptions for milestone-tagged evidence bundles.

## Notes

- This series is operational and does not change language/runtime semantics.
- Formal failures remain non-blocking under existing deferred-gate policy unless policy is explicitly tightened.

## Execution Evidence (2026-03-05)

### HN1 - No-Artifact Validation Default
- Command:
  - `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal -NoArtifacts`
- Result:
  - completed successfully (`[oxvba] meta check complete`)
  - verified no tracked working-tree churn (`git status --short` clean)

### HN2 - Stable Run-ID Discipline
- Additional fix applied:
  - `scripts/lib-run-context.ps1` `Resolve-RunId` now parses `generated_utc` robustly for JSON `DateTime` conversion cases, preventing false stale-age misses.
- Evidence:
  - repeated resolver call reused same ID:
    - `runid1=20260305T210719Z`
    - `runid2=20260305T210719Z`
  - conformance + perf runners accepted explicit run-id and wrote grouped output:
    - `./scripts/run-com-early-conformance.ps1 -RunId HN2RUN -NoArtifacts -NoLatest`
    - `./scripts/run-com-early-perf.ps1 -RunId HN2RUN -Iterations 1 -NoArtifacts -NoLatest`
    - artifacts under:
      - `temp/no-artifacts/com-early-conformance/HN2RUN/`
      - `temp/no-artifacts/com-early-perf/HN2RUN/`

### HN3 - Commit Scope Split Guard
- Synthetic failure check:
  - staged one evidence file + one non-evidence file
  - `./scripts/check-staged-commit-scope.ps1` failed as expected with
    - `mixed staged set detected (evidence + code/spec)`

### HN4 - Profile Artifact Scope Guard
- Synthetic failure check:
  - created untracked `docs/evidence/profiles/v1/HN4_SCOPE_TEST.tmp`
  - `./scripts/validate-profile-artifact-scope.ps1 -Mode working -IncludeUntracked` failed as expected with
    - `changed profile artifacts outside allowed set [...]`

### HN5 - Evidence Retention and Prune
- Dry run + live run on isolated root:
  - `./scripts/prune-evidence-artifacts.ps1 -Roots temp/hn5-prune -KeepCount 2 -WhatIf`
  - `./scripts/prune-evidence-artifacts.ps1 -Roots temp/hn5-prune -KeepCount 2`
- Result:
  - dry run reported `would delete ...`
  - live run reported `deleted ...`

### HN6 - Gate Manifest Formalization
- Command:
  - `./scripts/run-profile-gate.ps1 -ProfileScope mvp-profile-v1 -RunId HN6RUN -NoArtifacts -SkipBench`
- Result:
  - integrated gate pass (`integrated gate: PASS`)
  - manifest + report emitted:
    - `temp/no-artifacts/profile-gate/HN6RUN/gate.json`
    - `temp/no-artifacts/profile-gate/HN6RUN/integrated_gate.md`

### HN7 - Formal Lane Structured Output and Quiet Mode
- Command:
  - `./scripts/run-formal.ps1 -ProfileScope mvp-profile-v1 -RunId HN7RUN -NoArtifacts -Quiet`
- Result:
  - completed and emitted structured artifacts:
    - `temp/no-artifacts/formal/HN7RUN/latest_run.md`
    - `temp/no-artifacts/formal/HN7RUN/latest_run.csv`
    - `temp/no-artifacts/formal/HN7RUN/latest_run.jsonl`

### HN8 - Adoption and Backfill Hygiene
- Documentation was updated in the process-improvement commit (`a57e2f4`) across README/operations/testing/conformance/script docs.
- Executable guardrail examples verified:
  - `./scripts/check-staged-commit-scope.ps1`
  - `./scripts/validate-profile-artifact-scope.ps1 -Mode staged`
