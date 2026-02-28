# Formal Extended Todo

Non-blocking formal issues and follow-up items for later ladder profiles.

## Template
- ID:
- Profile:
- Summary:
- Current status (`todo` / `investigating` / `resolved`):
- Reproduction command:
- Suggested next action:

## Active Items
- ID: FTODO-V2-001
  Profile: v2-v4
  Summary: Native Windows `cargo-kani` remains unavailable; strict Kani is routed through WSL lanes.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `cargo kani --version` (Windows), `wsl bash -lc 'source $HOME/.cargo/env && cargo kani --version'` (WSL)
  Suggested next action: keep strict lanes on WSL path and avoid relying on native Windows Kani for profile gating.
- ID: FTODO-V72-001
  Profile: v72 reconciliation (`v67..v71` DG foldback)
  Summary: Deferred-gate strict Kani runs `v67-kani` through `v71-kani` completed and were folded during v86 reconciliation.
  Current status (`todo` / `investigating` / `resolved`): resolved
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v67-kani` (repeat for `v68-kani`..`v71-kani`)
  Suggested next action: none.
- ID: FTODO-V76-001
  Profile: v76 reconciliation (`v73..v75` DG foldback)
  Summary: `v73-kani`, `v74-kani`, and `v75-kani` have completed and are now folded in `DEFERRED_GATES.md`.
  Current status (`todo` / `investigating` / `resolved`): resolved
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v73-kani` (repeat for `v74-kani`, `v75-kani`)
  Suggested next action: none.
- ID: FTODO-V79-001
  Profile: v79 reconciliation (`v77..v78` DG foldback)
  Summary: Deferred-gate strict runs `v77-kani` and `v78-kani` completed with `exit=0` and are folded.
  Current status (`todo` / `investigating` / `resolved`): resolved
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v77-kani` (repeat for `v78-kani`)
  Suggested next action: none.
- ID: FTODO-V84-001
  Profile: v84 reconciliation (`v80..v83` DG foldback)
  Summary: Post-crash status sweep shows `v80-kani` completed pass, while `v81-kani`..`v83-kani` are `stale` (stopped runners/watchers without completion markers).
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v80-kani` (repeat for `v81-kani`..`v83-kani`)
  Suggested next action: fold pass for `v80-kani`; rerun `v81..v83` on remote Linux host (no local restart per current policy).
- ID: FTODO-V86-001
  Profile: v86 terminal reconciliation
  Summary: DG rows `DG-V79-001`, `DG-V80-001`, `DG-V81-001`, `DG-V82-001`, and `DG-V83-001` remain unresolved and are marked `dg-deferred`; `DG-V74/75/77/78` are now folded.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name <run-name>`
  Suggested next action: execute `DGD-V86-001` steps in `docs/evidence/formal/DG_AUDIT_V86.md` for remaining `v79..v83` lanes only.
- ID: FTODO-V86-002
  Profile: v86 terminal reconciliation
  Summary: `DG-V85-001` failed quickly (`exit_code=1`) with WSL Kani detection error; rerun lane `DG-V85-002` completed pass (`exit_code=0`) in post-crash status sweep.
  Current status (`todo` / `investigating` / `resolved`): resolved
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v85-kani-rerun`
  Suggested next action: fold completion into formal summary artifacts.
- ID: FTODO-V94-001
  Profile: v87..v94 formal lanes
  Summary: New strict Kani lanes for the current language-closure tranche were intentionally not started locally after crash/recovery due host resource-risk concerns.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v87-kani -ProfileScope mvp-lang-for-step-v87` (repeat for `v88`, `v89`, `v90`, `v91`, `v93`, `v94`) on remote Linux host.
  Suggested next action: migrate async Kani execution to remote Linux runner; keep local lanes in `dg-not-started` until remote orchestration is validated.
- ID: FTODO-V99-001
  Profile: v95/v96/v99 formal lanes
  Summary: Additional strict Kani lanes (`v95-kani`, `v96-kani`, `v99-kani`) intentionally not started locally per post-crash resource policy.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v95-kani -ProfileScope mvp-lang-resume-full-v95` (repeat for `v96`, `v99`) on remote Linux host.
  Suggested next action: execute these lanes on remote Linux and fold results back via `DEFERRED_GATES.md` + `latest_run` reconciliation.
- ID: FTODO-V106-001
  Profile: v100..v106 formal lanes
  Summary: Language-closure tail strict lanes (`v100-kani`..`v106-kani`) intentionally not started locally per post-crash resource policy.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v100-kani -ProfileScope mvp-lang-udt-fields-v100` (repeat for `v101`, `v102`, `v103`, `v104`, `v105`, `v106`) on remote Linux host.
  Suggested next action: execute remote async Kani runs, update `DEFERRED_GATES.md`, and fold results into `latest_run.*` during post-v106 reconciliation.
- ID: FTODO-V107-001
  Profile: v107 formal lane
  Summary: `v107` strict Kani lane is deferred to remote Linux execution; local formal run completed with non-blocking Kani skips.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "107" -DeferredMode cumulative`
  Suggested next action: run remote async `v107-kani`, then fold status into `DEFERRED_GATES.md` and `latest_run.*`.
