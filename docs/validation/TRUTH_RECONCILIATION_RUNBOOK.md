# Truth Reconciliation Runbook

Status: `active`

Use this runbook to keep the manifest-owned Ideal program, canonical validation truth, summaries, and bead rollout aligned.

## When To Run

Run reconciliation:
1. after changing canonical validation matrices,
2. after changing ownership or audit artifacts,
3. after changing active truth summaries,
4. at regular workset cycle boundaries during large validation work.

## Command

```powershell
./scripts/run-truth-reconciliation.ps1
```

## What It Checks

1. `docs/AUTORUN_STATE.md` names the accepted manifest/root/worksets and, in AutoRun, the certified umbrella terminal gate
2. the manifest-owned 42-item legacy migration ledger agrees with bead status, current successors/imports, PROFILE-EXT deferrals, labels, and ready state
3. all 15 owned matrix files, the V1 schema, and the traceability registry exist and agree with the manifest
4. the program has exactly three workset roots, 42 execution epics, one rollout leaf per epic, no cycles, and no stale/global-ready work
5. executable leaves are bounded, routable, evidenced, residual-aware, and traceable to matrices/rows
6. matrix truth-state, x64 target, clause, residual-owner, and terminal taxonomy stays valid
7. the manifest-derived Ideal program summary is up to date

## Expected Follow-Up

If reconciliation fails:
1. fix manifest/active-program or graph drift first,
2. fix canonical matrix ownership, row truth, or residual ownership,
3. fix bead traceability,
4. regenerate the derived summary with `./scripts/run-truth-reconciliation.ps1 -Refresh`,
5. rerun strict reconciliation before closing the bead.

During PROGRAM-0 migration only, `./scripts/validate-workset-rollout.ps1 -SkipReadyQueue` may be used to isolate structural/quality failures while known stale legacy ready work is being dispositioned. Governance and final rollout acceptance never use that bypass.
