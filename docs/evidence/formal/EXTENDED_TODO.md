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
  Summary: `v73-kani` completed and was folded; `v74-kani` and `v75-kani` remain unresolved and are explicitly deferred in `DG_AUDIT_V86.md`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v73-kani` (repeat for `v74-kani`, `v75-kani`)
  Suggested next action: follow `DGD-V86-001` unblock flow in `DG_AUDIT_V86.md` and fold rows when completed.
- ID: FTODO-V79-001
  Profile: v79 reconciliation (`v77..v78` DG foldback)
  Summary: Deferred-gate strict runs `v77-kani` and `v78-kani` are unresolved and explicitly deferred in `DG_AUDIT_V86.md`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v77-kani` (repeat for `v78-kani`)
  Suggested next action: follow `DGD-V86-001` unblock flow in `DG_AUDIT_V86.md`.
- ID: FTODO-V84-001
  Profile: v84 reconciliation (`v80..v83` DG foldback)
  Summary: Deferred-gate strict runs `v80-kani` through `v83-kani` are unresolved and explicitly deferred in `DG_AUDIT_V86.md`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v80-kani` (repeat for `v81-kani`..`v83-kani`)
  Suggested next action: follow `DGD-V86-001` unblock flow in `DG_AUDIT_V86.md`.
- ID: FTODO-V86-001
  Profile: v86 terminal reconciliation
  Summary: DG rows `DG-V74-001`, `DG-V75-001`, `DG-V77-001`, `DG-V78-001`, `DG-V79-001`, `DG-V80-001`, `DG-V81-001`, `DG-V82-001`, and `DG-V83-001` remain unresolved and are marked `dg-deferred`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name <run-name>`
  Suggested next action: execute `DGD-V86-001` steps in `docs/evidence/formal/DG_AUDIT_V86.md` until all rows can be folded.
- ID: FTODO-V86-002
  Profile: v86 terminal reconciliation
  Summary: `DG-V85-001` failed quickly (`exit_code=1`) with WSL Kani detection error and was folded with explicit triage.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `Get-Content temp/async/formal-kani/v85-kani/stderr.log -Tail 40`
  Suggested next action: execute `DGD-V86-002` restart steps in `docs/evidence/formal/DG_AUDIT_V86.md` and update DG register row after rerun.
