# WORKSET_2026-02-28_FORMAL_ASYNC_HARDENING_V57.md

## Purpose
Execute profile `v57` (`mvp-formal-async-hardening-v57`) as the first step of the `v57..v66` ladder.

## Scope
- Harden async formal orchestration so long-running Kani lanes are unattended and observable.
- Provide resilient liveness polling with explicit watcher lifecycle controls.
- Record evidence and status under `v57`.

## Implementation Targets
- `scripts/run-formal-kani-async.ps1`
- `scripts/watch-formal-kani-async.ps1`
- `docs/evidence/formal/ASYNC_KANI_V56.md` (operational policy notes)
- `docs/profile-status/PROFILE_STATUS_V57.md`

## Validation Commands
```powershell
cargo test -p oxvba-host --lib
./scripts/run-formal.ps1 -ProfileScope mvp-formal-async-hardening-v57
./scripts/run-matrix.ps1 -ProfileScope mvp-formal-async-hardening-v57 -OutputDir docs/evidence/profiles/v57
```

## Closure Signals
`v57` closes when:
- async runner exposes watcher start/stop controls,
- watcher emits stable liveness lines until completion,
- FO-V57-* obligations pass in the non-blocking formal lane,
- profile matrix report for `v57` is green.
