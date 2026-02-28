# ASYNC_KANI_V67.md

## Active Run
- Run name: `v67-kani`
- Profile scope: `mvp-typing-type-lattice-v67`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-typing-type-lattice-v67 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v67-kani -ProfileScope mvp-typing-type-lattice-v67 -StartWatcher $true -WatchPollSeconds 600`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v67-kani/liveness.log`

## Runtime Paths
- State: `temp/async/formal-kani/v67-kani/state.json`
- Stdout log: `temp/async/formal-kani/v67-kani/stdout.log`
- Stderr log: `temp/async/formal-kani/v67-kani/stderr.log`
- Exit code file: `temp/async/formal-kani/v67-kani/exit_code.txt`

## Policy Context
- This strict Kani lane is tracked as a deferred formal gate (`DG-V67-001`) in `docs/evidence/formal/DEFERRED_GATES.md`.
- Local non-blocking formal evidence remains in `docs/evidence/formal/latest_run.md` while strict async execution proceeds.
