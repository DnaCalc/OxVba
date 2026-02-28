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
  -DeferredConcurrency 2 `
  -DeferredVersions "81 82 83 87 88 89 90 91 93 94 95 96 99 100 101 102 103 104 105 106"
```

4. Show running jobs and deferred lane summary:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action Status
```

5. Tail latest dispatch output (and optional lane log):

```powershell
./scripts/run-formal-kani-remote.ps1 -Action Tail -TailLines 120
./scripts/run-formal-kani-remote.ps1 -Action Tail -Lane v106-kani -TailLines 120
```

6. Fetch packed remote artifacts locally:

```powershell
./scripts/run-formal-kani-remote.ps1 -Action FetchArtifacts -LocalArtifactsDir temp/async/kani_remote
```

## Remote Layout

Under `/home/ubuntu/.dnacalc_remote`:

- `bin/` remote wrappers (bootstrap, queue dispatch, status, fetch).
- `state/jobs/` async job envelopes (`run.log`, `exit_code`, `pid`, metadata).
- `state/deferred_lanes/` per-lane formal outputs (`formal_lane.csv`, `.md`, `.json`, obligation logs).
- `state/deferred_dispatch/` queue-level metadata and dispatch logs.
- `artifacts/` packaged bundles for retrieval.
- `work/OxVba` mirrored repository checkout used for formal execution.

## Capacity Policy

The remote probe uses a conservative no-swap heuristic:

- worker cap by CPU: `nproc / 8`
- worker cap by memory: `MemAvailableGiB / 24`
- choose minimum of the two
- clamp to `[1..4]`

This keeps CBMC/Kani resource pressure bounded while preserving forward progress.
