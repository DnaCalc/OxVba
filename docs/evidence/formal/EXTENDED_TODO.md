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
  Summary: Deferred-gate strict runs `v80-kani` through `v83-kani` remain live async lanes (still running at `2026-02-28T16:37:37Z`).
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v80-kani` (repeat for `v81-kani`..`v83-kani`)
  Suggested next action: continue async polling with `Status`; use `Reconcile` if runner exits without markers; restart only if logs remain unchanged beyond stall policy.
- ID: FTODO-V86-001
  Profile: v86 terminal reconciliation
  Summary: DG rows `DG-V79-001`, `DG-V80-001`, `DG-V81-001`, `DG-V82-001`, and `DG-V83-001` remain unresolved and are marked `dg-deferred`; `DG-V74/75/77/78` are now folded.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name <run-name>`
  Suggested next action: execute `DGD-V86-001` steps in `docs/evidence/formal/DG_AUDIT_V86.md` for remaining `v79..v83` lanes only.
- ID: FTODO-V86-002
  Profile: v86 terminal reconciliation
  Summary: `DG-V85-001` failed quickly (`exit_code=1`) with WSL Kani detection error; rerun lane `DG-V85-002` (`v85-kani-rerun`) is now active after async hardening/preflight.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v85-kani-rerun`
  Suggested next action: continue async run to completion; fold pass/fail outcome into `DEFERRED_GATES.md` and this register.
