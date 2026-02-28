# Deferred Formal Gates

This register tracks long-running async formal obligations that are started during profile execution and reconciled later.

## Status Legend
- `dg-started`: async run created and state/log paths recorded.
- `dg-running`: process observed live after start.
- `dg-pass`: async run completed and formal obligations passed.
- `dg-fail`: async run completed with one or more failing obligations.
- `dg-folded`: completion status merged into formal reports and backlog triage done.

## Register

| DG ID | Profile | Run Name | Status | Started UTC | Foldback Profile | Paths | Notes |
|---|---|---|---|---|---|---|---|
| DG-V56-001 | v56 | v56-kani-full | dg-folded | 2026-02-28T05:14:36Z | v66 | `temp/async/formal-kani/v56-kani-full/` | Historical reference run; foldback evidence in `docs/evidence/formal/ASYNC_KANI_V56.md`. |
| DG-V67-001 | v67 | v67-kani | dg-running | 2026-02-28T09:50:05Z | v72 | `temp/async/formal-kani/v67-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-type-lattice-v67`. |
| DG-V68-001 | v68 | v68-kani | dg-running | 2026-02-28T10:00:35Z | v72 | `temp/async/formal-kani/v68-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-option-explicit-diagnostics-v68`. |
| DG-V69-001 | v69 | v69-kani | dg-running | 2026-02-28T10:20:12Z | v72 | `temp/async/formal-kani/v69-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-default-type-rules-v69`. |
| DG-V70-001 | v70 | v70-kani | dg-running | 2026-02-28T10:28:49Z | v72 | `temp/async/formal-kani/v70-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-procedure-signatures-v70`. |
| DG-V71-001 | v71 | v71-kani | dg-running | 2026-02-28T10:37:16Z | v72 | `temp/async/formal-kani/v71-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-early-late-classification-v71`. |
| DG-V72-001 | v72 | v72-kani | dg-running | 2026-02-28T10:44:16Z | v76 | `temp/async/formal-kani/v72-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-diagnostic-rollup-v72`. |
| DG-V73-001 | v73 | v73-kani | dg-running | 2026-02-28T10:54:14Z | v76 | `temp/async/formal-kani/v73-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-coercion-matrix-v73`. |
| DG-V74-001 | v74 | v74-kani | dg-running | 2026-02-28T11:14:16Z | v76 | `temp/async/formal-kani/v74-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-operator-result-rules-v74`. |
| DG-V75-001 | v75 | v75-kani | dg-running | 2026-02-28T11:25:35Z | v76 | `temp/async/formal-kani/v75-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-typing-call-coercion-early-late-v75`. |
| DG-V77-001 | v77 | v77-kani | dg-running | 2026-02-28T11:50:22Z | v79 | `temp/async/formal-kani/v77-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-string-storage-semantics-v77`. |
| DG-V78-001 | v78 | v78-kani | dg-running | 2026-02-28T12:19:21Z | v79 | `temp/async/formal-kani/v78-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-string-compare-search-v78`. |
| DG-V79-001 | v79 | v79-kani | dg-running | 2026-02-28T12:32:24Z | v84 | `temp/async/formal-kani/v79-kani/` | Started with watcher polling (`600s`) for strict WSL Kani run of `mvp-string-mutation-and-slices-v79`. |

## Update Protocol
1. On async start, add a row with `dg-started` and paths.
2. After first successful liveness poll, move to `dg-running`.
3. On completion, set `dg-pass` or `dg-fail` and include exit status notes.
4. During planned reconciliation profile, merge results into:
   - `docs/evidence/formal/latest_run.md`
   - `docs/evidence/formal/latest_run.csv`
   - `docs/evidence/formal/EXTENDED_TODO.md` (for unresolved failures)
5. Mark the row `dg-folded` when foldback is complete.
