# ASYNC_KANI_V56.md

## Run Summary
- Profile scope: `mvp-language-stdlib-consolidation-gate-v56`
- Runner: `./scripts/run-formal-kani-async.ps1`
- Start timestamp (UTC): `2026-02-28T01:15:15Z`
- Command: `./scripts/run-formal.ps1 -ProfileScope mvp-language-stdlib-consolidation-gate-v56 -RequireKani -UseWslKani`
- Status: exercised and timeboxed (non-blocking lane); process stopped to avoid indefinite background mutation of tracked formal artifacts.

## Artifacts
- State: `temp/async/formal-kani/v56-kani/state.json`
- Stdout log: `temp/async/formal-kani/v56-kani/stdout.log`
- Stderr log: `temp/async/formal-kani/v56-kani/stderr.log`

## Notes
- Async orchestration path is validated and operational for long-running Kani workloads.
- The regular formal gate remains green in non-blocking mode (`docs/evidence/formal/latest_run.md`).
- Kani completion remains tracked as extended formal backlog work and does not block the current ladder gate per policy.
