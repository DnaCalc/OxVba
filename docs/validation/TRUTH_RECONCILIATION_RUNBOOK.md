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
3. all 15 owned matrix files, the V1 schema, traceability registry, environment manifest and contract-clause disposition ledger exist and agree with the manifest
4. every normative system-contract clause appears exactly once in the disposition ledger; only the three declared extended clauses are deferred; every in-scope clause is traced through declared profile, producer/consumer epic and matrix routes, with exhaustive owner, consumer and matrix witnesses
5. the environment ledger distinguishes the noncertifying `dev-oracle` from the clean pinned x64/64-bit-Excel `certification-vm` and pinned `linux-ci`; terminal Windows/Excel evidence resolves to the certification VM
6. the program has exactly three workset roots, 42 execution epics, one rollout leaf per epic, no cycles, and no stale/global-ready work
7. executable leaves and epics have typed command, expected-observable and evidence destinations; leaves are bounded, residual-aware and traceable to matrices/rows, and every leaf contract clause appears in its trace-clause union
8. matrix truth-state, x64 target, clause, residual-owner, and terminal taxonomy stays valid; producer traces cover every target-row clause while focused `evidences`/`projects` traces may be selective; verified rows carry resolvable actual evidence and classify result/full Err/side effects/lifecycle order/transport/balance
9. closed rollouts have no scaffold/planned-row ownership; capability epics have delivery proof, support-only epics have exact support proof and downstream delivery paths, all owned required rows are verified, and LSP capabilities advertise only after decoded/direct equivalence is verified
10. before AutoRun, every owned matrix is nonempty with a required row and every execution epic has an explicit row connection
11. the manifest-derived Ideal program summary is up to date

## Expected Follow-Up

If reconciliation fails:
1. fix manifest/active-program or graph drift first,
2. fix contract-clause disposition or environment-role/pinning drift,
3. fix canonical matrix ownership, row truth, or residual ownership,
4. fix bead traceability,
5. regenerate the derived summary with `./scripts/run-truth-reconciliation.ps1 -Refresh`,
6. rerun strict reconciliation before closing the bead.

Validator changes also run the bounded negative suite:

```powershell
./scripts/test-ideal-program-validator-negative-cases.ps1
```

Its 19 isolated cases prove fail-closed behavior for missing contract clauses, missing clause-owner and clause-matrix witnesses, undeclared consumer epics and matrix routes, bead-contract clauses omitted from the trace union, traces attached to non-leaf work, x86 or stale matrix environments, noncertifying Windows evidence, empty AutoRun matrices, closed-rollout scaffolds, incomplete acceptance/observable grammar, premature LSP advertisement, missing or inconsistent resource locks, resource-concurrency overflow, and trace substitution that omits a leaf's real command/artifact acceptance.

During PROGRAM-0 migration only, `./scripts/validate-workset-rollout.ps1 -SkipReadyQueue` may be used to isolate structural/quality failures while known stale legacy ready work is being dispositioned. Governance and final rollout acceptance never use that bypass.
