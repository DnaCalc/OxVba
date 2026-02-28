# ASYNC_KANI_V75.md

## Active Run
- Run name: `v75-kani`
- Profile scope: `mvp-typing-call-coercion-early-late-v75`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-typing-call-coercion-early-late-v75 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v75-kani -ProfileScope mvp-typing-call-coercion-early-late-v75 -StartWatcher $true -WatchPollSeconds 600`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v75-kani/liveness.log`

## Runtime Paths
- State: `temp/async/formal-kani/v75-kani/state.json`
- Stdout log: `temp/async/formal-kani/v75-kani/stdout.log`
- Stderr log: `temp/async/formal-kani/v75-kani/stderr.log`
- Exit code file: `temp/async/formal-kani/v75-kani/exit_code.txt`

## Policy Context
- This strict Kani lane is tracked as a deferred formal gate (`DG-V75-001`) in `docs/evidence/formal/DEFERRED_GATES.md`.
- Local non-blocking formal evidence remains in `docs/evidence/formal/latest_run.md` while strict async execution proceeds.
