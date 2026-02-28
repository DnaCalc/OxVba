# Deferred Formal Gates

This register tracks long-running async formal obligations that are started during profile execution and reconciled later.

## Status Legend
- `dg-started`: async run created and state/log paths recorded.
- `dg-running`: process observed live after start.
- `dg-pass`: async run completed and formal obligations passed.
- `dg-fail`: async run completed with one or more failing obligations.
- `dg-deferred`: unresolved at reconciliation point; explicitly deferred with unblock steps.
- `dg-folded`: completion status merged into formal reports and backlog triage done.

## Register

| DG ID | Profile | Run Name | Status | Started UTC | Foldback Profile | Paths | Notes |
|---|---|---|---|---|---|---|---|
| DG-V56-001 | v56 | v56-kani-full | dg-folded | 2026-02-28T05:14:36Z | v66 | `temp/async/formal-kani/v56-kani-full/` | Historical reference run; foldback evidence in `docs/evidence/formal/ASYNC_KANI_V56.md`. |
| DG-V67-001 | v67 | v67-kani | dg-folded | 2026-02-28T09:50:05Z | v72 | `temp/async/formal-kani/v67-kani/` | Completed `pass` at `2026-02-28T13:20:17Z` (exit `0`); folded in v86 audit. |
| DG-V68-001 | v68 | v68-kani | dg-folded | 2026-02-28T10:00:35Z | v72 | `temp/async/formal-kani/v68-kani/` | Completed `pass` at `2026-02-28T11:57:22Z` (exit `0`); folded in v86 audit. |
| DG-V69-001 | v69 | v69-kani | dg-folded | 2026-02-28T10:20:12Z | v72 | `temp/async/formal-kani/v69-kani/` | Completed `pass` at `2026-02-28T13:55:41Z` (exit `0`); folded in v86 audit. |
| DG-V70-001 | v70 | v70-kani | dg-folded | 2026-02-28T10:28:49Z | v72 | `temp/async/formal-kani/v70-kani/` | Completed `pass` at `2026-02-28T12:25:58Z` (exit `0`); folded in v86 audit. |
| DG-V71-001 | v71 | v71-kani | dg-folded | 2026-02-28T10:37:16Z | v72 | `temp/async/formal-kani/v71-kani/` | Completed `pass` at `2026-02-28T14:05:11Z` (exit `0`); folded in v86 audit. |
| DG-V72-001 | v72 | v72-kani | dg-folded | 2026-02-28T10:44:16Z | v76 | `temp/async/formal-kani/v72-kani/` | Completed `pass` at `2026-02-28T14:15:54Z` (exit `0`); folded in v86 audit. |
| DG-V73-001 | v73 | v73-kani | dg-folded | 2026-02-28T10:54:14Z | v76 | `temp/async/formal-kani/v73-kani/` | Completed `pass` at `2026-02-28T14:38:18Z` (exit `0`); folded in v86 audit. |
| DG-V74-001 | v74 | v74-kani | dg-folded | 2026-02-28T11:14:16Z | v86 | `temp/async/formal-kani/v74-kani/` | Completed `pass` at `2026-02-28T15:03:49Z` (exit `0`); folded during post-v86 hardening housekeeping. |
| DG-V75-001 | v75 | v75-kani | dg-folded | 2026-02-28T11:25:35Z | v86 | `temp/async/formal-kani/v75-kani/` | Completed `pass` at `2026-02-28T15:18:22Z` (exit `0`); folded during post-v86 hardening housekeeping. |
| DG-V77-001 | v77 | v77-kani | dg-folded | 2026-02-28T11:50:22Z | v86 | `temp/async/formal-kani/v77-kani/` | Completed `pass` at `2026-02-28T15:35:35Z` (exit `0`); folded during post-v86 hardening housekeeping. |
| DG-V78-001 | v78 | v78-kani | dg-folded | 2026-02-28T12:19:21Z | v86 | `temp/async/formal-kani/v78-kani/` | Completed `pass` at `2026-02-28T16:19:59Z` (exit `0`); folded during post-v86 hardening housekeeping. |
| DG-V79-001 | v79 | v79-kani | dg-deferred | 2026-02-28T12:32:24Z | v84 | `temp/async/formal-kani/v79-kani/` | Still running as of `2026-02-28T16:37:37Z`; reconcile via `run-formal-kani-async.ps1 -Action Status|Reconcile`. |
| DG-V80-001 | v80 | v80-kani | dg-deferred | 2026-02-28T12:44:34Z | v84 | `temp/async/formal-kani/v80-kani/` | Still running as of `2026-02-28T16:37:37Z`; reconcile via `run-formal-kani-async.ps1 -Action Status|Reconcile`. |
| DG-V81-001 | v81 | v81-kani | dg-deferred | 2026-02-28T13:12:13Z | v84 | `temp/async/formal-kani/v81-kani/` | Still running as of `2026-02-28T16:37:37Z`; reconcile via `run-formal-kani-async.ps1 -Action Status|Reconcile`. |
| DG-V82-001 | v82 | v82-kani | dg-deferred | 2026-02-28T13:27:54Z | v84 | `temp/async/formal-kani/v82-kani/` | Still running as of `2026-02-28T16:37:37Z`; reconcile via `run-formal-kani-async.ps1 -Action Status|Reconcile`. |
| DG-V83-001 | v83 | v83-kani | dg-deferred | 2026-02-28T13:47:52Z | v84 | `temp/async/formal-kani/v83-kani/` | Still running as of `2026-02-28T16:37:37Z`; reconcile via `run-formal-kani-async.ps1 -Action Status|Reconcile`. |
| DG-V85-001 | v85 | v85-kani | dg-folded | 2026-02-28T14:36:54Z | v86 | `temp/async/formal-kani/v85-kani/` | Completed `fail` at `2026-02-28T14:37:25Z` (exit `1`, WSL Kani detection failure); triaged in `DGD-V86-002` and followed by rerun lane `DG-V85-002`. |
| DG-V85-002 | v85 | v85-kani-rerun | dg-running | 2026-02-28T16:36:58Z | v86 | `temp/async/formal-kani/v85-kani-rerun/` | Async rerun started after hardening (`preflight.json` shows WSL Kani available); pending completion and foldback. |

## Update Protocol
1. On async start, add a row with `dg-started` and paths.
2. After first successful liveness poll, move to `dg-running`.
3. On completion, set `dg-pass` or `dg-fail` and include exit status notes.
4. During planned reconciliation profile, merge results into:
   - `docs/evidence/formal/latest_run.md`
   - `docs/evidence/formal/latest_run.csv`
   - `docs/evidence/formal/EXTENDED_TODO.md` (for unresolved failures)
5. Mark the row `dg-folded` when foldback is complete.
6. If a lane remains unresolved at a terminal gate, mark `dg-deferred` and link explicit unblock steps.
