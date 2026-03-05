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
- ID: FTODO-V132-001
  Profile: v120..v134 formal lanes
  Summary: New strict Kani lanes for conversion/introspection/built-in expansion/file-stub subsets were not started locally; they are queued as remote Linux deferred gates (`DG-V120-001`, `DG-V126-001`, `DG-V132-001`, `DG-V134-001`).
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "120,126,132,134" -DeferredMode cumulative`
  Suggested next action: launch remote async lanes, poll completion, and fold results into `latest_run.*` at the next foldback checkpoint.
- ID: FTODO-V146-001
  Profile: v146 terminal formal lane
  Summary: Terminal strict lane `DG-V146-001` remains deferred pending remote Linux capacity and batching policy.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "146" -DeferredMode cumulative`
  Suggested next action: run terminal strict lane remotely during final v146 gate reconciliation and fold status into `DEFERRED_GATES.md`.
- ID: FTODO-V175-001
  Profile: v175 formal lane expansion I
  Summary: New strict harnesses (`cverr_tag_encoding_stays_in_reserved_error_band`, `resume_next_clears_err_number_after_raise`) were added but not yet executed in strict Kani mode.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "175" -DeferredMode cumulative`
  Suggested next action: dispatch lane `v175-kani` on remote Linux and fold completion status into `DEFERRED_GATES.md` + `latest_run.*`.
- ID: FTODO-V176-001
  Profile: v176 formal lane expansion II
  Summary: Strict foldback lane for the v175/v176 formal expansion tranche remains deferred to remote Linux batch execution.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "176" -DeferredMode cumulative`
  Suggested next action: execute `v176-kani` remotely and reconcile deferred register + formal summaries at the v186 terminal closure sweep.
- ID: FTODO-V287-001
  Profile: v287 PMR/Declare formal lane setup
  Summary: New strict harnesses for PMR project-graph invariants and Declare descriptor-contract checks were executed in pinned remote lane with non-empty selection (`selected_count=3`); host PMR harnesses timed out while HAL dynlink harness passed.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredVersions "287" -DeferredMode exact`
  Suggested next action: perform host-harness timeout remediation for `FO-V287-001/002` (slicing/assumption/bounds), rerun remote `v287` lane, and only then reopen bridge-retirement decision.
- ID: FTODO-KANI-REVIEW-001
  Profile: cross-profile (`v87+` remote deferred lanes)
  Summary: Multiple deferred lanes report `selected_count=0` / `completed:no-op` with `probable-commit-obligation-mismatch`; this is a runner-input mismatch class, not a proof result.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action Status`
  Suggested next action: add an explicit lane-to-obligation selection preflight and fail fast when selected count is zero unexpectedly.
- ID: FTODO-KANI-REVIEW-002
  Profile: high-signal harnesses (`FO-V2-001`, `FO-V4-001`, `FO-V287-001`, `FO-V287-002`)
  Summary: Strict Kani runs are dominated by timeout/OOM from broad symbolic state (`unicode`/`memchr`/host-heavy harnesses).
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action Tail -Lane v89-kani -TailLines 120`
  Suggested next action: slice high-signal harnesses into smaller bounded proofs with focused assumptions and/or stubs.
- ID: FTODO-KANI-REVIEW-003
  Profile: remote runner control plane
  Summary: Memory guardrails are now implemented (`soft`/`hard` thresholds + `pause`/`halt-*` actions), but need burn-in and threshold tuning under real queue load.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action Monitor -MonitorDurationSeconds 600 -MonitorIntervalSeconds 30`
  Suggested next action: run 24h monitor sample, record pressure/action incidence, then tune defaults for this host.
- ID: FTODO-KANI-REVIEW-004
  Profile: remote deferred queue continuity (`v2/v4/v162/v175/v287`)
  Summary: Active deferred dispatch is currently executing on commit `560e5a0` while local head has advanced; this is acceptable for now but requires explicit post-run reconcile/restart on latest commit to avoid prolonged drift.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-remote.ps1 -Action Status`
  Suggested next action: keep current run until terminal state, then run `./scripts/run-formal-kani-sync.ps1` to reconcile and restart unresolved lanes against latest head.
