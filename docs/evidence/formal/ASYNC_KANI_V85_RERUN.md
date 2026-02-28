# ASYNC_KANI_V85_RERUN.md

## Active Run
- Run name: `v85-kani-rerun`
- Profile scope: `mvp-typed-execution-fastpaths-v85`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-typed-execution-fastpaths-v85 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v85-kani-rerun -ProfileScope mvp-typed-execution-fastpaths-v85 -WatchPollSeconds 600`

## Hardening Signals
- Preflight artifact: `temp/async/formal-kani/v85-kani-rerun/preflight.json`
- Current probe summary:
  - `uses_wsl_kani = true`
  - `wsl_kani_available = true`
  - `wsl_kani_version = cargo-kani 0.67.0`
- Status snapshot artifact: `temp/async/formal-kani/v85-kani-rerun/status_snapshot.json`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v85-kani-rerun/liveness.log`

## Runtime Paths
- State: `temp/async/formal-kani/v85-kani-rerun/state.json`
- Stdout log: `temp/async/formal-kani/v85-kani-rerun/stdout.log`
- Stderr log: `temp/async/formal-kani/v85-kani-rerun/stderr.log`
- Exit code file: `temp/async/formal-kani/v85-kani-rerun/exit_code.txt`

## Policy Context
- Historical failed lane remains recorded as `DG-V85-001`.
- Active rerun lane is tracked as `DG-V85-002` in `docs/evidence/formal/DEFERRED_GATES.md`.
