# ASYNC_KANI_V78.md

## Active Run
- Run name: `v78-kani`
- Profile scope: `mvp-string-compare-search-v78`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-string-compare-search-v78 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v78-kani -ProfileScope mvp-string-compare-search-v78 -StartWatcher $true -WatchPollSeconds 600`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v78-kani/liveness.log`

## Runtime Paths
- State: `temp/async/formal-kani/v78-kani/state.json`
- Stdout log: `temp/async/formal-kani/v78-kani/stdout.log`
- Stderr log: `temp/async/formal-kani/v78-kani/stderr.log`
- Exit code file: `temp/async/formal-kani/v78-kani/exit_code.txt`

## Policy Context
- This strict Kani lane is tracked as a deferred formal gate (`DG-V78-001`) in `docs/evidence/formal/DEFERRED_GATES.md`.
- Local non-blocking formal evidence remains in `docs/evidence/formal/latest_run.md` while strict async execution proceeds.
