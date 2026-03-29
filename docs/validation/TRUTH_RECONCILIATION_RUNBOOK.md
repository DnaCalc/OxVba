# Truth Reconciliation Runbook

Status: `active`

Use this runbook to keep canonical validation truth, summaries, and bead rollout aligned.

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

1. validation ownership files exist and remain coherent
2. active workset rollout state is structurally present
3. matrix truth-state taxonomy stays within the allowed set
4. bead-to-matrix traceability artifact is internally consistent
5. derived validation summary is up to date

## Expected Follow-Up

If reconciliation fails:
1. fix the canonical matrix or ownership issue first,
2. then fix any derived summary drift,
3. then fix bead traceability or rollout drift,
4. rerun reconciliation before closing the bead.
