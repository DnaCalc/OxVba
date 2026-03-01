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
  -DeferredConcurrency 3 `
  -ObligationTimeoutSeconds 10800 `
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
- `Status` job rows classify detached wrappers explicitly as `running-detached` (instead of ambiguous `unknown`) when lane/dispatch workers are still live.
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

## Timeout and Dedup

- Per-obligation timeout is configurable with `-ObligationTimeoutSeconds` (default `10800`).
- Dedup strategy (`-DeferredStrategy dedup`) avoids recomputing identical Kani commands across lanes for the same commit/toolchain.
