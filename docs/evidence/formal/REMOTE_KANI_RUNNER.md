# Remote Kani Runner

This project supports long-running deferred Kani lanes on a remote Linux host using an isolated transient directory.

## Scope Constraint

- Remote work directory: `/home/ubuntu/.dnacalc_remote`
- Do not write outside that directory.
- Treat the directory as transient: when reconnecting, always re-run `Ensure` before queueing jobs.

## Orchestration Script

- Script: `scripts/run-formal-kani-remote.ps1`
- Transport: `ssh` + `scp`
- Default remote target:
  - user: `ubuntu`
  - host: `94.72.99.81`
  - key: `%USERPROFILE%\.ssh\acfs_ed25519`

## Actions

1. Provision/refresh remote wrappers and tooling shell scripts:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action Ensure
```

2. Probe concurrency recommendation:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action ProbeCapacity
```

3. Start deferred queue dispatch:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action StartDeferred
```

Optional overrides:

```powershell
./scripts/run-formal-kani-remote.ps1 `
 -Action StartDeferred `
  -DeferredMode cumulative `
 -DeferredStrategy dedup `
  -DeferredConcurrency 2 `
  -ObligationTimeoutSeconds 10800 `
  -ObligationTimeoutRetries 1 `
  -ObligationTimeoutMultiplier 10 `
  -MemorySoftUsedPercent 85 `
  -MemoryHardUsedPercent 92 `
  -HardPressureAction pause `
  -DeferredVersions "81 82 83 87 88 89 90 91 93 94 95 96 99 100 101 102 103 104 105 106"
```

Recommended for foldback sweeps with cumulative semantics:

```powershell
./scripts/run-formal-kani-remote.ps1 `
  -Action StartDeferred `
  -DeferredMode cumulative `
  -DeferredStrategy dedup `
  -DeferredVersions "146" `
  -DeferredConcurrency 3
```

This runs one cumulative lane at the highest target version and reuses deduplicated command results.

4. Show running jobs and deferred lane summary:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action Status
```

Optional status snapshots (JSONL, one record per status poll):

```powershell
./scripts/run-formal-kani-remote.ps1 `
  -Action Status `
  -StatusSnapshotJsonl temp/async/kani_remote/status_snapshots.jsonl
```

4b. Active memory telemetry samples with optional guard actions:

```powershell
# one sample
./scripts/run-formal-kani-remote.ps1 -Action Monitor

# 10-minute monitor loop, 30s interval, auto-pause on pressure and resume on recovery
./scripts/run-formal-kani-remote.ps1 `
  -Action Monitor `
  -MonitorDurationSeconds 600 `
  -MonitorIntervalSeconds 30 `
  -MemorySoftUsedPercent 85 `
  -MemoryHardUsedPercent 92 `
  -HardPressureAction pause `
  -MonitorAutoResume $true `
  -MonitorSnapshotJsonl temp/async/kani_remote/monitor_snapshots.jsonl
```

5. Stop deferred jobs:

```powershell
# reconcile stale/dead envelopes only (does not kill active lanes)
./scripts/run-formal-kani-remote.ps1 -Action StopDeferred -StopMode stale

# stop all active deferred lane/dispatch/kani processes under the remote base
./scripts/run-formal-kani-remote.ps1 -Action StopDeferred -StopMode all
```

6. Tail latest dispatch output (and optional lane log):

```powershell
./scripts/run-formal-kani-remote.ps1 -Action Tail -TailLines 120
./scripts/run-formal-kani-remote.ps1 -Action Tail -Lane v106-kani -TailLines 120
```

7. Fetch packed remote artifacts locally:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action FetchArtifacts -LocalArtifactsDir temp/async/kani_remote
```

8. Preferred drift-control entrypoint (one-shot reconcile + guarded start):

```powershell
./scripts/run-formal-kani-sync.ps1
```

Optional targeted queue:

```powershell
./scripts/run-formal-kani-sync.ps1 -VersionList "2 4 162 175 287" -DeferredConcurrency 2
```

## Remote Layout

Under `/home/ubuntu/.dnacalc_remote`:

- `bin/` remote wrappers (bootstrap, queue dispatch, status, fetch).
- `state/jobs/` async job envelopes (`run.log`, `exit_code`, `pid`, metadata).
- `state/deferred_lanes/` per-lane formal outputs (`formal_lane.csv`, `.md`, `.json`, obligation logs).
- `state/deferred_lanes/` per-lane formal outputs (`formal_lane.csv`, `.md`, `.json`, obligation logs, `progress.json`, `summary.txt`, `status.txt`).
- `state/deferred_dispatch/` queue-level metadata and dispatch logs.
- `state/dedup/` cross-lane command cache (`<hash>.json` + `<hash>.log`) keyed by commit/toolchain/command.
- `artifacts/` packaged bundles for retrieval.
- `work/OxVba` mirrored repository checkout used for formal execution.

## Runtime Telemetry

- `Status` now includes per-lane:
  - `phase`
  - `progress` (`completed_count/selected_count`)
  - `current` obligation id
  - `status` marker (`running` / `completed:*`)
  - `log_bytes`
  - `progress_age_s` (age of latest heartbeat/progress update)
- `Status` now classifies empty lane selections as explicit `no-op` with
  `warning=probable-commit-obligation-mismatch` instead of treating them as
  silent successes.
- `Status` adds local-side health warnings:
  - `warning=dispatch-commit-drift` when running dispatch commit differs from local head.
  - `warning=lane-no-progress-threshold` when a running lane shows no completed obligations beyond configured threshold.
  - `warning=no-op-lane-count` summary for probable obligation-mismatch lanes.
- `reconcile-formal-deferred-gates.ps1` reads remote lane states directly and
  updates DG rows from mutable states (`dg-not-started`/`dg-deferred`/`dg-fail`/`dg-running`)
  to reduce long-lived local-vs-remote drift.
- `Status` includes `resource_snapshot` fields (`mem_used_percent`, swap/load, `cbmc_count`, `kani_count`) and active pause-flag visibility.
- `Status` job rows classify detached wrappers explicitly as `running-detached` (instead of ambiguous `unknown`) when lane/dispatch workers are still live.
- `Status` prints `status_summary` counters (`running`, `pending`, `finished_pass`, `finished_fail`, `finished_unknown`, `no_op`) to make live queue health/rate visible at a glance.
- `StopDeferred -StopMode stale` marks stale/unknown deferred envelopes with explicit terminal state (`exit_code=143`, `completed:stopped`) so status no longer shows ambiguous `unknown` for dead runs.
- `Tail -Lane <name>` includes:
  - `run.log` (streamed command output)
  - `driver.log` (lane wrapper output)
  - `summary.txt`
  - `progress.json`

## Capacity Policy

The remote probe uses a conservative no-swap heuristic:

- worker cap by CPU: `nproc / 8`
- worker cap by memory: `MemAvailableGiB / 24`
- choose minimum of the two
- clamp to `[1..4]`

This keeps CBMC/Kani resource pressure bounded while preserving forward progress.

## Memory Guardrails

- `StartDeferred` accepts:
  - `-MemorySoftUsedPercent` (default `85`)
  - `-MemoryHardUsedPercent` (default `92`)
  - `-HardPressureAction` (`pause` | `halt-one` | `halt-all` | `none`; default `pause`)
- Dispatch behavior:
  - if used memory is above soft threshold, lane starts pause until memory recovers;
  - if hard threshold is reached, configured hard action is applied;
  - automatic pause file: `state/deferred_dispatch/PAUSE_NEW_LANES.auto`;
  - optional manual pause file: `state/deferred_dispatch/PAUSE_NEW_LANES.manual`.
- `Monitor` action can enforce the same thresholds out-of-band and auto-resume the auto-pause flag when memory drops below soft threshold.
- `Monitor` now prints `monitor_summary` per sample and can append machine-readable telemetry (`-MonitorSnapshotJsonl`) for trend/rate analysis.
- `Status` supports configurable no-progress warning threshold:
  - `-StatusStalledMinutesWarn` (default `90`).

## Timeout and Dedup

- Per-obligation base timeout is configurable with `-ObligationTimeoutSeconds` (default `10800`).
- Timeout retries are configurable:
  - `-ObligationTimeoutRetries` (default `1`)
  - `-ObligationTimeoutMultiplier` (default `10`)
- Scheduling policy:
  - run all obligations once with base timeout;
  - obligations that timeout are moved to an end-of-queue retry pass;
  - each retry round uses `base_timeout * multiplier^retry_round`.
- Dedup strategy (`-DeferredStrategy dedup`) avoids recomputing identical Kani commands across lanes for the same commit/toolchain/timeout tuple.
- Lane CSV now records `initial_status`, `attempts`, `attempt`, `retry_round`, and `timeout_seconds` to distinguish first-pass timeouts from final outcomes.
- Lane completion semantics:
  - `completed:pass`: obligations selected and all passed.
  - `completed:fail`: one or more obligations failed/timed out.
  - `completed:no-op`: `selected_count=0` (treated as mismatch/no-op, not as a passing gate).
