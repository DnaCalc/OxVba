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
  Summary: `cargo-kani` is not installed in current environment, so FO-V2-001/002, FO-V3-001, and FO-V4-001 cannot execute yet.
  Current status (`todo` / `investigating` / `resolved`): todo
  Reproduction command: `cargo kani --version`
  Suggested next action: install `cargo-kani`, then rerun `./scripts/run-formal.ps1` and update manifest/report status.
- ID: FTODO-V72-001
  Profile: v72 reconciliation (`v67..v71` DG foldback)
  Summary: Deferred-gate strict Kani runs `v67-kani` through `v71-kani` are still live (`dg-running`) at v72 checkpoint and cannot yet be folded to `dg-pass|dg-fail`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v67-kani` (repeat for `v68-kani`..`v71-kani`)
  Suggested next action: continue periodic status polling; when any run completes, merge outcome into `latest_run.md/csv`, update `DEFERRED_GATES.md`, and mark foldback status.
- ID: FTODO-V76-001
  Profile: v76 reconciliation (`v73..v75` DG foldback)
  Summary: Deferred-gate strict Kani runs `v73-kani`, `v74-kani`, and `v75-kani` remain live (`dg-running`) at the v76 checkpoint and cannot yet be folded to `dg-pass|dg-fail`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v73-kani` (repeat for `v74-kani`, `v75-kani`)
  Suggested next action: continue periodic status polling; fold completed outcomes into `latest_run.md/csv`, update `DEFERRED_GATES.md`, and close the item once all three DG rows are folded.
- ID: FTODO-V79-001
  Profile: v79 reconciliation (`v77..v78` DG foldback)
  Summary: Deferred-gate strict Kani runs `v77-kani` and `v78-kani` remain live (`dg-running`) at the v79 checkpoint and cannot yet be folded to `dg-pass|dg-fail`.
  Current status (`todo` / `investigating` / `resolved`): investigating
  Reproduction command: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v77-kani` (repeat for `v78-kani`)
  Suggested next action: continue periodic status polling; fold completed outcomes into `latest_run.md/csv`, update `DEFERRED_GATES.md`, and close the item once both DG rows are folded.
