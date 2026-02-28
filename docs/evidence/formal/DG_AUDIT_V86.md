# DG_AUDIT_V86.md

## Scope
Final deferred-gate reconciliation snapshot for typing ladder terminal profile `v86` (`mvp-full-typing-conformance-gate-v86`).

Timestamp (UTC): `2026-02-28T14:45:00Z`

## Folded Strict Runs (Completed)
- `DG-V67-001` through `DG-V73-001`: completed with `exit_code=0` and folded in `DEFERRED_GATES.md`.
- `DG-V85-001`: completed with `exit_code=1` and folded with failure triage.

## Explicit Deferred Lanes
- `DGD-V86-001`:
  - DG rows: `DG-V74-001`, `DG-V75-001`, `DG-V77-001`, `DG-V78-001`, `DG-V79-001`, `DG-V80-001`, `DG-V81-001`, `DG-V82-001`, `DG-V83-001`.
  - Current state: async lanes remain live at v86 cutoff (`status=running`).
  - Reason: long-running strict Kani jobs exceed ladder completion window.
  - Unblocking steps:
    1. Poll each lane:
       - `./scripts/run-formal-kani-async.ps1 -Action Status -Name <run-name>`
    2. If a lane is stuck for >6h with no stdout/stderr growth, stop and restart:
       - `./scripts/run-formal-kani-async.ps1 -Action Stop -Name <run-name>`
       - `./scripts/run-formal-kani-async.ps1 -Action Start -Name <run-name> -ProfileScope <scope> -StartWatcher $true -WatchPollSeconds 600`
    3. On completion, update `DEFERRED_GATES.md` row to `dg-pass|dg-fail`, fold into `latest_run.md/csv`, then set `dg-folded`.

- `DGD-V86-002`:
  - DG row: `DG-V85-001`.
  - Current state: completed with failure (`exit_code=1`).
  - Failure note: `formal lane: -UseWslKani requested but cargo-kani is unavailable in WSL` (from `temp/async/formal-kani/v85-kani/stderr.log`).
  - Unblocking steps:
    1. Verify WSL toolchain visibility:
       - `wsl bash -lc 'source $HOME/.cargo/env && cargo kani --version'`
    2. Restart strict run once WSL probe is stable:
       - `./scripts/run-formal-kani-async.ps1 -Action Start -Name v85-kani-rerun -ProfileScope mvp-typed-execution-fastpaths-v85 -StartWatcher $true -WatchPollSeconds 600`
    3. Record rerun outcome in `DEFERRED_GATES.md` and update `EXTENDED_TODO.md`.

## Notes
- This audit satisfies terminal-gate requirement that non-folded DG rows are explicitly deferred with concrete unblock steps.

## Post-v86 Addendum (2026-02-28T16:37:37Z)
- Folded since cutoff:
  - `DG-V74-001`: completed `pass` at `2026-02-28T15:03:49Z` (exit `0`)
  - `DG-V75-001`: completed `pass` at `2026-02-28T15:18:22Z` (exit `0`)
  - `DG-V77-001`: completed `pass` at `2026-02-28T15:35:35Z` (exit `0`)
  - `DG-V78-001`: completed `pass` at `2026-02-28T16:19:59Z` (exit `0`)
- Remaining unresolved from `DGD-V86-001`: `DG-V79-001`, `DG-V80-001`, `DG-V81-001`, `DG-V82-001`, `DG-V83-001`.
- `DGD-V86-002` follow-up:
  - A rerun lane was started with hardened preflight checks:
    - `./scripts/run-formal-kani-async.ps1 -Action Start -Name v85-kani-rerun -ProfileScope mvp-typed-execution-fastpaths-v85 -WatchPollSeconds 600`
  - Register row: `DG-V85-002` (`dg-running`).
