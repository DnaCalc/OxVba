# ASYNC_KANI_V56.md

## Active Run
- Run name: `v56-kani-full`
- Profile scope: `mvp-language-stdlib-consolidation-gate-v56`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-language-stdlib-consolidation-gate-v56 -RequireKani -UseWslKani`
- Started via: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v56-kani-full -ProfileScope mvp-language-stdlib-consolidation-gate-v56 -StartWatcher $true -WatchPollSeconds 600`

## Liveness Polling
- Poller script: `scripts/watch-formal-kani-async.ps1`
- Poll cadence: `600s` (10 minutes)
- Poller launch mode: background hidden process
- Liveness log: `temp/async/formal-kani/v56-kani-full/liveness.log`
- Watcher control actions:
  - `./scripts/run-formal-kani-async.ps1 -Action WatchStart -Name v56-kani-full -WatchPollSeconds 600`
  - `./scripts/run-formal-kani-async.ps1 -Action WatchStop -Name v56-kani-full`

## Runtime Paths
- State: `temp/async/formal-kani/v56-kani-full/state.json`
- Stdout log: `temp/async/formal-kani/v56-kani-full/stdout.log`
- Stderr log: `temp/async/formal-kani/v56-kani-full/stderr.log`
- Exit code file: `temp/async/formal-kani/v56-kani-full/exit_code.txt`

## Policy Context
- This strict Kani lane is non-blocking for the current ladder per `AGENTS.md` policy.
- Local formal gate remains tracked in `docs/evidence/formal/latest_run.md` while strict async Kani progresses independently.
