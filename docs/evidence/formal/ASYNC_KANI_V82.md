# ASYNC_KANI_V82.md

## Active Run
- Run name: `v82-kani`
- Profile scope: `mvp-array-redim-full-v82`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-array-redim-full-v82 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v82-kani -ProfileScope mvp-array-redim-full-v82 -StartWatcher $true -WatchPollSeconds 600`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v82-kani/liveness.log`

## Runtime Paths
- State: `temp/async/formal-kani/v82-kani/state.json`
- Stdout log: `temp/async/formal-kani/v82-kani/stdout.log`
- Stderr log: `temp/async/formal-kani/v82-kani/stderr.log`
- Exit code file: `temp/async/formal-kani/v82-kani/exit_code.txt`

## Policy Context
- This strict Kani lane is tracked as a deferred formal gate (`DG-V82-001`) in `docs/evidence/formal/DEFERRED_GATES.md`.
- Local non-blocking formal evidence remains in `docs/evidence/formal/latest_run.md` while strict async execution proceeds.
