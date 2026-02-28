# Deferred Formal Gates

This register tracks long-running async formal obligations that are started during profile execution and reconciled later.

## Status Legend
- `dg-started`: async run created and state/log paths recorded.
- `dg-running`: process observed live after start.
- `dg-pass`: async run completed and formal obligations passed.
- `dg-fail`: async run completed with one or more failing obligations.
- `dg-deferred`: unresolved at reconciliation point; explicitly deferred with unblock steps.
- `dg-not-started`: lane intentionally not started on current machine; deferred pending approved execution environment.
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
| DG-V79-001 | v79 | v79-kani | dg-pass | 2026-02-28T12:32:24Z | v84 | `temp/async/formal-kani/v79-kani/` | Status check after crash recovery shows `completed` at `2026-02-28T16:49:58Z` (`exit_code=0`); foldback still pending. |
| DG-V80-001 | v80 | v80-kani | dg-pass | 2026-02-28T12:44:34Z | v84 | `temp/async/formal-kani/v80-kani/` | Status check after crash recovery shows `completed` at `2026-02-28T18:39:07Z` (`exit_code=0`); foldback still pending. |
| DG-V81-001 | v81 | v81-kani | dg-deferred | 2026-02-28T13:12:13Z | v84 | `temp/async/formal-kani/v81-kani/` | Status check after crash recovery reports `stale` (runner/watcher stopped unexpectedly). No local restart per policy; queue for remote Linux rerun. |
| DG-V82-001 | v82 | v82-kani | dg-deferred | 2026-02-28T13:27:54Z | v84 | `temp/async/formal-kani/v82-kani/` | Status check after crash recovery reports `stale` (runner/watcher stopped unexpectedly). No local restart per policy; queue for remote Linux rerun. |
| DG-V83-001 | v83 | v83-kani | dg-deferred | 2026-02-28T13:47:52Z | v84 | `temp/async/formal-kani/v83-kani/` | Status check after crash recovery reports `stale` (runner/watcher stopped unexpectedly). No local restart per policy; queue for remote Linux rerun. |
| DG-V85-001 | v85 | v85-kani | dg-folded | 2026-02-28T14:36:54Z | v86 | `temp/async/formal-kani/v85-kani/` | Completed `fail` at `2026-02-28T14:37:25Z` (exit `1`, WSL Kani detection failure); triaged in `DGD-V86-002` and followed by rerun lane `DG-V85-002`. |
| DG-V85-002 | v85 | v85-kani-rerun | dg-pass | 2026-02-28T16:36:58Z | v86 | `temp/async/formal-kani/v85-kani-rerun/` | Status check after crash recovery shows `completed` at `2026-02-28T19:34:55Z` (`exit_code=0`); foldback pending. |
| DG-V87-001 | v87 | v87-kani | dg-not-started | n/a | v92 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V88-001 | v88 | v88-kani | dg-not-started | n/a | v92 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V89-001 | v89 | v89-kani | dg-not-started | n/a | v92 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V90-001 | v90 | v90-kani | dg-not-started | n/a | v92 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V91-001 | v91 | v91-kani | dg-not-started | n/a | v92 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V93-001 | v93 | v93-kani | dg-not-started | n/a | v98 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V94-001 | v94 | v94-kani | dg-not-started | n/a | v98 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V95-001 | v95 | v95-kani | dg-not-started | n/a | v98 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V96-001 | v96 | v96-kani | dg-not-started | n/a | v98 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V99-001 | v99 | v99-kani | dg-not-started | n/a | v103 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V100-001 | v100 | v100-kani | dg-not-started | n/a | v103 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V101-001 | v101 | v101-kani | dg-not-started | n/a | v103 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V102-001 | v102 | v102-kani | dg-not-started | n/a | v103 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V103-001 | v103 | v103-kani | dg-not-started | n/a | v103 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V104-001 | v104 | v104-kani | dg-not-started | n/a | v106 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V105-001 | v105 | v105-kani | dg-not-started | n/a | v106 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |
| DG-V106-001 | v106 | v106-kani | dg-not-started | n/a | v106 | `remote/linux-pending` | Intentionally not started locally after crash/recovery and resource-risk review; queue for remote Linux execution handoff. |

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
7. If a lane is intentionally not started on the current machine, mark `dg-not-started` and include migration/unblock details in notes and `EXTENDED_TODO`.
